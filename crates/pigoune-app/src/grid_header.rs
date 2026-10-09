use adw::subclass::prelude::*;
use gettextrs::gettext;

use crate::filter_popover::PigouneFilterPopover;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const HERE_ICON: &str = "folder-symbolic";
const EVERYWHERE_ICON: &str = "web-browser-symbolic";
pub struct SearchScope {
    pub label: String,
    pub wide: bool,
    pub menu_name: Option<String>,
    pub detail: Option<String>,
}

const CHIP_START: i32 = 30;
const TEXT_GAP: i32 = 6;

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
        pub scope_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub scope_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub search_slot: TemplateChild<gtk::Box>,
        #[template_child]
        pub search_space: TemplateChild<PigouneSearchSpace>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_bar_slot: TemplateChild<gtk::Box>,
        #[template_child]
        pub search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub scope_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub scope_arrow: TemplateChild<gtk::Image>,
        #[template_child]
        pub filter_popover: TemplateChild<PigouneFilterPopover>,

        pub scope_choosable: Cell<bool>,

        #[property(get, set)]
        pub compact: Cell<bool>,
        #[property(get, set)]
        pub narrow: Cell<bool>,
        #[property(get, set)]
        pub tight: Cell<bool>,
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
    impl ObjectImpl for PigouneGridHeader {
        fn constructed(&self) {
            self.parent_constructed();
            if let Some(overlay) = self.scope_button.parent().and_downcast::<gtk::Overlay>() {
                overlay.set_measure_overlay(&*self.scope_button, false);
            }
            self.search_bar.connect_entry(&*self.search_entry);
            self.search_button.connect_toggled(glib::clone!(
                #[weak(rename_to = bar)]
                self.search_bar,
                move |button| bar.set_search_mode(button.is_active())
            ));
            self.search_bar
                .connect_search_mode_enabled_notify(glib::clone!(
                    #[weak(rename_to = header)]
                    self.obj(),
                    move |bar| {
                        header.imp().search_button.set_active(bar.is_search_mode());
                        if !bar.is_search_mode() && header.tight() {
                            header.imp().search_entry.set_text("");
                        }
                    }
                ));
            self.obj()
                .connect_tight_notify(super::PigouneGridHeader::place_search);
        }
    }
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

    pub fn show_search_scope(&self, scope: &SearchScope) {
        let imp = self.imp();
        imp.scope_label.set_label(&scope.label);
        imp.scope_icon.set_icon_name(Some(if scope.wide {
            EVERYWHERE_ICON
        } else {
            HERE_ICON
        }));
        if scope.wide {
            imp.scope_button.remove_css_class("here");
        } else {
            imp.scope_button.add_css_class("here");
        }
        imp.scope_button.set_tooltip_text(scope.detail.as_deref());
        imp.scope_button
            .update_property(&[gtk::accessible::Property::Description(
                scope.detail.as_deref().unwrap_or(&scope.label),
            )]);
        imp.scope_choosable.set(scope.menu_name.is_some());
        imp.scope_button.set_can_target(scope.menu_name.is_some());
        imp.scope_button.set_focusable(scope.menu_name.is_some());
        let menu = scope
            .menu_name
            .as_deref()
            .map_or_else(gio::Menu::new, scope_menu);
        imp.scope_button.set_menu_model(Some(&menu));
        imp.scope_button.set_visible(true);
        self.update_scope_arrow();
        self.align_search_text();
    }

    pub fn focus_search(&self) {
        let imp = self.imp();
        if self.tight() {
            imp.search_button.set_active(true);
        }
        imp.search_entry.grab_focus();
    }

    fn place_search(&self) {
        let imp = self.imp();
        let space = imp.search_space.get();
        let (from, to) = if self.tight() {
            (&imp.search_slot, &imp.search_bar_slot)
        } else {
            (&imp.search_bar_slot, &imp.search_slot)
        };
        if space.parent().as_ref() == Some(from.upcast_ref()) {
            from.remove(&space);
            to.append(&space);
        }
        if self.tight() {
            imp.search_button
                .set_active(!imp.search_entry.text().is_empty());
        } else {
            imp.search_button.set_active(false);
        }
    }

    fn update_scope_arrow(&self) {
        let imp = self.imp();
        imp.scope_arrow.set_visible(imp.scope_choosable.get());
    }

    fn align_search_text(&self) {
        self.update_scope_arrow();
        let imp = self.imp();
        let offset = if imp.scope_button.is_visible() {
            let (_, natural, _, _) = imp.scope_button.measure(gtk::Orientation::Horizontal, -1);
            natural - CHIP_START + TEXT_GAP
        } else {
            0
        };
        if let Some(text) = imp
            .search_entry
            .delegate()
            .and_then(|editable| editable.downcast::<gtk::Text>().ok())
        {
            text.set_margin_start(offset);
        }
    }
}

fn scope_menu(name: &str) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("In “{name}”").replace("{name}", name)),
        Some("win.search-scope::here"),
    );
    menu.append(
        Some(&gettext("In the Whole Library")),
        Some("win.search-scope::everywhere"),
    );
    menu
}
