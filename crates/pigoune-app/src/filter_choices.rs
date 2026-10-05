use std::cell::Cell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::{AssetColor, AssetFilter, AssetFormat};

use crate::{asset_colors, asset_facts};

const COLORS_PER_LINE: u32 = 6;
const CHECKMARK_ICON: &str = "object-select-symbolic";

type ChangedCallback = Rc<dyn Fn()>;

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::{AssetColor, AssetFormat};

    use super::ChangedCallback;

    #[derive(Default)]
    pub struct PigouneFilterChoices {
        pub format_checks: RefCell<Vec<(AssetFormat, gtk::CheckButton)>>,
        pub favorites_check: OnceCell<gtk::CheckButton>,
        pub color_buttons: RefCell<Vec<(AssetColor, gtk::ToggleButton)>>,
        pub color_names: OnceCell<gtk::Label>,
        pub quiet: Cell<bool>,
        pub on_changed: RefCell<Option<ChangedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneFilterChoices {
        const NAME: &'static str = "PigouneFilterChoices";
        type Type = super::PigouneFilterChoices;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneFilterChoices {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigouneFilterChoices {}
    impl BoxImpl for PigouneFilterChoices {}
}

glib::wrapper! {
    pub struct PigouneFilterChoices(ObjectSubclass<imp::PigouneFilterChoices>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneFilterChoices {
    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.imp().on_changed.replace(Some(Rc::new(callback)));
    }

    #[must_use]
    pub fn chosen(&self, text: &str) -> AssetFilter {
        let imp = self.imp();
        AssetFilter {
            text: text.to_owned(),
            formats: imp
                .format_checks
                .borrow()
                .iter()
                .filter(|(_, check)| check.is_active())
                .map(|(format, _)| *format)
                .collect(),
            favorites_only: self.favorites_check().is_active(),
            colors: self.chosen_colors(),
        }
    }

    fn chosen_colors(&self) -> Vec<AssetColor> {
        self.imp()
            .color_buttons
            .borrow()
            .iter()
            .filter(|(_, button)| button.is_active())
            .map(|(color, _)| *color)
            .collect()
    }

    pub fn choose(&self, filter: &AssetFilter) {
        self.quietly(|| {
            for (format, check) in self.imp().format_checks.borrow().iter() {
                check.set_active(filter.formats.contains(format));
            }
            self.favorites_check().set_active(filter.favorites_only);
            for (color, button) in self.imp().color_buttons.borrow().iter() {
                button.set_active(filter.colors.contains(color));
            }
        });
        self.show_color_names();
    }

    pub fn clear(&self) {
        self.choose(&AssetFilter::default());
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(12);
        let heading = gtk::Label::builder()
            .label(gettext("Type"))
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let formats = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(3)
            .max_children_per_line(3)
            .homogeneous(true)
            .build();
        let checks = AssetFormat::ALL
            .into_iter()
            .map(|format| {
                let check = gtk::CheckButton::with_label(asset_facts::format_name(format));
                self.notify_on_toggle(&check);
                formats.append(&check);
                (format, check)
            })
            .collect();
        imp.format_checks.replace(checks);
        let favorites = gtk::CheckButton::with_label(&gettext("Favorites Only"));
        self.notify_on_toggle(&favorites);
        self.append(&heading);
        self.append(&formats);
        self.build_colors();
        self.append(&favorites);
        if imp.favorites_check.set(favorites).is_err() {
            unreachable!("filter choices are built once");
        }
    }

    fn build_colors(&self) {
        let imp = self.imp();
        let heading = gtk::Label::builder()
            .label(gettext("Color"))
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let swatches = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(COLORS_PER_LINE)
            .max_children_per_line(COLORS_PER_LINE)
            .homogeneous(true)
            .row_spacing(6)
            .build();
        let buttons = AssetColor::ALL
            .into_iter()
            .map(|color| {
                let button = self.color_button(color);
                swatches.append(&button);
                (color, button)
            })
            .collect();
        imp.color_buttons.replace(buttons);
        let names = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .visible(false)
            .css_classes(["dim-label", "caption"])
            .build();
        self.append(&heading);
        self.append(&swatches);
        self.append(&names);
        if imp.color_names.set(names).is_err() {
            unreachable!("filter choices are built once");
        }
    }

    fn color_button(&self, color: AssetColor) -> gtk::ToggleButton {
        let name = asset_colors::color_name(color);
        let button = gtk::ToggleButton::builder()
            .child(&gtk::Image::from_icon_name(CHECKMARK_ICON))
            .tooltip_text(&name)
            .halign(gtk::Align::Center)
            .css_classes(["collection-swatch", "resource-swatch"])
            .build();
        button.add_css_class(&asset_colors::swatch_class(color));
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        button.connect_toggled(glib::clone!(
            #[weak(rename_to = choices)]
            self,
            move |_| {
                choices.show_color_names();
                if !choices.imp().quiet.get() {
                    choices.changed();
                }
            }
        ));
        button
    }

    fn show_color_names(&self) {
        let Some(label) = self.imp().color_names.get() else {
            return;
        };
        let names: Vec<String> = self
            .chosen_colors()
            .into_iter()
            .map(asset_colors::color_name)
            .collect();
        label.set_label(&names.join(", "));
        label.set_visible(!names.is_empty());
    }

    fn favorites_check(&self) -> gtk::CheckButton {
        self.imp()
            .favorites_check
            .get()
            .cloned()
            .expect("filter choices are built at construction")
    }

    fn notify_on_toggle(&self, check: &gtk::CheckButton) {
        check.connect_toggled(glib::clone!(
            #[weak(rename_to = choices)]
            self,
            move |_| {
                if !choices.imp().quiet.get() {
                    choices.changed();
                }
            }
        ));
    }

    fn quietly(&self, change: impl FnOnce()) {
        let quiet: &Cell<bool> = &self.imp().quiet;
        quiet.set(true);
        change();
        quiet.set(false);
    }

    fn changed(&self) {
        let callback = self.imp().on_changed.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}

impl Default for PigouneFilterChoices {
    fn default() -> Self {
        glib::Object::new()
    }
}
