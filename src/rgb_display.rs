// Outline Copied from Assignment HSV description
use embedded_hal::digital::OutputPin;
use hsv::Hsv;
use microbit::{
    hal::{
        Timer,
        gpio::{Output, Pin, PushPull},
    },
    pac,
};

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
        let r_steps = (rgb.r * 100.0).clamp(0.0, 99.0) as u32;
        let g_steps = (rgb.g * 100.0).clamp(0.0, 99.0) as u32;
        let b_steps = (rgb.b * 100.0).clamp(0.0, 99.0) as u32;
        self.next_schedule = Some([r_steps, g_steps, b_steps]);
    }

    /// Return the next scheduled tick to stop at
    /// If Nothing scheduled return something greater than 100
    fn get_next_ticks(&self) -> u32 {
        let mut min = 100;
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
        // reset if start of new frame
        if self.tick == 0 {
            for (i, pin) in self.rgb_pins.iter_mut().enumerate() {
                if self.schedule[i] != 0 {
                    pin.set_low();
                }
            }
        }

        // Turn component off if we are at its scheduled time
        if self.tick == self.schedule[0] {
            self.rgb_pins[0].set_high();
        }
        if self.tick == self.schedule[1] {
            self.rgb_pins[1].set_high();
        }
        if self.tick == self.schedule[2] {
            self.rgb_pins[2].set_high();
        }

        // Find the next delay
        let next_ticks = self.get_next_ticks();
        let ts = if next_ticks == 100 {
            if let Some(sched) = self.next_schedule {
                self.schedule = sched;
            }
            0
        } else {
            next_ticks
        };
        let mut cycles = ((next_ticks - self.tick) * 100) / 2;
        self.tick = ts;

        // Set new timer
        // Bad thing when you send 0 so send 1 instead
        if cycles == 0 {
            cycles = 1;
        }
        self.timer0.reset_event();
        self.timer0.start(cycles);
    }
}
