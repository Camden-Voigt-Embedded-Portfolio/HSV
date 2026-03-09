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
use rtt_target::rtt_init_print;

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

struct Buttons {
    timer_a: Timer<pac::TIMER2>,
    timer_b: Timer<pac::TIMER3>,
    button_a: Pin<Input<Floating>>,
    button_b: Pin<Input<Floating>>,
}
static DEBOUNCE_BUTTONS: LockMut<Buttons> = LockMut::new();
static GPIOTE: LockMut<Gpiote> = LockMut::new();
static A_BUTTON_STATE: AtomicBool = AtomicBool::new(false);
static B_BUTTON_STATE: AtomicBool = AtomicBool::new(false);

static RGB_MUT: LockMut<RgbDisplay> = LockMut::new();

const DEBOUNCE_TIME: u32 = 100 * 1_000_000 / 1000;
#[interrupt]
fn GPIOTE() {
    DEBOUNCE_BUTTONS.with_lock(|buttons| {
        if buttons.timer_a.read() == 0 {
            A_BUTTON_STATE.store(buttons.button_a.is_low().unwrap(), Release);
            buttons.timer_a.start(DEBOUNCE_TIME);
        }
        if buttons.timer_b.read() == 0 {
            B_BUTTON_STATE.store(buttons.button_b.is_low().unwrap(), Release);
            buttons.timer_b.start(DEBOUNCE_TIME);
        }
    });
    GPIOTE.with_lock(|gpiote| {
        gpiote.channel0().reset_events();
        gpiote.channel1().reset_events();
    });
}

#[interrupt]
fn TIMER0() {
    RGB_MUT.with_lock(|rgb_display| {
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
    let timer2 = Timer::new(board.TIMER2);
    let timer3 = Timer::new(board.TIMER3);
    let mut a_button = board.buttons.button_a.degrade();
    let mut b_button = board.buttons.button_b.degrade();
    let pin_r = board.edge.e08.into_push_pull_output(Level::High);
    let pin_g = board.edge.e09.into_push_pull_output(Level::Low);
    let pin_b = board.edge.e16.into_push_pull_output(Level::Low);
    let mut pin_2 = board.edge.e02.into_floating_input();
    let rgb_pins = [pin_r.degrade(), pin_g.degrade(), pin_b.degrade()];

    let saadc_config = SaadcConfig::default();
    let mut saadc = Saadc::new(board.ADC, saadc_config);

    // NVIC setup
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::GPIOTE);
        pac::NVIC::unmask(pac::Interrupt::TIMER0);
    };
    pac::NVIC::unpend(pac::Interrupt::GPIOTE);
    pac::NVIC::unpend(pac::Interrupt::TIMER0);

    // Setup GPIOTE
    let gpiote = Gpiote::new(board.GPIOTE);
    gpiote
        .channel0()
        .input_pin(&a_button)
        .hi_to_lo()
        .enable_interrupt();
    gpiote
        .channel1()
        .input_pin(&b_button)
        .hi_to_lo()
        .enable_interrupt();
    GPIOTE.init(gpiote);

    // Setup buttons
    A_BUTTON_STATE.store(a_button.is_low().unwrap(), Release);
    B_BUTTON_STATE.store(b_button.is_low().unwrap(), Release);
    DEBOUNCE_BUTTONS.init(Buttons {
        timer_a: timer2,
        timer_b: timer3,
        button_a: a_button,
        button_b: b_button,
    });

    // Setup timer for PWM
    timer0.enable_interrupt();
    let rgb_display = RgbDisplay::new(rgb_pins, timer0);
    RGB_MUT.init(rgb_display);

    // Setup State
    let mut mode = Mode::Hue;
    let mut hsv = Hsv {
        h: 0.5,
        s: 1.0,
        v: 1.0,
    };

    let value = saadc.read_channel(&mut pin_2).unwrap();
    hsv = update_hsv(hsv, value, mode);
    RGB_MUT.with_lock(|rgb_display| {
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
        A_BUTTON_STATE.store(false, Release);
        B_BUTTON_STATE.store(false, Release);

        // Handle HSV update and conversion
        let value = saadc.read_channel(&mut pin_2).unwrap();
        hsv = update_hsv(hsv, value, mode);
        RGB_MUT.with_lock(|rgb_display| rgb_display.set(&hsv));

        // Display blocking
        display.show(&mut timer1, mode.get_display(), 100);
    }
}
