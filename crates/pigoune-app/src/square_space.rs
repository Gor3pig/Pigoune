use gtk::glib;

mod imp {
    use std::cell::Cell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PigouneSquareSpace)]
    pub struct PigouneSquareSpace {
        #[property(get, set = Self::set_smallest_side)]
        pub smallest_side: Cell<i32>,
    }

    impl PigouneSquareSpace {
        fn set_smallest_side(&self, side: i32) {
            if self.smallest_side.replace(side) != side {
                self.obj().queue_resize();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSquareSpace {
        const NAME: &'static str = "PigouneSquareSpace";
        type Type = super::PigouneSquareSpace;
        type ParentType = gtk::Widget;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneSquareSpace {}

    impl WidgetImpl for PigouneSquareSpace {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let side = super::side_for(orientation, for_size, self.smallest_side.get());
            (side, side, -1, -1)
        }
    }
}

glib::wrapper! {
    pub struct PigouneSquareSpace(ObjectSubclass<imp::PigouneSquareSpace>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PigouneSquareSpace {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn side_for(orientation: gtk::Orientation, for_size: i32, smallest_side: i32) -> i32 {
    match orientation {
        gtk::Orientation::Vertical if for_size >= 0 => for_size,
        _ => smallest_side.max(0),
    }
}

#[cfg(test)]
mod tests {
    use super::side_for;

    #[test]
    fn the_height_follows_the_width_it_is_given() {
        assert_eq!(side_for(gtk::Orientation::Vertical, 150, 128), 150);
    }

    #[test]
    fn the_width_asks_for_the_smallest_side() {
        assert_eq!(side_for(gtk::Orientation::Horizontal, 300, 128), 128);
        assert_eq!(side_for(gtk::Orientation::Horizontal, -1, 128), 128);
    }

    #[test]
    fn without_a_width_the_height_is_the_smallest_side() {
        assert_eq!(side_for(gtk::Orientation::Vertical, -1, 128), 128);
    }
}
