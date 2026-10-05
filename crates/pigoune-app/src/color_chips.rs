use std::cell::OnceCell;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::{AssetColor, DominantColor};

use crate::asset_colors;

const PILL_SPACING: i32 = 4;

mod imp {
    use super::OnceCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default)]
    pub struct PigouneColorChips {
        pub pills: OnceCell<adw::WrapBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneColorChips {
        const NAME: &'static str = "PigouneColorChips";
        type Type = super::PigouneColorChips;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneColorChips {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigouneColorChips {}
    impl BoxImpl for PigouneColorChips {}
}

glib::wrapper! {
    pub struct PigouneColorChips(ObjectSubclass<imp::PigouneColorChips>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneColorChips {
    pub fn show_colors(&self, colors: &[DominantColor]) -> bool {
        let pills = self.pills();
        pills.remove_all();
        for color in colors {
            pills.append(&Self::pill(color.family));
        }
        !colors.is_empty()
    }

    fn pill(color: AssetColor) -> gtk::Box {
        let dot = gtk::Box::builder()
            .css_classes(["color-dot", &format!("color-dot-{}", color.code())])
            .valign(gtk::Align::Center)
            .build();
        let pill = gtk::Box::builder()
            .spacing(6)
            .css_classes(["tag-pill", "color-chip"])
            .build();
        pill.append(&dot);
        pill.append(&gtk::Label::new(Some(&asset_colors::color_name(color))));
        pill
    }

    fn pills(&self) -> adw::WrapBox {
        self.imp()
            .pills
            .get()
            .cloned()
            .expect("the color chips build their parts at construction")
    }

    fn build(&self) {
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);
        let heading = gtk::Label::builder()
            .label(gettext("Detected Colors"))
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build();
        let pills = adw::WrapBox::builder()
            .child_spacing(PILL_SPACING)
            .line_spacing(PILL_SPACING)
            .build();
        self.append(&heading);
        self.append(&pills);
        if self.imp().pills.set(pills).is_err() {
            unreachable!("the color chips build their parts once");
        }
    }
}

impl Default for PigouneColorChips {
    fn default() -> Self {
        glib::Object::new()
    }
}
