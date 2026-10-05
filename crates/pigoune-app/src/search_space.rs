use gtk::glib;
use gtk::prelude::*;

use crate::search_width::EntrySizes;

const SPACING: i32 = 6;

mod imp {
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    use crate::search_width;

    #[derive(Default)]
    pub struct PigouneSearchSpace;

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSearchSpace {
        const NAME: &'static str = "PigouneSearchSpace";
        type Type = super::PigouneSearchSpace;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PigouneSearchSpace {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for PigouneSearchSpace {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let space = self.obj();
            let (Some(entry), Some(button)) = (space.entry(), space.button()) else {
                return (0, 0, -1, -1);
            };
            if orientation == gtk::Orientation::Vertical {
                let (entry_minimum, entry_natural, _, _) = entry.measure(orientation, -1);
                let (button_minimum, button_natural, _, _) = button.measure(orientation, -1);
                return (
                    entry_minimum.max(button_minimum),
                    entry_natural.max(button_natural),
                    -1,
                    -1,
                );
            }
            let sizes = super::PigouneSearchSpace::entry_sizes(&entry);
            let (button_minimum, button_natural, _, _) = button.measure(orientation, for_size);
            (
                sizes.minimum + super::SPACING + button_minimum,
                search_width::requested_space(&sizes) + super::SPACING + button_natural,
                -1,
                -1,
            )
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let space = self.obj();
            let (Some(entry), Some(button)) = (space.entry(), space.button()) else {
                return;
            };
            let sizes = super::PigouneSearchSpace::entry_sizes(&entry);
            let (button_minimum, button_natural, _, _) =
                button.measure(gtk::Orientation::Horizontal, -1);
            let entry_width =
                search_width::entry_width(width - super::SPACING - button_natural, &sizes);
            let button_width = (width - entry_width - super::SPACING)
                .min(button_natural)
                .max(button_minimum);
            entry.size_allocate(&gtk::Allocation::new(0, 0, entry_width, height), baseline);
            button.size_allocate(
                &gtk::Allocation::new(entry_width + super::SPACING, 0, button_width, height),
                baseline,
            );
        }
    }
}

glib::wrapper! {
    pub struct PigouneSearchSpace(ObjectSubclass<imp::PigouneSearchSpace>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneSearchSpace {
    fn entry(&self) -> Option<gtk::Widget> {
        self.first_child()
    }

    fn button(&self) -> Option<gtk::Widget> {
        self.first_child()?.next_sibling()
    }

    fn entry_sizes(entry: &gtk::Widget) -> EntrySizes {
        let (minimum, natural, _, _) = entry.measure(gtk::Orientation::Horizontal, -1);
        let char_width = entry
            .pango_context()
            .metrics(None, None)
            .approximate_char_width()
            / gtk::pango::SCALE;
        EntrySizes {
            minimum,
            natural,
            char_width,
        }
    }
}

impl Default for PigouneSearchSpace {
    fn default() -> Self {
        glib::Object::new()
    }
}
