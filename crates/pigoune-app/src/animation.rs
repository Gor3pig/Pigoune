use std::path::PathBuf;
use std::time::Duration;

use gtk::{gdk, gio, glib};

const SHORTEST_FRAME: Duration = Duration::from_millis(20);

pub fn play(file: PathBuf, show_frame: impl Fn(&gdk::Texture) + 'static) -> glib::JoinHandle<()> {
    glib::spawn_future_local(async move {
        let mut loader = glycin::Loader::new(gio::File::for_path(&file));
        loader.use_expose_base_dir(false);
        let Ok(mut image) = loader.load().await else {
            return;
        };
        loop {
            let Ok(frame) = image.next_frame().await else {
                return;
            };
            show_frame(&frame.texture());
            let Some(delay) = frame.delay() else {
                return;
            };
            glib::timeout_future(delay.max(SHORTEST_FRAME)).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gtk::glib;

    use super::play;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    fn frames_shown_within(name: &str, wanted: usize, limit: Duration) -> usize {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let shown = Rc::new(Cell::new(0));
                let counter = Rc::clone(&shown);
                let playing = play(fixture(name), move |_| counter.set(counter.get() + 1));
                let started = Instant::now();
                while shown.get() < wanted && started.elapsed() < limit {
                    context.iteration(false);
                }
                playing.abort();
                shown.get()
            })
            .expect("context available")
    }

    #[test]
    fn an_animated_gif_loops_over_its_frames() {
        assert!(frames_shown_within("spinner.gif", 4, Duration::from_secs(5)) >= 4);
    }

    #[test]
    fn animated_png_and_webp_loop_over_their_frames() {
        for name in ["blinking.png", "blinking.webp"] {
            assert!(
                frames_shown_within(name, 4, Duration::from_secs(5)) >= 4,
                "{name}"
            );
        }
    }

    #[test]
    fn a_still_image_shows_a_single_frame() {
        assert_eq!(
            frames_shown_within("still.gif", 2, Duration::from_secs(2)),
            1
        );
    }
}
