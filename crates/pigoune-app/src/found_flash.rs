use std::cell::Cell;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

const GLOW_LEVELS: u32 = 20;
const GLOW_DURATION: Duration = Duration::from_millis(700);
const CLEANUP_MARGIN: Duration = Duration::from_millis(200);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const POLL_ATTEMPTS: u32 = 20;

pub fn flash_after_previous(find_and_flash: impl Fn() -> bool + 'static) {
    glib::timeout_add_local_once(GLOW_DURATION, move || flash_once_shown(find_and_flash));
}

pub fn flash_once_shown(find_and_flash: impl Fn() -> bool + 'static) {
    let attempts = Cell::new(0);
    glib::timeout_add_local(POLL_INTERVAL, move || {
        attempts.set(attempts.get() + 1);
        if find_and_flash() || attempts.get() >= POLL_ATTEMPTS {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

pub fn flash(widget: &gtk::Widget) {
    let started = Cell::new(None);
    let shown = Cell::new(None);
    widget.add_tick_callback(move |widget, clock| {
        let now = clock.frame_time();
        let began = started.get().unwrap_or(now);
        started.set(Some(began));
        let elapsed = Duration::from_micros(u64::try_from(now - began).unwrap_or(0));
        let level = level_at(elapsed);
        if level != shown.get() {
            show_level(widget, shown.get(), level);
            shown.set(level);
        }
        if level.is_some() {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
    let widget = widget.downgrade();
    glib::timeout_add_local_once(GLOW_DURATION + CLEANUP_MARGIN, move || {
        if let Some(widget) = widget.upgrade() {
            for level in 0..GLOW_LEVELS {
                widget.remove_css_class(&class_of(level));
            }
        }
    });
}

fn level_at(elapsed: Duration) -> Option<u32> {
    if elapsed >= GLOW_DURATION {
        return None;
    }
    let level = elapsed.as_micros() * u128::from(GLOW_LEVELS) / GLOW_DURATION.as_micros();
    u32::try_from(level).ok()
}

fn show_level(widget: &gtk::Widget, previous: Option<u32>, next: Option<u32>) {
    if let Some(previous) = previous {
        widget.remove_css_class(&class_of(previous));
    }
    if let Some(next) = next {
        widget.add_css_class(&class_of(next));
    }
}

fn class_of(level: u32) -> String {
    format!("found-{level}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{GLOW_DURATION, GLOW_LEVELS, level_at};

    #[test]
    fn the_glow_starts_at_its_first_level() {
        assert_eq!(level_at(Duration::ZERO), Some(0));
    }

    #[test]
    fn the_glow_reaches_its_last_level_just_before_the_end() {
        let almost_over = GLOW_DURATION.saturating_sub(Duration::from_millis(1));
        assert_eq!(level_at(almost_over), Some(GLOW_LEVELS - 1));
    }

    #[test]
    fn the_glow_is_gone_once_its_time_is_over() {
        assert_eq!(level_at(GLOW_DURATION), None);
        assert_eq!(level_at(GLOW_DURATION * 2), None);
    }
}
