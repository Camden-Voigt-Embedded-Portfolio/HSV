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
pub enum Mode {
    Hue,
    Saturation,
    Value,
}

impl Mode {
    pub fn get_next(self) -> Mode {
        match self {
            Mode::Hue => Mode::Saturation,
            Mode::Saturation => Mode::Value,
            Mode::Value => Mode::Hue,
        }
    }

    pub fn get_prev(self) -> Mode {
        match self {
            Mode::Hue => Mode::Value,
            Mode::Saturation => Mode::Hue,
            Mode::Value => Mode::Saturation,
        }
    }

    pub fn get_display(self) -> [[u8; 5]; 5] {
        match self {
            Mode::Hue => H_DISPLAY,
            Mode::Saturation => S_DISPLAY,
            Mode::Value => V_DISPLAY,
        }
    }
}
