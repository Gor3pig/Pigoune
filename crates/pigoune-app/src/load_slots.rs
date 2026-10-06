use async_channel::{Receiver, Sender, bounded};

#[derive(Clone)]
pub struct LoadSlots {
    taken: Sender<()>,
    released: Receiver<()>,
}

pub struct Slot(Receiver<()>);

impl LoadSlots {
    pub fn new(limit: usize) -> Self {
        let (taken, released) = bounded(limit);
        Self { taken, released }
    }

    pub async fn acquire(&self) -> Slot {
        let _ = self.taken.send(()).await;
        Slot(self.released.clone())
    }

    #[cfg(test)]
    fn try_acquire(&self) -> Option<Slot> {
        self.taken
            .try_send(())
            .ok()
            .map(|()| Slot(self.released.clone()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let _ = self.0.try_recv();
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::LoadSlots;

    fn poll_once<T>(future: &mut std::pin::Pin<&mut impl Future<Output = T>>) -> Poll<T> {
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn no_more_slots_than_the_limit_are_given_at_once() {
        let slots = LoadSlots::new(2);

        let first = slots.try_acquire();
        let second = slots.try_acquire();

        assert!(first.is_some() && second.is_some());
        assert!(slots.try_acquire().is_none());
    }

    #[test]
    fn a_slot_is_given_back_when_it_is_dropped() {
        let slots = LoadSlots::new(1);
        let held = slots.try_acquire().expect("the only slot is free");
        assert!(slots.try_acquire().is_none());

        drop(held);

        assert!(slots.try_acquire().is_some());
    }

    #[test]
    fn a_request_waits_for_a_free_slot_and_gets_it_once_one_is_dropped() {
        let slots = LoadSlots::new(1);
        let held = slots.try_acquire().expect("the only slot is free");
        let mut waiting = pin!(slots.acquire());

        assert!(poll_once(&mut waiting).is_pending());
        drop(held);

        assert!(poll_once(&mut waiting).is_ready());
    }

    #[test]
    fn an_abandoned_request_never_takes_a_slot() {
        let slots = LoadSlots::new(1);
        let held = slots.try_acquire().expect("the only slot is free");
        {
            let mut abandoned = pin!(slots.acquire());
            assert!(poll_once(&mut abandoned).is_pending());
        }

        drop(held);

        assert!(slots.try_acquire().is_some());
    }
}
