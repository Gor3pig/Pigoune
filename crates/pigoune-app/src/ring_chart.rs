use std::f32::consts::{FRAC_PI_2, TAU};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

const SIDE: i32 = 168;
const THICKNESS_RATIO: f32 = 0.16;
const GAP: f32 = 0.025;

mod imp {
    use std::cell::RefCell;

    use gtk::subclass::prelude::*;
    use gtk::{gdk, glib};

    #[derive(Default)]
    pub struct PigouneRingChart {
        pub parts: RefCell<Vec<(f32, gdk::RGBA)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneRingChart {
        const NAME: &'static str = "PigouneRingChart";
        type Type = super::PigouneRingChart;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for PigouneRingChart {}

    impl WidgetImpl for PigouneRingChart {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (super::SIDE, super::SIDE, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct PigouneRingChart(ObjectSubclass<imp::PigouneRingChart>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneRingChart {
    pub fn set_parts(&self, parts: Vec<(f32, gdk::RGBA)>) {
        self.imp().parts.replace(parts);
        self.queue_draw();
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "widget sides are small enough to be exact as floats"
    )]
    fn draw(&self, snapshot: &gtk::Snapshot) {
        let side = self.width().min(self.height()) as f32;
        let thickness = side * THICKNESS_RATIO;
        let radius = (side - thickness) / 2.0;
        let center = graphene::Point::new(self.width() as f32 / 2.0, self.height() as f32 / 2.0);
        let stroke = gsk::Stroke::new(thickness);
        let parts = self.imp().parts.borrow();
        let gap = if parts.len() > 1 { GAP } else { 0.0 };
        let mut start = -FRAC_PI_2;
        for (fraction, color) in parts.iter() {
            let sweep = fraction * TAU;
            if sweep > gap {
                snapshot.append_stroke(&arc(center, radius, start, sweep - gap), &stroke, color);
            }
            start += sweep;
        }
    }
}

fn arc(center: graphene::Point, radius: f32, start: f32, sweep: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    if sweep >= TAU - f32::EPSILON {
        builder.add_circle(&center, radius);
        return builder.to_path();
    }
    let point = |angle: f32| {
        (
            center.x() + radius * angle.cos(),
            center.y() + radius * angle.sin(),
        )
    };
    let (start_x, start_y) = point(start);
    let (end_x, end_y) = point(start + sweep);
    builder.move_to(start_x, start_y);
    builder.svg_arc_to(radius, radius, 0.0, sweep > TAU / 2.0, true, end_x, end_y);
    builder.to_path()
}
