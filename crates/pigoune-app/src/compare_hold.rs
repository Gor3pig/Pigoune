pub const LARGE_ORIGINAL_PIXELS: u64 = 50_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Original {
    Preparing,
    OnDemand,
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressOutcome {
    Reveal,
    Wait,
    StartLoading,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub const WHOLE: Region = Region {
    x: 0.0,
    y: 0.0,
    width: 1.0,
    height: 1.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    generation: u64,
    held: bool,
    original: Original,
}

impl Default for Comparison {
    fn default() -> Self {
        Self {
            generation: 0,
            held: false,
            original: Original::Unavailable,
        }
    }
}

impl Comparison {
    #[must_use]
    pub fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn original(self) -> Original {
        self.original
    }

    #[must_use]
    pub fn is_held(self) -> bool {
        self.held
    }

    #[must_use]
    pub fn revealed(self) -> bool {
        self.held && self.original == Original::Ready
    }

    #[must_use]
    pub fn waiting(self) -> bool {
        self.held && matches!(self.original, Original::Preparing | Original::OnDemand)
    }

    pub fn show_image(&mut self, available: bool, pixels: Option<u64>) -> u64 {
        self.generation += 1;
        self.held = false;
        self.original = if !available {
            Original::Unavailable
        } else if pixels.is_some_and(|pixels| pixels > LARGE_ORIGINAL_PIXELS) {
            Original::OnDemand
        } else {
            Original::Preparing
        };
        self.generation
    }

    pub fn press(&mut self) -> PressOutcome {
        match self.original {
            Original::Unavailable => PressOutcome::Nothing,
            Original::Ready => {
                self.held = true;
                PressOutcome::Reveal
            }
            Original::Preparing => {
                self.held = true;
                PressOutcome::Wait
            }
            Original::OnDemand => {
                self.held = true;
                self.original = Original::Preparing;
                PressOutcome::StartLoading
            }
        }
    }

    pub fn release(&mut self) {
        self.held = false;
    }

    pub fn loaded(&mut self, generation: u64, usable: bool) -> bool {
        if generation != self.generation || self.original != Original::Preparing {
            return false;
        }
        if usable {
            self.original = Original::Ready;
            self.held
        } else {
            self.original = Original::Unavailable;
            self.held = false;
            false
        }
    }
}

#[must_use]
pub fn region_of(crop: Option<(f64, f64, f64, f64)>, size: Option<(u32, u32)>) -> Option<Region> {
    let Some((x, y, width, height)) = crop else {
        return Some(WHOLE);
    };
    let (full_width, full_height) = size.filter(|(w, h)| *w > 0 && *h > 0)?;
    let (full_width, full_height) = (f64::from(full_width), f64::from(full_height));
    let left = x.clamp(0.0, full_width);
    let top = y.clamp(0.0, full_height);
    let right = (x + width).clamp(0.0, full_width);
    let bottom = (y + height).clamp(0.0, full_height);
    (right - left >= 1.0 && bottom - top >= 1.0).then(|| Region {
        x: left / full_width,
        y: top / full_height,
        width: (right - left) / full_width,
        height: (bottom - top) / full_height,
    })
}

#[must_use]
pub fn destination(bounds: (f32, f32, f32, f32), region: Region) -> (f32, f32, f32, f32) {
    let (x, y, width, height) = bounds;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "screen coordinates fit easily in f32"
    )]
    let (whole_width, whole_height) = (
        (f64::from(width) / region.width) as f32,
        (f64::from(height) / region.height) as f32,
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "screen coordinates fit easily in f32"
    )]
    let (left, top) = (
        x - (region.x * f64::from(whole_width)) as f32,
        y - (region.y * f64::from(whole_height)) as f32,
    );
    (left, top, whole_width, whole_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> (Comparison, u64) {
        let mut comparison = Comparison::default();
        let generation = comparison.show_image(true, Some(1_000_000));
        assert!(!comparison.loaded(generation, true));
        (comparison, generation)
    }

    #[test]
    fn an_ordinary_image_is_prepared_and_a_huge_one_waits_for_the_first_press() {
        let mut comparison = Comparison::default();
        comparison.show_image(true, Some(1_000_000));
        assert_eq!(comparison.original(), Original::Preparing);
        comparison.show_image(true, Some(LARGE_ORIGINAL_PIXELS + 1));
        assert_eq!(comparison.original(), Original::OnDemand);
        comparison.show_image(true, None);
        assert_eq!(comparison.original(), Original::Preparing);
        comparison.show_image(false, Some(10));
        assert_eq!(comparison.original(), Original::Unavailable);
    }

    #[test]
    fn nothing_is_revealed_until_the_original_is_ready_and_the_button_is_held() {
        let mut comparison = Comparison::default();
        let generation = comparison.show_image(true, Some(1_000_000));
        assert!(!comparison.revealed());
        assert_eq!(comparison.press(), PressOutcome::Wait);
        assert!(comparison.waiting() && !comparison.revealed());
        assert!(
            comparison.loaded(generation, true),
            "still held: reveal now"
        );
        assert!(comparison.revealed());
        comparison.release();
        assert!(!comparison.revealed());
    }

    #[test]
    fn an_original_that_arrives_after_the_release_is_never_revealed() {
        let mut comparison = Comparison::default();
        let generation = comparison.show_image(true, Some(1_000_000));
        comparison.press();
        comparison.release();
        assert!(!comparison.loaded(generation, true));
        assert!(!comparison.revealed());
        assert_eq!(comparison.original(), Original::Ready);
        assert_eq!(comparison.press(), PressOutcome::Reveal);
        assert!(comparison.revealed());
    }

    #[test]
    fn the_first_press_loads_a_huge_original_and_reveals_it_only_if_still_held() {
        let mut comparison = Comparison::default();
        let generation = comparison.show_image(true, Some(LARGE_ORIGINAL_PIXELS + 1));
        assert_eq!(comparison.press(), PressOutcome::StartLoading);
        assert!(comparison.waiting());
        assert!(comparison.loaded(generation, true));

        let generation = comparison.show_image(true, Some(LARGE_ORIGINAL_PIXELS + 1));
        assert_eq!(comparison.press(), PressOutcome::StartLoading);
        comparison.release();
        assert!(!comparison.loaded(generation, true));
        assert!(!comparison.revealed());
    }

    #[test]
    fn a_result_for_an_image_that_is_no_longer_shown_is_ignored() {
        let mut comparison = Comparison::default();
        let old = comparison.show_image(true, Some(1_000_000));
        comparison.press();
        let new = comparison.show_image(true, Some(1_000_000));
        assert_ne!(old, new);
        assert!(!comparison.is_held(), "changing the image releases");
        assert!(!comparison.loaded(old, true), "the old result is dropped");
        assert_eq!(comparison.original(), Original::Preparing);
        assert!(!comparison.loaded(new, true));
        assert_eq!(comparison.original(), Original::Ready);
    }

    #[test]
    fn a_missing_or_unreadable_original_reveals_nothing_and_releases() {
        let mut comparison = Comparison::default();
        let generation = comparison.show_image(true, Some(1_000_000));
        comparison.press();
        assert!(!comparison.loaded(generation, false));
        assert_eq!(comparison.original(), Original::Unavailable);
        assert!(!comparison.is_held() && !comparison.revealed());
        assert_eq!(comparison.press(), PressOutcome::Nothing);
        assert!(!comparison.revealed());
    }

    #[test]
    fn every_interruption_puts_the_annotated_image_back() {
        for interruption in 0..6 {
            let (mut comparison, _) = ready();
            assert_eq!(comparison.press(), PressOutcome::Reveal);
            assert!(comparison.revealed());
            match interruption {
                0..=4 => comparison.release(),
                _ => {
                    comparison.show_image(true, Some(10));
                }
            }
            assert!(!comparison.revealed(), "interruption {interruption}");
        }
    }

    #[test]
    fn quick_repeated_presses_never_leave_the_original_shown() {
        let (mut comparison, _) = ready();
        for _ in 0..50 {
            assert_eq!(comparison.press(), PressOutcome::Reveal);
            assert!(comparison.revealed());
            comparison.release();
            assert!(!comparison.revealed());
            comparison.release();
            assert!(!comparison.revealed());
        }
    }

    #[test]
    fn without_a_crop_the_whole_original_is_shown_in_the_frame() {
        assert_eq!(region_of(None, None), Some(WHOLE));
        assert_eq!(region_of(None, Some((400, 300))), Some(WHOLE));
        let frame = (10.0, 20.0, 200.0, 100.0);
        assert_eq!(destination(frame, WHOLE), frame);
    }

    #[test]
    fn a_crop_maps_its_region_of_the_original_onto_the_frame_of_the_annotated_image() {
        let region =
            region_of(Some((100.0, 50.0, 200.0, 100.0)), Some((400, 200))).expect("region");
        assert_eq!(
            region,
            Region {
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5
            }
        );
        let (left, top, width, height) = destination((10.0, 20.0, 200.0, 100.0), region);
        assert_eq!((width, height), (400.0, 200.0));
        assert_eq!((left, top), (10.0 - 100.0, 20.0 - 50.0));
    }

    #[test]
    fn a_crop_without_the_size_of_the_image_or_outside_it_is_refused() {
        assert_eq!(region_of(Some((0.0, 0.0, 10.0, 10.0)), None), None);
        assert_eq!(region_of(Some((0.0, 0.0, 10.0, 10.0)), Some((0, 0))), None);
        assert_eq!(
            region_of(Some((500.0, 500.0, 10.0, 10.0)), Some((100, 100))),
            None
        );
        let clamped =
            region_of(Some((-50.0, -50.0, 100.0, 100.0)), Some((100, 100))).expect("region");
        assert_eq!(
            (clamped.x, clamped.y, clamped.width, clamped.height),
            (0.0, 0.0, 0.5, 0.5)
        );
    }

    #[test]
    fn the_hold_never_treats_the_stopped_signal_of_a_click_as_a_release_or_ignores_an_interruption()
    {
        let preview = include_str!("asset_preview.rs");
        assert!(
            !preview.contains("connect_stopped"),
            "a click gesture stops after the double-click delay while the button is still held"
        );
        for interruption in [
            "connect_cancel",
            "connect_released",
            "connect_unpaired_release",
            "connect_key_released",
            "EventControllerFocus",
            "connect_is_active_notify",
            "connect_unmap",
        ] {
            assert!(
                preview.contains(interruption),
                "{interruption} puts the annotated image back"
            );
        }
    }
}
