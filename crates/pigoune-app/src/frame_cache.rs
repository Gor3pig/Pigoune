pub struct FrameCache<T> {
    frames: Vec<Option<T>>,
    kept_bytes: usize,
    byte_budget: usize,
}

impl<T: Clone> FrameCache<T> {
    pub fn new(frame_count: usize, byte_budget: usize) -> Self {
        Self {
            frames: vec![None; frame_count],
            kept_bytes: 0,
            byte_budget,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn get(&self, index: usize) -> Option<T> {
        self.frames.get(index).cloned().flatten()
    }

    pub fn keep(&mut self, index: usize, frame: T, bytes: usize) {
        let Some(slot) = self.frames.get_mut(index) else {
            return;
        };
        if slot.is_some() || self.kept_bytes + bytes > self.byte_budget {
            return;
        }
        *slot = Some(frame);
        self.kept_bytes += bytes;
    }

    pub fn stepped(&self, index: usize, delta: isize) -> usize {
        let count = isize::try_from(self.frame_count())
            .unwrap_or(isize::MAX)
            .max(1);
        let index = isize::try_from(index).unwrap_or(0);
        usize::try_from((index + delta).rem_euclid(count)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::FrameCache;

    #[test]
    fn kept_frames_come_back() {
        let mut cache = FrameCache::new(3, 100);
        cache.keep(1, "b", 10);
        assert_eq!(cache.get(1), Some("b"));
        assert_eq!(cache.get(0), None);
        assert_eq!(cache.get(7), None);
    }

    #[test]
    fn frames_beyond_the_budget_are_not_kept() {
        let mut cache = FrameCache::new(3, 25);
        cache.keep(0, "a", 10);
        cache.keep(1, "b", 10);
        cache.keep(2, "c", 10);
        assert_eq!(cache.get(1), Some("b"));
        assert_eq!(cache.get(2), None);
    }

    #[test]
    fn a_frame_is_counted_once() {
        let mut cache = FrameCache::new(2, 15);
        cache.keep(0, "a", 10);
        cache.keep(0, "a", 10);
        cache.keep(1, "b", 5);
        assert_eq!(cache.get(1), Some("b"));
    }

    #[test]
    fn stepping_wraps_around_the_animation() {
        let cache = FrameCache::<()>::new(4, 0);
        assert_eq!(cache.stepped(3, 1), 0);
        assert_eq!(cache.stepped(0, -1), 3);
        assert_eq!(cache.stepped(1, 1), 2);
    }
}
