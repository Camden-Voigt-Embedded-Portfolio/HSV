#![no_main]
#![no_std]

mod mode;
mod rgb_display;

use crate::{mode::Mode, rgb_display::RgbDisplay};
use core::sync::atomic::{
    AtomicBool,
    Ordering::{Acquire, Release},
};
use cortex_m_rt::entry;
use critical_section_lock_mut::LockMut;
use embedded_hal::digital::InputPin;
use hsv::Hsv;
use microbit::{
    board::Board,
    display::blocking::Display,
    hal::{
        Timer,
        gpio::{Floating, Input, Level, Pin},
        gpiote::Gpiote,
        pac::{self, interrupt},
        saadc::{Saadc, SaadcConfig},
    },
};
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

const FRAME_MS: u32 = 10;

fn update_hsv(hsv: Hsv, new_val: i16, mode: Mode) -> Hsv {
    let mut update_val = new_val;
    update_val = update_val.clamp(0, i16::MAX);
    let new = update_val as f32 / (i16::MAX / 2) as f32;

    match mode {
        Mode::Hue => Hsv {
            h: new,
            s: hsv.s,
            v: hsv.v,
        },
        Mode::Saturation => Hsv {
            h: hsv.h,
            s: new,
            v: hsv.v,
        },
        Mode::Value => Hsv {
            h: hsv.h,
            s: hsv.s,
            v: new,
        },
    }
}

struct AppState {
    gpiote: Gpiote,
    a_button: Pin<Input<Floating>>,
    b_button: Pin<Input<Floating>>,
}

static TIMER_MUT: LockMut<RgbDisplay> = LockMut::new();
static APP_STATE: LockMut<AppState> = LockMut::new();
static A_BUTTON_STATE: AtomicBool = AtomicBool::new(false);
static B_BUTTON_STATE: AtomicBool = AtomicBool::new(false);

#[interrupt]
fn GPIOTE() {
    APP_STATE.with_lock(|app_state| {
        let a_button_changed = app_state.gpiote.channel0().is_event_triggered();
        if a_button_changed {
            A_BUTTON_STATE.store(app_state.a_button.is_low().unwrap(), Release);
        }
        let b_button_changed = app_state.gpiote.channel1().is_event_triggered();
        if b_button_changed {
            B_BUTTON_STATE.store(app_state.b_button.is_low().unwrap(), Release);
        }
        app_state.gpiote.channel0().reset_events();
        app_state.gpiote.channel1().reset_events();
    });
}

#[interrupt]
fn TIMER0() {
    TIMER_MUT.with_lock(|rgb_display| {
        rgb_display.step();
    });
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = Board::take().unwrap();
    let mut display = Display::new(board.display_pins);
    let mut timer0 = Timer::new(board.TIMER0);
    let mut timer1 = Timer::new(board.TIMER1);
    let mut a_button = board.buttons.button_a.degrade();
    let mut b_button = board.buttons.button_b.degrade();
    let pin_r = board.edge.e08.into_push_pull_output(Level::High);
    let pin_g = board.edge.e09.into_push_pull_output(Level::Low);
    let pin_b = board.edge.e16.into_push_pull_output(Level::Low);
    let mut pin_2 = board.edge.e02.into_floating_input();
    let rgb_pins = [pin_r.degrade(), pin_g.degrade(), pin_b.degrade()];

    let saadc_config = SaadcConfig::default();
    let mut saadc = Saadc::new(board.ADC, saadc_config);

    // needed setup
    let gpiote = Gpiote::new(board.GPIOTE);
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::GPIOTE);
        pac::NVIC::unmask(pac::Interrupt::TIMER0);
    };
    pac::NVIC::unpend(pac::Interrupt::GPIOTE);
    pac::NVIC::unpend(pac::Interrupt::TIMER0);

    gpiote
        .channel0()
        .input_pin(&a_button)
        .toggle()
        .enable_interrupt();
    gpiote
        .channel1()
        .input_pin(&b_button)
        .toggle()
        .enable_interrupt();
    timer0.enable_interrupt();

    A_BUTTON_STATE.store(a_button.is_low().unwrap(), Release);
    B_BUTTON_STATE.store(b_button.is_low().unwrap(), Release);
    let mut mode = Mode::Hue;
    let rgb_display = RgbDisplay::new(rgb_pins, timer0);
    TIMER_MUT.init(rgb_display);
    let app_state = AppState {
        gpiote,
        a_button,
        b_button,
    };
    APP_STATE.init(app_state);

    let mut hsv = Hsv {
        h: 0.5,
        s: 1.0,
        v: 1.0,
    };

    let value = saadc.read_channel(&mut pin_2).unwrap();
    hsv = update_hsv(hsv, value, mode);
    TIMER_MUT.with_lock(|rgb_display| {
        rgb_display.set(&hsv);
        rgb_display.step();
    });

    loop {
        // Handle Mode Changes
        let a_pressed = A_BUTTON_STATE.load(Acquire);
        let b_pressed = B_BUTTON_STATE.load(Acquire);
        mode = match (a_pressed, b_pressed, mode) {
            (true, _, m) => m.get_prev(),
            (false, true, m) => m.get_next(),
            (false, false, m) => m,
        };

        // Handle HSV update and conversion
        let value = saadc.read_channel(&mut pin_2).unwrap();
        hsv = update_hsv(hsv, value, mode);
        TIMER_MUT.with_lock(|rgb_display| rgb_display.set(&hsv));

        // Display blocking
        display.show(&mut timer1, mode.get_display(), FRAME_MS);
    }
}
