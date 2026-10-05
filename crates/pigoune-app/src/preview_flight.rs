use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

pub type RectSource = Box<dyn Fn() -> Option<graphene::Rect>>;
type FlightDone = Box<dyn FnOnce()>;

const LONGEST_STEP_MILLISECONDS: f64 = 1000.0 / 30.0;

pub struct Flight {
    pub texture: gdk::Texture,
    pub from: RectSource,
    pub to: RectSource,
    pub milliseconds: u32,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use super::{Flight, FlightDone};

    #[derive(Default)]
    pub struct PigounePreviewFlight {
        pub flight: RefCell<Option<Flight>>,
        pub done: RefCell<Option<FlightDone>>,
        pub ticking: RefCell<Option<gtk::TickCallbackId>>,
        pub elapsed: Cell<f64>,
        pub last_frame: Cell<Option<i64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigounePreviewFlight {
        const NAME: &'static str = "PigounePreviewFlight";
        type Type = super::PigounePreviewFlight;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PigounePreviewFlight {
        fn constructed(&self) {
            self.parent_constructed();
            let flight = self.obj();
            flight.set_can_target(false);
            flight.set_hexpand(true);
            flight.set_vexpand(true);
        }
    }

    impl WidgetImpl for PigounePreviewFlight {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct PigounePreviewFlight(ObjectSubclass<imp::PigounePreviewFlight>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigounePreviewFlight {
    pub fn fly(&self, flight: Flight, done: impl FnOnce() + 'static) {
        self.land();
        let imp = self.imp();
        imp.flight.replace(Some(flight));
        imp.done.replace(Some(Box::new(done)));
        imp.elapsed.set(0.0);
        imp.last_frame.set(None);
        if !self.settings().is_gtk_enable_animations() {
            self.finish();
            return;
        }
        self.queue_draw();
        let ticking = self.add_tick_callback(|view, clock| view.advance(clock.frame_time()));
        imp.ticking.replace(Some(ticking));
    }

    pub fn land(&self) {
        let ticking = self.imp().ticking.take();
        if let Some(ticking) = ticking {
            ticking.remove();
        }
        self.finish();
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "frame times are microseconds since startup, exact in f64"
    )]
    fn advance(&self, frame_time: i64) -> glib::ControlFlow {
        let imp = self.imp();
        if let Some(last) = imp.last_frame.replace(Some(frame_time)) {
            let step = (frame_time - last) as f64 / 1000.0;
            imp.elapsed
                .set(imp.elapsed.get() + step.min(LONGEST_STEP_MILLISECONDS));
        }
        self.queue_draw();
        if self.raw_progress() < 1.0 {
            return glib::ControlFlow::Continue;
        }
        imp.ticking.replace(None);
        self.finish();
        glib::ControlFlow::Break
    }

    fn finish(&self) {
        let imp = self.imp();
        imp.flight.replace(None);
        self.queue_draw();
        let done = imp.done.take();
        if let Some(done) = done {
            done();
        }
    }

    fn raw_progress(&self) -> f64 {
        let imp = self.imp();
        let duration = imp
            .flight
            .borrow()
            .as_ref()
            .map_or(0, |flight| flight.milliseconds);
        if duration == 0 {
            return 1.0;
        }
        (imp.elapsed.get() / f64::from(duration)).min(1.0)
    }

    fn progress(&self) -> f64 {
        adw::Easing::EaseOutCubic.ease(self.raw_progress())
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let flight = self.imp().flight.borrow();
        let Some(flight) = flight.as_ref() else {
            return;
        };
        let Some(from) = (flight.from)() else {
            return;
        };
        let progress = self.progress();
        let (bounds, opacity) = match (flight.to)() {
            Some(to) => (between(&from, &to, progress), 1.0),
            None => (from, 1.0 - progress),
        };
        snapshot.push_opacity(opacity);
        snapshot.append_scaled_texture(&flight.texture, gsk::ScalingFilter::Linear, &bounds);
        snapshot.pop();
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
pub fn between(from: &graphene::Rect, to: &graphene::Rect, progress: f64) -> graphene::Rect {
    let step = progress.clamp(0.0, 1.0) as f32;
    let mix = |start: f32, end: f32| start + (end - start) * step;
    graphene::Rect::new(
        mix(from.x(), to.x()),
        mix(from.y(), to.y()),
        mix(from.width(), to.width()),
        mix(from.height(), to.height()),
    )
}

#[cfg(test)]
mod tests {
    use gtk::graphene;

    use super::between;

    #[test]
    fn between_starts_at_the_origin_and_ends_at_the_destination() {
        let from = graphene::Rect::new(10.0, 20.0, 100.0, 50.0);
        let to = graphene::Rect::new(110.0, 220.0, 300.0, 150.0);
        assert_eq!(between(&from, &to, 0.0), from);
        assert_eq!(between(&from, &to, 1.0), to);
        assert_eq!(
            between(&from, &to, 0.5),
            graphene::Rect::new(60.0, 120.0, 200.0, 100.0)
        );
    }

    #[test]
    fn between_never_overshoots() {
        let from = graphene::Rect::new(0.0, 0.0, 10.0, 10.0);
        let to = graphene::Rect::new(100.0, 100.0, 20.0, 20.0);
        assert_eq!(between(&from, &to, 1.5), to);
    }
}
