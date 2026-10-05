use std::cell::Cell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::{AssetFilter, AssetFormat};

use crate::asset_facts;

type ChangedCallback = Rc<dyn Fn()>;

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::AssetFormat;

    use super::ChangedCallback;

    #[derive(Default)]
    pub struct PigouneFilterChoices {
        pub format_checks: RefCell<Vec<(AssetFormat, gtk::CheckButton)>>,
        pub favorites_check: OnceCell<gtk::CheckButton>,
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
        }
    }

    pub fn choose(&self, filter: &AssetFilter) {
        self.quietly(|| {
            for (format, check) in self.imp().format_checks.borrow().iter() {
                check.set_active(filter.formats.contains(format));
            }
            self.favorites_check().set_active(filter.favorites_only);
        });
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
        self.append(&favorites);
        if imp.favorites_check.set(favorites).is_err() {
            unreachable!("filter choices are built once");
        }
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
