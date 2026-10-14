use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;

const CALM_AFTER: Duration = Duration::from_millis(1500);
const CHECK_EVERY: Duration = Duration::from_millis(400);
const BETWEEN_ASSETS: Duration = Duration::from_millis(5);
pub const START_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct Activity(Rc<Cell<Instant>>);

impl Default for Activity {
    fn default() -> Self {
        Self(Rc::new(Cell::new(Instant::now())))
    }
}

impl Activity {
    pub fn noticed(&self) {
        self.0.set(Instant::now());
    }

    pub fn is_calm(&self) -> bool {
        self.0.get().elapsed() >= CALM_AFTER
    }

    pub async fn wait_for_calm(&self) {
        while self.0.get().elapsed() < CALM_AFTER {
            glib::timeout_future(CHECK_EVERY).await;
        }
        glib::timeout_future(BETWEEN_ASSETS).await;
    }
}
