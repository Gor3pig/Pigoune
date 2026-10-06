use std::cell::Cell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::{AssetColor, AssetFilter, AssetFormat, AssetShape};

use crate::custom_color_swatch::{self, CustomColorSwatch};
use crate::{asset_colors, asset_facts, screen_size};

const COLORS_PER_LINE: u32 = 7;
const CHECKMARK_ICON: &str = "object-select-symbolic";
const SHAPE_OUTLINE_SPACE: i32 = 24;

type ChangedCallback = Rc<dyn Fn()>;

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::{AssetColor, AssetFormat, AssetShape};

    use super::ChangedCallback;
    use crate::custom_color_swatch::CustomColorSwatch;

    #[derive(Default)]
    pub struct PigouneFilterChoices {
        pub format_checks: RefCell<Vec<(AssetFormat, gtk::CheckButton)>>,
        pub favorites_check: OnceCell<gtk::CheckButton>,
        pub shape_buttons: RefCell<Vec<(AssetShape, gtk::ToggleButton)>>,
        pub fits_screen_check: OnceCell<gtk::CheckButton>,
        pub screen_label: OnceCell<gtk::Label>,
        pub color_buttons: RefCell<Vec<(AssetColor, gtk::ToggleButton)>>,
        pub color_names: OnceCell<gtk::Label>,
        pub custom_swatch: OnceCell<Rc<CustomColorSwatch>>,
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
            custom_color: self.custom_swatch().color(),
            shapes: imp
                .shape_buttons
                .borrow()
                .iter()
                .filter(|(_, button)| button.is_active())
                .map(|(shape, _)| *shape)
                .collect(),
            fits_screen: self.fits_screen_check().is_active(),
            screen: None,
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
            self.custom_swatch().set_color(filter.custom_color);
            for (shape, button) in self.imp().shape_buttons.borrow().iter() {
                button.set_active(filter.shapes.contains(shape));
            }
            self.fits_screen_check().set_active(filter.fits_screen);
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
        self.build_shapes();
        self.build_fits_screen();
        self.build_colors();
        self.append(&favorites);
        if imp.favorites_check.set(favorites).is_err() {
            unreachable!("filter choices are built once");
        }
        self.connect_map(Self::show_screen_size);
    }

    fn build_shapes(&self) {
        let heading = gtk::Label::builder()
            .label(gettext("Shape"))
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let row = gtk::Box::builder().spacing(6).homogeneous(true).build();
        let buttons = AssetShape::ALL
            .into_iter()
            .map(|shape| {
                let button = self.shape_button(shape);
                row.append(&button);
                (shape, button)
            })
            .collect();
        self.imp().shape_buttons.replace(buttons);
        self.append(&heading);
        self.append(&row);
    }

    fn shape_button(&self, shape: AssetShape) -> gtk::ToggleButton {
        let outline = gtk::Box::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["shape-outline", shape.code()])
            .build();
        let frame = gtk::Box::builder()
            .height_request(SHAPE_OUTLINE_SPACE)
            .halign(gtk::Align::Center)
            .build();
        frame.append(&outline);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        content.append(&frame);
        content.append(&gtk::Label::new(Some(&asset_facts::shape_name(shape))));
        let button = gtk::ToggleButton::builder()
            .child(&content)
            .css_classes(["shape-choice"])
            .build();
        button.connect_toggled(glib::clone!(
            #[weak(rename_to = choices)]
            self,
            move |_| {
                if !choices.imp().quiet.get() {
                    choices.changed();
                }
            }
        ));
        button
    }

    fn build_fits_screen(&self) {
        let imp = self.imp();
        let size = gtk::Label::builder()
            .xalign(0.0)
            .visible(false)
            .css_classes(["dim-label", "caption"])
            .build();
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        text.append(
            &gtk::Label::builder()
                .label(gettext("Fits My Screen"))
                .xalign(0.0)
                .build(),
        );
        text.append(&size);
        let check = gtk::CheckButton::builder().child(&text).build();
        self.notify_on_toggle(&check);
        self.append(&check);
        if imp.fits_screen_check.set(check).is_err() || imp.screen_label.set(size).is_err() {
            unreachable!("filter choices are built once");
        }
    }

    fn show_screen_size(&self) {
        let Some(label) = self.imp().screen_label.get() else {
            return;
        };
        let screen = screen_size::screen_of(self);
        label.set_label(&match screen {
            Some(screen) => screen_size::at_least(screen),
            None => gettext("The size of your screen could not be detected"),
        });
        label.set_visible(true);
        let check = self.fits_screen_check();
        if screen.is_none() {
            check.set_active(false);
        }
        check.set_sensitive(screen.is_some());
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
        let custom = CustomColorSwatch::new();
        swatches.append(custom.button());
        custom.connect_changed(glib::clone!(
            #[weak(rename_to = choices)]
            self,
            move || {
                choices.show_color_names();
                choices.changed();
            }
        ));
        if imp.custom_swatch.set(custom).is_err() {
            unreachable!("filter choices are built once");
        }
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
        let mut names: Vec<String> = self
            .chosen_colors()
            .into_iter()
            .map(asset_colors::color_name)
            .collect();
        if let Some(color) = self.custom_swatch().color() {
            names.push(
                gettext("Custom {code}").replace("{code}", &custom_color_swatch::code_of(color)),
            );
        }
        label.set_label(&names.join(", "));
        label.set_visible(!names.is_empty());
    }

    fn custom_swatch(&self) -> Rc<CustomColorSwatch> {
        self.imp()
            .custom_swatch
            .get()
            .cloned()
            .expect("filter choices are built at construction")
    }

    fn fits_screen_check(&self) -> gtk::CheckButton {
        self.imp()
            .fits_screen_check
            .get()
            .cloned()
            .expect("filter choices are built at construction")
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
