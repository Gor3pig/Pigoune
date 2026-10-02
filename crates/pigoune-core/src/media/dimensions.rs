use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dimensions {
    width: u32,
    height: u32,
}

impl Dimensions {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Option<Self> {
        (width > 0 && height > 0).then_some(Self { width, height })
    }

    #[must_use]
    pub fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(self) -> u32 {
        self.height
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidDimensions;

impl FromStr for Dimensions {
    type Err = InvalidDimensions;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (width, height) = text.split_once('x').ok_or(InvalidDimensions)?;
        let width = width.parse().map_err(|_| InvalidDimensions)?;
        let height = height.parse().map_err(|_| InvalidDimensions)?;
        Self::new(width, height).ok_or(InvalidDimensions)
    }
}

#[cfg(test)]
mod tests {
    use super::Dimensions;

    #[test]
    fn empty_dimensions_are_refused() {
        assert_eq!(Dimensions::new(0, 16), None);
        assert_eq!(Dimensions::new(16, 0), None);
    }

    #[test]
    fn dimensions_survive_a_round_trip_through_text() {
        let dimensions = Dimensions::new(1920, 1080).expect("valid dimensions");
        assert_eq!(dimensions.to_string(), "1920x1080");
        assert_eq!("1920x1080".parse(), Ok(dimensions));
    }

    #[test]
    fn malformed_text_is_not_dimensions() {
        for text in ["", "16", "16x", "x16", "0x16", "16x-1", "ax16"] {
            assert!(text.parse::<Dimensions>().is_err(), "{text}");
        }
    }
}
