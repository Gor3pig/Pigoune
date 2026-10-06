const FULL_PERCENT: f64 = 100.0;
const LARGEST_PIXEL_COUNT: u64 = 16_384 * 16_384;

#[must_use]
pub fn exceeds_limit(target: (u32, u32), natural: (u32, u32)) -> bool {
    let pixels = |(width, height): (u32, u32)| u64::from(width) * u64::from(height);
    pixels(target) > LARGEST_PIXEL_COUNT.max(pixels(natural))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    Pixels,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomSize {
    pub unit: SizeUnit,
    pub width: f64,
    pub height: f64,
    pub linked: bool,
}

impl Default for CustomSize {
    fn default() -> Self {
        Self {
            unit: SizeUnit::Percent,
            width: FULL_PERCENT,
            height: FULL_PERCENT,
            linked: true,
        }
    }
}

impl CustomSize {
    pub fn original(unit: SizeUnit, natural: (u32, u32)) -> Self {
        let (width, height) = match unit {
            SizeUnit::Pixels => (f64::from(natural.0), f64::from(natural.1)),
            SizeUnit::Percent => (FULL_PERCENT, FULL_PERCENT),
        };
        Self {
            unit,
            width,
            height,
            linked: true,
        }
    }

    pub fn target(self, natural: (u32, u32)) -> (u32, u32) {
        let (width, height) = (f64::from(natural.0), f64::from(natural.1));
        match (self.unit, self.linked) {
            (SizeUnit::Percent, true) => {
                let scale = self.width / FULL_PERCENT;
                (whole(width * scale), whole(height * scale))
            }
            (SizeUnit::Percent, false) => (
                whole(width * self.width / FULL_PERCENT),
                whole(height * self.height / FULL_PERCENT),
            ),
            (SizeUnit::Pixels, true) => {
                let scale = (self.width / width).min(self.height / height);
                (whole(width * scale), whole(height * scale))
            }
            (SizeUnit::Pixels, false) => (whole(self.width), whole(self.height)),
        }
    }

    #[must_use]
    pub fn with_width(self, width: f64, reference: (u32, u32)) -> Self {
        let height = if self.linked {
            match self.unit {
                SizeUnit::Percent => width,
                SizeUnit::Pixels => rounded(width * ratio(reference)),
            }
        } else {
            self.height
        };
        Self {
            width,
            height,
            ..self
        }
    }

    #[must_use]
    pub fn with_height(self, height: f64, reference: (u32, u32)) -> Self {
        let width = if self.linked {
            match self.unit {
                SizeUnit::Percent => height,
                SizeUnit::Pixels => rounded(height / ratio(reference)),
            }
        } else {
            self.width
        };
        Self {
            width,
            height,
            ..self
        }
    }

    #[must_use]
    pub fn with_linked(self, linked: bool, reference: (u32, u32)) -> Self {
        Self { linked, ..self }.with_width(self.width, reference)
    }

    #[must_use]
    pub fn in_unit(self, unit: SizeUnit, reference: (u32, u32)) -> Self {
        if unit == self.unit {
            return self;
        }
        let (natural_width, natural_height) = (f64::from(reference.0), f64::from(reference.1));
        let (width, height) = match unit {
            SizeUnit::Pixels => (
                rounded(natural_width * self.width / FULL_PERCENT),
                rounded(natural_height * self.height / FULL_PERCENT),
            ),
            SizeUnit::Percent => (
                rounded(self.width / natural_width * FULL_PERCENT),
                rounded(self.height / natural_height * FULL_PERCENT),
            ),
        };
        Self {
            unit,
            width,
            height,
            ..self
        }
    }
}

fn ratio(reference: (u32, u32)) -> f64 {
    f64::from(reference.1.max(1)) / f64::from(reference.0.max(1))
}

fn rounded(value: f64) -> f64 {
    value.round().max(1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    Fill,
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
    pub framing: Framing,
}

impl ScreenSize {
    pub fn scaled(self, natural: (u32, u32)) -> (u32, u32) {
        let (width, height) = (f64::from(natural.0.max(1)), f64::from(natural.1.max(1)));
        let across = f64::from(self.width) / width;
        let down = f64::from(self.height) / height;
        let scale = match self.framing {
            Framing::Fill => across.max(down),
            Framing::Fit => across.min(down),
        };
        let (scaled_width, scaled_height) = (whole(width * scale), whole(height * scale));
        match self.framing {
            Framing::Fill => (scaled_width.max(self.width), scaled_height.max(self.height)),
            Framing::Fit => (scaled_width.min(self.width), scaled_height.min(self.height)),
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the side is rounded and kept between one pixel and the largest side"
)]
fn whole(value: f64) -> u32 {
    value.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::{CustomSize, Framing, ScreenSize, SizeUnit, exceeds_limit, whole};

    fn sides(size: CustomSize) -> (u32, u32) {
        (whole(size.width), whole(size.height))
    }

    const PHOTO: (u32, u32) = (800, 600);

    #[test]
    fn a_size_beyond_the_pixel_limit_is_refused() {
        assert!(!exceeds_limit((16_384, 16_384), PHOTO));
        assert!(exceeds_limit((16_385, 16_384), PHOTO));
        assert!(exceeds_limit((153_600, 96_000), (15_360, 9_600)));
    }

    #[test]
    fn the_original_size_is_never_refused() {
        assert!(!exceeds_limit((30_000, 20_000), (30_000, 20_000)));
        assert!(!exceeds_limit((15_360, 9_600), (15_360, 9_600)));
        assert!(!exceeds_limit((100, 100), (30_000, 20_000)));
    }

    fn pixels(width: f64, height: f64, linked: bool) -> CustomSize {
        CustomSize {
            unit: SizeUnit::Pixels,
            width,
            height,
            linked,
        }
    }

    #[test]
    fn a_linked_percentage_scales_both_sides() {
        let half = CustomSize::default().with_width(50.0, PHOTO);
        assert_eq!(whole(half.height), 50);
        assert_eq!(half.target(PHOTO), (400, 300));
        assert_eq!(half.target((24, 24)), (12, 12));
    }

    #[test]
    fn free_percentages_stretch_the_image() {
        let size = CustomSize {
            linked: false,
            ..CustomSize::default()
        }
        .with_height(200.0, PHOTO);
        assert_eq!(whole(size.width), 100);
        assert_eq!(size.target(PHOTO), (800, 1200));
    }

    #[test]
    fn linked_pixels_follow_the_proportions_of_the_reference() {
        let size = pixels(800.0, 600.0, true).with_width(512.0, PHOTO);
        assert_eq!(whole(size.height), 384);
        let size = size.with_height(150.0, PHOTO);
        assert_eq!(whole(size.width), 200);
    }

    #[test]
    fn linked_pixels_fit_other_images_inside_the_same_frame() {
        let size = pixels(512.0, 384.0, true);
        assert_eq!(size.target(PHOTO), (512, 384));
        assert_eq!(size.target((100, 100)), (384, 384));
        assert_eq!(size.target((300, 900)), (128, 384));
    }

    #[test]
    fn free_pixels_give_exactly_the_typed_size() {
        assert_eq!(pixels(64.0, 32.0, false).target(PHOTO), (64, 32));
    }

    #[test]
    fn enlarging_is_allowed_when_asked() {
        let double = CustomSize::default().with_width(200.0, PHOTO);
        assert_eq!(double.target((24, 24)), (48, 48));
    }

    #[test]
    fn switching_units_keeps_the_same_size() {
        let half = CustomSize::default().with_width(50.0, PHOTO);
        let in_pixels = half.in_unit(SizeUnit::Pixels, PHOTO);
        assert_eq!(sides(in_pixels), (400, 300));
        let back = in_pixels.in_unit(SizeUnit::Percent, PHOTO);
        assert_eq!(sides(back), (50, 50));
    }

    #[test]
    fn linking_again_realigns_the_height() {
        let stretched = pixels(800.0, 100.0, false);
        let linked = stretched.with_linked(true, PHOTO);
        assert_eq!(sides(linked), (800, 600));
    }

    #[test]
    fn the_original_size_can_be_shown_in_either_unit() {
        let in_pixels = CustomSize::original(SizeUnit::Pixels, PHOTO);
        assert_eq!(sides(in_pixels), (800, 600));
        assert!(in_pixels.linked);
        assert_eq!(in_pixels.target(PHOTO), PHOTO);
        let in_percent = CustomSize::original(SizeUnit::Percent, PHOTO);
        assert_eq!(sides(in_percent), (100, 100));
        assert_eq!(in_percent.target(PHOTO), PHOTO);
    }

    #[test]
    fn a_side_is_never_smaller_than_one_pixel() {
        let tiny = CustomSize::default().with_width(1.0, PHOTO);
        assert_eq!(tiny.target((10, 10)), (1, 1));
    }

    fn screen(framing: Framing) -> ScreenSize {
        ScreenSize {
            width: 1920,
            height: 1080,
            framing,
        }
    }

    #[test]
    fn filling_the_screen_covers_both_sides() {
        assert_eq!(screen(Framing::Fill).scaled(PHOTO), (1920, 1440));
        assert_eq!(screen(Framing::Fill).scaled((1080, 1920)), (1920, 3413));
        assert_eq!(screen(Framing::Fill).scaled((3840, 2160)), (1920, 1080));
    }

    #[test]
    fn fitting_the_screen_keeps_the_whole_image_inside() {
        assert_eq!(screen(Framing::Fit).scaled(PHOTO), (1440, 1080));
        assert_eq!(screen(Framing::Fit).scaled((1080, 1920)), (608, 1080));
        assert_eq!(screen(Framing::Fit).scaled((4000, 1000)), (1920, 480));
    }
}
