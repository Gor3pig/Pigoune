use std::path::PathBuf;
use std::time::Duration;

use gtk::{gdk, glib};

use crate::animation::SHORTEST_FRAME;
use crate::frame_cache::FrameCache;
use crate::thumbnails;

const FRAME_BYTE_BUDGET: usize = 256 * 1024 * 1024;

#[derive(Clone)]
struct DecodedFrame {
    texture: gdk::Texture,
    delay: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackState {
    pub index: usize,
    pub frame_count: usize,
    pub playing: bool,
}

enum Command {
    TogglePlaying,
    Step(isize),
}

pub struct AnimationPlayer {
    commands: async_channel::Sender<Command>,
    task: glib::JoinHandle<()>,
}

impl AnimationPlayer {
    pub fn start(
        file: PathBuf,
        frame_count: usize,
        show: impl Fn(&gdk::Texture, PlaybackState) + 'static,
    ) -> Self {
        let (commands, received) = async_channel::unbounded();
        let source = FrameSource {
            file,
            image: None,
            cache: FrameCache::new(frame_count, FRAME_BYTE_BUDGET),
        };
        let task = glib::spawn_future_local(drive(source, received, show));
        Self { commands, task }
    }

    pub fn toggle_playing(&self) {
        let _ = self.commands.try_send(Command::TogglePlaying);
    }

    pub fn step(&self, delta: isize) {
        let _ = self.commands.try_send(Command::Step(delta));
    }
}

impl Drop for AnimationPlayer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn drive(
    mut source: FrameSource,
    commands: async_channel::Receiver<Command>,
    show: impl Fn(&gdk::Texture, PlaybackState),
) {
    let mut state = PlaybackState {
        index: 0,
        frame_count: source.cache.frame_count(),
        playing: true,
    };
    let Some(mut frame) = source.frame(0).await else {
        return;
    };
    loop {
        show(&frame.texture, state);
        let command = if state.playing {
            match glib::future_with_timeout(frame.delay.max(SHORTEST_FRAME), commands.recv()).await
            {
                Ok(Ok(command)) => Some(command),
                Ok(Err(_)) => return,
                Err(_) => None,
            }
        } else {
            let Ok(command) = commands.recv().await else {
                return;
            };
            Some(command)
        };
        match command {
            None => state.index = source.cache.stepped(state.index, 1),
            Some(Command::TogglePlaying) => state.playing = !state.playing,
            Some(Command::Step(delta)) => {
                state.playing = false;
                state.index = source.cache.stepped(state.index, delta);
            }
        }
        if let Some(next) = source.frame(state.index).await {
            frame = next;
        }
    }
}

struct FrameSource {
    file: PathBuf,
    image: Option<glycin::Image>,
    cache: FrameCache<DecodedFrame>,
}

impl FrameSource {
    async fn frame(&mut self, index: usize) -> Option<DecodedFrame> {
        if let Some(frame) = self.cache.get(index) {
            return Some(frame);
        }
        for _ in 0..=self.cache.frame_count() {
            let (decoded_index, frame) = self.decode_next().await?;
            if decoded_index == index {
                return Some(frame);
            }
        }
        None
    }

    async fn decode_next(&mut self) -> Option<(usize, DecodedFrame)> {
        if self.image.is_none() {
            self.image = Some(thumbnails::loader_for(&self.file).load().await.ok()?);
        }
        let frame = self.image.as_mut()?.next_frame().await.ok()?;
        let index = usize::try_from(frame.details().n_frame()?).ok()? % self.cache.frame_count();
        let bytes = usize::try_from(frame.stride())
            .ok()?
            .saturating_mul(usize::try_from(frame.height()).ok()?);
        let decoded = DecodedFrame {
            texture: frame.texture(),
            delay: frame.delay().unwrap_or(SHORTEST_FRAME),
        };
        self.cache.keep(index, decoded.clone(), bytes);
        Some((index, decoded))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gtk::{gdk, glib};

    use super::{AnimationPlayer, PlaybackState};

    const RED: [u8; 3] = [255, 0, 0];
    const GREEN: [u8; 3] = [0, 255, 0];
    const BLUE: [u8; 3] = [0, 0, 255];

    type Shown = Rc<RefCell<Vec<(PlaybackState, [u8; 3])>>>;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    fn first_pixel(texture: &gdk::Texture) -> [u8; 3] {
        let mut downloader = gdk::TextureDownloader::new(texture);
        downloader.set_format(gdk::MemoryFormat::R8g8b8);
        let (bytes, _) = downloader.download_bytes();
        [bytes[0], bytes[1], bytes[2]]
    }

    fn wait_until(context: &glib::MainContext, shown: &Shown, count: usize) {
        let started = Instant::now();
        while shown.borrow().len() < count && started.elapsed() < Duration::from_secs(5) {
            context.iteration(false);
        }
    }

    fn last(shown: &Shown) -> (usize, bool, [u8; 3]) {
        let (state, pixel) = *shown.borrow().last().expect("a frame was shown");
        (state.index, state.playing, pixel)
    }

    #[test]
    fn frames_can_be_paused_and_stepped_in_both_directions() {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let shown: Shown = Rc::default();
                let recorder = Rc::clone(&shown);
                let player =
                    AnimationPlayer::start(fixture("blinking.png"), 3, move |frame, state| {
                        recorder.borrow_mut().push((state, first_pixel(frame)));
                    });
                wait_until(&context, &shown, 1);
                assert_eq!(last(&shown), (0, true, RED));
                assert_eq!(shown.borrow()[0].0.frame_count, 3);

                player.toggle_playing();
                wait_until(&context, &shown, 2);
                let (paused_at, playing, _) = last(&shown);
                assert!(!playing);

                let count = shown.borrow().len();
                player.step(1);
                wait_until(&context, &shown, count + 1);
                let expected = [RED, GREEN, BLUE];
                let index = (paused_at + 1) % 3;
                assert_eq!(last(&shown), (index, false, expected[index]));

                player.step(-2);
                wait_until(&context, &shown, count + 2);
                let index = (paused_at + 2) % 3;
                assert_eq!(last(&shown), (index, false, expected[index]));
            })
            .expect("context available");
    }
}
