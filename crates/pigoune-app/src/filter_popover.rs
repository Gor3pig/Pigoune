use std::cell::Cell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::AssetFormat;

use crate::asset_facts;

type ChangedCallback = Rc<dyn Fn()>;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::AssetFormat;

    use super::ChangedCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/filter-popover.ui")]
    pub struct PigouneFilterPopover {
        #[template_child]
        pub formats_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub favorites_check: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,
        pub format_checks: RefCell<Vec<(AssetFormat, gtk::CheckButton)>>,
        pub quiet: Cell<bool>,
        pub on_changed: RefCell<Option<ChangedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneFilterPopover {
        const NAME: &'static str = "PigouneFilterPopover";
        type Type = super::PigouneFilterPopover;
        type ParentType = gtk::Popover;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneFilterPopover {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_up();
        }
    }

    impl WidgetImpl for PigouneFilterPopover {}
    impl PopoverImpl for PigouneFilterPopover {}
}

glib::wrapper! {
    pub struct PigouneFilterPopover(ObjectSubclass<imp::PigouneFilterPopover>)
        @extends gtk::Popover, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
            gtk::ShortcutManager;
}

pub struct FilterChoice {
    pub formats: Vec<AssetFormat>,
    pub favorites_only: bool,
}

impl PigouneFilterPopover {
    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.imp().on_changed.replace(Some(Rc::new(callback)));
    }

    pub fn choice(&self) -> FilterChoice {
        let imp = self.imp();
        FilterChoice {
            formats: checked(&imp.format_checks.borrow()),
            favorites_only: imp.favorites_check.is_active(),
        }
    }

    pub fn clear(&self) {
        let imp = self.imp();
        self.quietly(|| {
            for (_, check) in imp.format_checks.borrow().iter() {
                check.set_active(false);
            }
            imp.favorites_check.set_active(false);
        });
    }

    fn set_up(&self) {
        let imp = self.imp();
        let checks = AssetFormat::ALL
            .into_iter()
            .map(|format| {
                let check = gtk::CheckButton::with_label(asset_facts::format_name(format));
                self.notify_on_toggle(&check);
                imp.formats_box.append(&check);
                (format, check)
            })
            .collect();
        imp.format_checks.replace(checks);
        self.notify_on_toggle(&imp.favorites_check);
        imp.clear_button.connect_clicked(glib::clone!(
            #[weak(rename_to = popover)]
            self,
            move |_| {
                popover.clear();
                popover.changed();
            }
        ));
    }

    fn notify_on_toggle(&self, check: &gtk::CheckButton) {
        check.connect_toggled(glib::clone!(
            #[weak(rename_to = popover)]
            self,
            move |_| {
                if !popover.imp().quiet.get() {
                    popover.changed();
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

fn checked<T: Copy>(checks: &[(T, gtk::CheckButton)]) -> Vec<T> {
    checks
        .iter()
        .filter(|(_, check)| check.is_active())
        .map(|(value, _)| *value)
        .collect()
}
