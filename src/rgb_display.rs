use core::cmp::max;

use embedded_hal::digital::OutputPin;
// Outline Copied from Assignment HSV description
use hsv::Hsv;
use microbit::{
    hal::{
        Timer,
        gpio::{Output, Pin, PushPull},
    },
    pac,
};
use rtt_target::rprintln;

const FRAME_TICKS: u32 = 100;

pub struct RgbDisplay {
    // What tick of the frame are we currently on?
    // Setting to 0 starts a new frame.
    tick: u32,
    // What ticks should R, G, B LEDs turn off at?
    schedule: [u32; 3],
    // Schedule to start at next frame.
    next_schedule: Option<[u32; 3]>,
    // R, G, and B pins.
    rgb_pins: [Pin<Output<PushPull>>; 3],
    // Timer used to reach next tick.
    timer0: Timer<pac::TIMER0>,
}

impl RgbDisplay {
    pub fn new(rgb_pins: [Pin<Output<PushPull>>; 3], timer0: Timer<pac::TIMER0>) -> Self {
        RgbDisplay {
            tick: 0,
            schedule: [0, 0, 0],
            next_schedule: None,
            rgb_pins,
            timer0,
        }
    }

    /// Set up a new schedule, to be started next frame.
    pub fn set(&mut self, hsv: &Hsv) {
        let rgb = hsv.to_rgb();
        rprintln!("hsv - {}, {}, {}", hsv.h, hsv.s, hsv.v);
        rprintln!("rgb - {}, {}, {}", rgb.r, rgb.g, rgb.b);
        let r_steps = (rgb.r * 100.0) as u32;
        let g_steps = (rgb.g * 100.0) as u32;
        let b_steps = (rgb.b * 100.0) as u32;
        self.next_schedule = Some([r_steps, g_steps, b_steps]);
    }

    fn get_next_ticks(&self) -> u32 {
        let mut min = 500;
        for v in self.schedule {
            if v > self.tick && v < min {
                min = v;
            }
        }

        min
    }

    /// Take the next frame update step. Called at startup
    /// and then from the timer interrupt handler.
    pub fn step(&mut self) {
        if self.tick == 0 {
            for pin in &mut self.rgb_pins {
                pin.set_low();
            }
            if let Some(sched) = self.next_schedule {
                self.schedule = sched;
            }
        }
        if self.tick == self.schedule[0] {
            self.rgb_pins[0].set_high();
        }
        if self.tick == self.schedule[1] {
            self.rgb_pins[1].set_high();
        }
        if self.tick == self.schedule[2] {
            self.rgb_pins[2].set_high();
        }
        self.timer0.reset_event();
        let next_ticks = self.get_next_ticks();
        self.tick = if next_ticks > 100 { 0 } else { next_ticks };
        let mut cycles = (next_ticks - self.tick) * 100;
        rprintln!("{:?}", self.schedule);
        rprintln!(
            "self.tick - {}, next_ticks - {}, cycles - {}",
            self.tick,
            next_ticks,
            cycles
        );
        if cycles == 0 {
            cycles = 1;
        }
        self.timer0.start(cycles);
    }
}
