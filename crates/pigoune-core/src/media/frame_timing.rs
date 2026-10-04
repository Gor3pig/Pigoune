use std::time::Duration;

use super::AnimationTiming;

const SHORTEST_HONORED_DURATION: Duration = Duration::from_millis(11);
const DURATION_OF_A_TOO_SHORT_FRAME: Duration = Duration::from_millis(100);
const FRAMES_OF_AN_ANIMATION: usize = 2;

#[derive(Debug, Default)]
pub struct FrameTally {
    frames: usize,
    total: Duration,
}

impl FrameTally {
    pub fn add(&mut self, declared: Duration) {
        self.frames += 1;
        self.total += displayed_duration(declared);
    }

    pub fn is_animation(&self) -> bool {
        self.frames >= FRAMES_OF_AN_ANIMATION
    }

    pub fn timing(&self) -> Option<AnimationTiming> {
        (self.frames > 0).then_some(AnimationTiming {
            frames: self.frames,
            duration: self.total,
        })
    }
}

pub fn displayed_duration(declared: Duration) -> Duration {
    if declared < SHORTEST_HONORED_DURATION {
        DURATION_OF_A_TOO_SHORT_FRAME
    } else {
        declared
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{FrameTally, displayed_duration};

    #[test]
    fn a_frame_of_ten_milliseconds_or_less_lasts_a_tenth_of_a_second_like_in_browsers() {
        for declared in [0, 1, 10] {
            assert_eq!(
                displayed_duration(Duration::from_millis(declared)),
                Duration::from_millis(100)
            );
        }
        assert_eq!(
            displayed_duration(Duration::from_millis(20)),
            Duration::from_millis(20)
        );
    }

    #[test]
    fn a_tally_adds_up_the_displayed_durations() {
        let mut tally = FrameTally::default();
        assert_eq!(tally.timing(), None);
        tally.add(Duration::from_millis(500));
        assert!(!tally.is_animation());
        tally.add(Duration::ZERO);
        assert!(tally.is_animation());
        let timing = tally.timing().expect("frames counted");
        assert_eq!(timing.frames, 2);
        assert_eq!(timing.duration, Duration::from_millis(600));
    }
}
