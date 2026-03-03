#![no_main]
#![no_std]

use core::sync::atomic::{
    AtomicBool,
    Ordering::{Acquire, Release},
};

use cortex_m_rt::entry;
use critical_section_lock_mut::LockMut;
use embedded_hal::digital::{InputPin, OutputPin};
use hsv::Hsv;
use microbit::{
    board::Board,
    display::blocking::Display,
    hal::{
        Timer,
        gpio::{Floating, Input, Level, Pin},
        gpiote::Gpiote,
        pac::{self, TIMER1, interrupt},
        saadc::{Saadc, SaadcConfig},
    },
};
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

const FRAME_MS: u32 = 200;

// Display arrays
const H_DISPLAY: [[u8; 5]; 5] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 1, 1, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
];
const S_DISPLAY: [[u8; 5]; 5] = [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 0],
    [0, 1, 1, 0, 0],
    [0, 0, 0, 1, 0],
    [1, 1, 1, 0, 0],
];
const V_DISPLAY: [[u8; 5]; 5] = [
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 1, 0, 1, 0],
    [0, 0, 1, 0, 0],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hue,
    Saturation,
    Value,
}

fn get_prev_mode(s: Mode) -> Mode {
    match s {
        Mode::Hue => Mode::Value,
        Mode::Saturation => Mode::Hue,
        Mode::Value => Mode::Saturation,
    }
}

fn get_next_mode(s: Mode) -> Mode {
    match s {
        Mode::Hue => Mode::Saturation,
        Mode::Saturation => Mode::Value,
        Mode::Value => Mode::Hue,
    }
}

fn display_for_mode(display: &mut Display, timer: &mut Timer<TIMER1>, duration: u32, s: Mode) {
    match s {
        Mode::Hue => display.show(timer, H_DISPLAY, duration),
        Mode::Saturation => display.show(timer, S_DISPLAY, duration),
        Mode::Value => display.show(timer, V_DISPLAY, duration),
    };
}

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

static APP_STATE: LockMut<AppState> = LockMut::new();
static A_BUTTON_STATE: AtomicBool = AtomicBool::new(false);
static B_BUTTON_STATE: AtomicBool = AtomicBool::new(false);

#[interrupt]
fn GPIOTE() {
    APP_STATE.with_lock(|app_state| {
        rprintln!("interrupt");
        let a_button_changed = app_state.gpiote.channel0().is_event_triggered();
        if a_button_changed {
            A_BUTTON_STATE.store(app_state.a_button.is_low().unwrap(), Release);
            rprintln!("interrupt - a_pressed");
        }
        let b_button_changed = app_state.gpiote.channel1().is_event_triggered();
        if b_button_changed {
            rprintln!("interrupt - b_pressed");
            B_BUTTON_STATE.store(app_state.b_button.is_low().unwrap(), Release);
        }
        app_state.gpiote.channel0().reset_events();
        app_state.gpiote.channel1().reset_events();
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
    let mut rgb_pins = [pin_r.degrade(), pin_g.degrade(), pin_b.degrade()];

    let saadc_config = SaadcConfig::default();
    let mut saadc = Saadc::new(board.ADC, saadc_config);

    // needed setup
    let gpiote = Gpiote::new(board.GPIOTE);
    unsafe { pac::NVIC::unmask(pac::Interrupt::GPIOTE) };
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
    let app_state = AppState {
        gpiote,
        a_button,
        b_button,
    };
    APP_STATE.init(app_state);
    let mut mode = Mode::Hue;

    let mut hsv = Hsv {
        h: 0.0,
        s: 1.0,
        v: 1.0,
    };

    loop {
        let a_pressed = A_BUTTON_STATE.load(Acquire);
        let b_pressed = B_BUTTON_STATE.load(Acquire);
        mode = match (a_pressed, b_pressed, mode) {
            (true, _, m) => {
                rprintln!("a_pressed");
                get_prev_mode(m)
            }
            (false, true, m) => {
                rprintln!("b_pressed");
                get_next_mode(m)
            }
            (false, false, m) => {
                rprintln!("none pressed");
                m
            }
        };

        display_for_mode(&mut display, &mut timer1, FRAME_MS, mode);
        let value = saadc.read_channel(&mut pin_2);
        rprintln!("ADC: {}", value.unwrap());
        hsv = update_hsv(hsv, value.unwrap(), mode);
        rprintln!("hsv: {},{},{}", hsv.h, hsv.s, hsv.v);
        let rgb = hsv.to_rgb();

        if rgb.r > 0.5 {
            rgb_pins[0].set_low();
        } else {
            rgb_pins[0].set_high();
        }

        if rgb.g > 0.5 {
            rgb_pins[1].set_low();
        } else {
            rgb_pins[1].set_high();
        }

        if rgb.b > 0.5 {
            rgb_pins[2].set_low();
        } else {
            rgb_pins[2].set_high();
        }
    }
}
