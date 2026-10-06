use super::Dimensions;

const SQUARE_TOLERANCE_PERCENT: u64 = 5;
const CODE_SEPARATOR: char = ',';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssetShape {
    Landscape,
    Portrait,
    Square,
}

impl AssetShape {
    pub const ALL: [Self; 3] = [Self::Landscape, Self::Portrait, Self::Square];

    #[must_use]
    pub fn of(size: Dimensions) -> Self {
        let (width, height) = (u64::from(size.width()), u64::from(size.height()));
        let longest = width.max(height);
        let gap = width.abs_diff(height);
        if gap * 100 <= longest * SQUARE_TOLERANCE_PERCENT {
            Self::Square
        } else if width > height {
            Self::Landscape
        } else {
            Self::Portrait
        }
    }

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Landscape => "landscape",
            Self::Portrait => "portrait",
            Self::Square => "square",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shape| shape.code() == code)
    }
}

#[must_use]
pub fn shapes_text(shapes: &[AssetShape]) -> String {
    shapes
        .iter()
        .map(|shape| shape.code())
        .collect::<Vec<_>>()
        .join(&CODE_SEPARATOR.to_string())
}

#[must_use]
pub fn shapes_from_text(text: &str) -> Vec<AssetShape> {
    text.split(CODE_SEPARATOR)
        .filter_map(AssetShape::from_code)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AssetShape, shapes_from_text, shapes_text};
    use crate::media::Dimensions;

    fn shape(width: u32, height: u32) -> AssetShape {
        AssetShape::of(Dimensions::new(width, height).expect("valid size"))
    }

    #[test]
    fn wider_images_are_landscapes_and_taller_ones_portraits() {
        assert_eq!(shape(1920, 1080), AssetShape::Landscape);
        assert_eq!(shape(1080, 1920), AssetShape::Portrait);
    }

    #[test]
    fn nearly_equal_sides_make_a_square() {
        assert_eq!(shape(512, 512), AssetShape::Square);
        assert_eq!(shape(1000, 950), AssetShape::Square);
        assert_eq!(shape(1000, 949), AssetShape::Landscape);
        assert_eq!(shape(950, 1000), AssetShape::Square);
    }

    #[test]
    fn shapes_survive_a_round_trip_through_text() {
        let shapes = vec![AssetShape::Landscape, AssetShape::Square];
        assert_eq!(shapes_from_text(&shapes_text(&shapes)), shapes);
        assert!(shapes_from_text("").is_empty());
        assert_eq!(shapes_from_text("square,unknown"), [AssetShape::Square]);
    }
}
