use adw::subclass::prelude::*;
use gettextrs::gettext;

use crate::filter_popover::PigouneFilterPopover;
use gtk::glib;
use gtk::prelude::*;

mod imp {
    use std::cell::Cell;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use crate::filter_popover::PigouneFilterPopover;
    use crate::search_space::PigouneSearchSpace;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/grid-header.ui")]
    #[properties(wrapper_type = super::PigouneGridHeader)]
    pub struct PigouneGridHeader {
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub size_adjustment: TemplateChild<gtk::Adjustment>,
        #[template_child]
        pub sort_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub details_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub filter_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub filter_popover: TemplateChild<PigouneFilterPopover>,

        #[property(get, set)]
        pub compact: Cell<bool>,
        #[property(get, set)]
        pub narrow: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneGridHeader {
        const NAME: &'static str = "PigouneGridHeader";
        type Type = super::PigouneGridHeader;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            PigouneFilterPopover::ensure_type();
            PigouneSearchSpace::ensure_type();
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneGridHeader {}
    impl WidgetImpl for PigouneGridHeader {}
    impl BinImpl for PigouneGridHeader {}
}

glib::wrapper! {
    pub struct PigouneGridHeader(ObjectSubclass<imp::PigouneGridHeader>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneGridHeader {
    pub fn search_entry(&self) -> gtk::SearchEntry {
        self.imp().search_entry.get()
    }

    pub fn filter_popover(&self) -> PigouneFilterPopover {
        self.imp().filter_popover.get()
    }

    pub fn show_filter_count(&self, count: usize) {
        let button = &self.imp().filter_button;
        if count == 0 {
            button.set_label(&gettext("Filters"));
            button.remove_css_class("accent");
        } else {
            button.set_label(&gettext("Filters ({count})").replace("{count}", &count.to_string()));
            button.add_css_class("accent");
        }
    }

    pub fn sidebar_button(&self) -> gtk::ToggleButton {
        self.imp().sidebar_button.get()
    }

    pub fn details_button(&self) -> gtk::ToggleButton {
        self.imp().details_button.get()
    }

    pub fn show_sort_criterion(&self, criterion: &str) {
        self.imp().sort_button.set_tooltip_text(Some(
            &gettext("Sort: {criterion}").replace("{criterion}", criterion),
        ));
    }

    pub fn size_adjustment(&self) -> gtk::Adjustment {
        self.imp().size_adjustment.get()
    }
}
