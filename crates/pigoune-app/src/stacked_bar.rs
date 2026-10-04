use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

const HEIGHT: i32 = 14;

mod imp {
    use std::cell::RefCell;

    use gtk::subclass::prelude::*;
    use gtk::{gdk, glib};

    #[derive(Default)]
    pub struct PigouneStackedBar {
        pub parts: RefCell<Vec<(f32, gdk::RGBA)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneStackedBar {
        const NAME: &'static str = "PigouneStackedBar";
        type Type = super::PigouneStackedBar;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for PigouneStackedBar {}

    impl WidgetImpl for PigouneStackedBar {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Vertical => (super::HEIGHT, super::HEIGHT, -1, -1),
                _ => (0, 0, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct PigouneStackedBar(ObjectSubclass<imp::PigouneStackedBar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneStackedBar {
    pub fn set_parts(&self, parts: Vec<(f32, gdk::RGBA)>) {
        self.imp().parts.replace(parts);
        self.queue_draw();
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "widget sides are small enough to be exact as floats"
    )]
    fn draw(&self, snapshot: &gtk::Snapshot) {
        let (width, height) = (self.width() as f32, self.height() as f32);
        let bounds = graphene::Rect::new(0.0, 0.0, width, height);
        let corner = graphene::Size::new(height / 2.0, height / 2.0);
        snapshot.push_rounded_clip(&gsk::RoundedRect::new(
            bounds, corner, corner, corner, corner,
        ));
        let parts = self.imp().parts.borrow();
        let fractions: Vec<f32> = parts.iter().map(|(fraction, _)| *fraction).collect();
        let mut left = 0.0;
        for ((_, color), part) in parts.iter().zip(visible_widths(&fractions, width, height)) {
            snapshot.append_color(color, &graphene::Rect::new(left, 0.0, part, height));
            left += part;
        }
        snapshot.pop();
    }
}

fn visible_widths(fractions: &[f32], width: f32, minimum: f32) -> Vec<f32> {
    let is_tiny = |fraction: f32| fraction > 0.0 && fraction * width < minimum;
    let reserved: f32 = fractions
        .iter()
        .filter(|fraction| is_tiny(**fraction))
        .map(|_| minimum)
        .sum();
    let others: f32 = fractions
        .iter()
        .filter(|fraction| !is_tiny(**fraction))
        .sum();
    let scale = if others > 0.0 {
        (width - reserved).max(0.0) / (others * width)
    } else {
        0.0
    };
    fractions
        .iter()
        .map(|fraction| {
            if is_tiny(*fraction) {
                minimum
            } else {
                fraction * width * scale
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::visible_widths;

    #[test]
    fn a_tiny_part_stays_visible_and_the_others_make_room() {
        let widths = visible_widths(&[0.0005, 0.3, 0.6995], 400.0, 14.0);
        assert!((widths[0] - 14.0).abs() < 0.01);
        assert!((widths.iter().sum::<f32>() - 400.0).abs() < 0.01);
        assert!(widths[2] > widths[1]);
    }

    #[test]
    fn large_parts_keep_their_exact_share() {
        let widths = visible_widths(&[0.25, 0.75], 400.0, 14.0);
        assert!((widths[0] - 100.0).abs() < 0.01);
        assert!((widths[1] - 300.0).abs() < 0.01);
    }

    #[test]
    fn an_empty_part_takes_no_room() {
        let widths = visible_widths(&[0.0, 1.0], 400.0, 14.0);
        assert!(widths[0].abs() < 0.01);
        assert!((widths[1] - 400.0).abs() < 0.01);
    }
}
