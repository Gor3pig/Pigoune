use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::AssetFilter;

mod imp {
    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use crate::filter_choices::PigouneFilterChoices;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/filter-popover.ui")]
    pub struct PigouneFilterPopover {
        #[template_child]
        pub choices: TemplateChild<PigouneFilterChoices>,
        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneFilterPopover {
        const NAME: &'static str = "PigouneFilterPopover";
        type Type = super::PigouneFilterPopover;
        type ParentType = gtk::Popover;

        fn class_init(class: &mut Self::Class) {
            PigouneFilterChoices::ensure_type();
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneFilterPopover {}
    impl WidgetImpl for PigouneFilterPopover {}
    impl PopoverImpl for PigouneFilterPopover {}
}

glib::wrapper! {
    pub struct PigouneFilterPopover(ObjectSubclass<imp::PigouneFilterPopover>)
        @extends gtk::Popover, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
            gtk::ShortcutManager;
}

impl PigouneFilterPopover {
    pub fn connect_changed(&self, callback: impl Fn() + Clone + 'static) {
        let imp = self.imp();
        imp.choices.connect_changed(callback.clone());
        imp.clear_button.connect_clicked(glib::clone!(
            #[weak(rename_to = popover)]
            self,
            move |_| {
                popover.clear();
                callback();
            }
        ));
    }

    #[must_use]
    pub fn choice(&self) -> AssetFilter {
        self.imp().choices.chosen("")
    }

    pub fn clear(&self) {
        self.imp().choices.clear();
    }
}
