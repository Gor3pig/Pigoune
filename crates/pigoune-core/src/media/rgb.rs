const HEX_DIGITS: usize = 6;
const HEX_PREFIX: char = '#';
const BYTE_MAX: f64 = 255.0;
const CLOSE_DISTANCE: f64 = 0.12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[must_use]
    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.trim().trim_start_matches(HEX_PREFIX);
        if digits.len() != HEX_DIGITS || !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |start: usize| u8::from_str_radix(&digits[start..start + 2], 16).ok();
        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }

    #[must_use]
    pub fn is_close_to(self, other: Self) -> bool {
        self.distance_to(other) <= CLOSE_DISTANCE
    }

    fn distance_to(self, other: Self) -> f64 {
        let [l1, a1, b1] = self.oklab();
        let [l2, a2, b2] = other.oklab();
        ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
    }

    fn oklab(self) -> [f64; 3] {
        let [red, green, blue] = [self.red, self.green, self.blue].map(linear);
        let long =
            (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
        let medium =
            (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
        let short =
            (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();
        [
            0.210_454_255_3 * long + 0.793_617_785_0 * medium - 0.004_072_046_8 * short,
            1.977_998_495_1 * long - 2.428_592_205_0 * medium + 0.450_593_709_9 * short,
            0.025_904_037_1 * long + 0.782_771_766_2 * medium - 0.808_675_766_0 * short,
        ]
    }
}

fn linear(channel: u8) -> f64 {
    let value = f64::from(channel) / BYTE_MAX;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::Rgb;

    const BRICK: Rgb = Rgb::new(0xc0, 0x39, 0x2b);

    fn hex(text: &str) -> Rgb {
        Rgb::from_hex(text).expect("valid color")
    }

    #[test]
    fn a_color_is_written_and_read_as_six_hex_digits() {
        assert_eq!(BRICK.hex(), "c0392b");
        assert_eq!(Rgb::from_hex("#C0392B"), Some(BRICK));
        assert_eq!(Rgb::from_hex("c0392b"), Some(BRICK));
        assert_eq!(Rgb::from_hex("c0392"), None);
        assert_eq!(Rgb::from_hex("g0392b"), None);
    }

    #[test]
    fn close_colors_follow_what_the_eye_sees() {
        assert!(BRICK.is_close_to(hex("e01b24")));
        assert!(BRICK.is_close_to(hex("a51d2d")));
        assert!(BRICK.is_close_to(hex("b5482f")));
        assert!(BRICK.is_close_to(hex("986a44")));
        assert!(!BRICK.is_close_to(hex("f66151")));
        assert!(!BRICK.is_close_to(hex("ff7800")));
        assert!(!BRICK.is_close_to(hex("d56199")));
        assert!(!BRICK.is_close_to(hex("3584e4")));
    }

    #[test]
    fn a_color_is_close_to_itself() {
        assert!(BRICK.is_close_to(BRICK));
        assert!(hex("ffffff").is_close_to(hex("fafafa")));
        assert!(!hex("ffffff").is_close_to(hex("000000")));
    }
}
