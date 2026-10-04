const SWIPE_THRESHOLD: f64 = 50.0;

#[derive(Debug, Default)]
pub struct SwipeSteps {
    travelled: f64,
    stepped: bool,
}

impl SwipeSteps {
    pub fn swiped(&mut self, horizontal: f64) -> Option<i32> {
        if self.stepped {
            return None;
        }
        self.travelled += horizontal;
        if self.travelled.abs() < SWIPE_THRESHOLD {
            return None;
        }
        self.stepped = true;
        Some(if self.travelled > 0.0 { 1 } else { -1 })
    }

    pub fn ended(&mut self) {
        *self = Self::default();
    }
}

pub fn wheel_tilt_step(horizontal: f64) -> Option<i32> {
    if horizontal > 0.0 {
        Some(1)
    } else if horizontal < 0.0 {
        Some(-1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{SwipeSteps, wheel_tilt_step};

    #[test]
    fn a_short_swipe_does_nothing() {
        let mut swipe = SwipeSteps::default();
        assert_eq!(swipe.swiped(20.0), None);
        assert_eq!(swipe.swiped(20.0), None);
    }

    #[test]
    fn a_long_swipe_steps_once_in_its_direction() {
        let mut swipe = SwipeSteps::default();
        assert_eq!(swipe.swiped(30.0), None);
        assert_eq!(swipe.swiped(30.0), Some(1));
        assert_eq!(swipe.swiped(300.0), None);

        let mut backwards = SwipeSteps::default();
        assert_eq!(backwards.swiped(-80.0), Some(-1));
    }

    #[test]
    fn a_new_swipe_can_step_again() {
        let mut swipe = SwipeSteps::default();
        assert_eq!(swipe.swiped(80.0), Some(1));
        swipe.ended();
        assert_eq!(swipe.swiped(-30.0), None);
        assert_eq!(swipe.swiped(-30.0), Some(-1));
    }

    #[test]
    fn each_wheel_tilt_steps_once() {
        assert_eq!(wheel_tilt_step(1.0), Some(1));
        assert_eq!(wheel_tilt_step(-1.0), Some(-1));
        assert_eq!(wheel_tilt_step(0.0), None);
    }
}
