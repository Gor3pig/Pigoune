use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetView, CollectionId};

use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry};

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use crate::sidebar_item::PigouneSidebarItem;

    #[derive(Default)]
    pub struct PigouneSidebarRow {
        pub icon: OnceCell<gtk::Image>,
        pub label: OnceCell<gtk::Label>,
        pub count: OnceCell<gtk::Label>,
        pub header_buttons: OnceCell<gtk::Box>,
        pub menu: OnceCell<gtk::PopoverMenu>,
        pub item: RefCell<Option<PigouneSidebarItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSidebarRow {
        const NAME: &'static str = "PigouneSidebarRow";
        type Type = super::PigouneSidebarRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneSidebarRow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigouneSidebarRow {}
    impl BoxImpl for PigouneSidebarRow {}
}

glib::wrapper! {
    pub struct PigouneSidebarRow(ObjectSubclass<imp::PigouneSidebarRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneSidebarRow {
    pub fn new() -> Self {
        glib::Object::builder().property("spacing", 12).build()
    }

    pub fn show(&self, item: &PigouneSidebarItem) {
        let imp = self.imp();
        let is_header = item.entry() == SidebarEntry::CollectionsHeader;
        let icon = part(&imp.icon);
        icon.set_icon_name(Some(item.icon_name()));
        icon.set_visible(!is_header);
        let label = part(&imp.label);
        label.set_label(item.label());
        if is_header {
            label.set_css_classes(&["heading", "dim-label"]);
        } else {
            label.set_css_classes(&[]);
        }
        let count = part(&imp.count);
        count.set_visible(item.count().is_some());
        count.set_label(
            &item
                .count()
                .map(|count| count.to_string())
                .unwrap_or_default(),
        );
        part(&imp.header_buttons).set_visible(is_header);
        imp.item.replace(Some(item.clone()));
    }

    fn collection(&self) -> Option<CollectionId> {
        match self.imp().item.borrow().as_ref()?.view()? {
            AssetView::Collection(id) => Some(id),
            AssetView::All | AssetView::Unclassified => None,
        }
    }

    fn build(&self) {
        let imp = self.imp();
        let icon = gtk::Image::new();
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let count = gtk::Label::builder()
            .css_classes(["dim-label", "numeric", "caption"])
            .build();
        let header_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header_buttons.append(
            &gtk::MenuButton::builder()
                .icon_name("view-sort-descending-symbolic")
                .tooltip_text(gettext("Sort Collections"))
                .menu_model(&collection_sort_menu())
                .css_classes(["flat"])
                .build(),
        );
        header_buttons.append(
            &gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text(gettext("New Collection"))
                .action_name("win.new-collection")
                .css_classes(["flat"])
                .build(),
        );
        let menu = gtk::PopoverMenu::builder().has_arrow(false).build();
        self.append(&icon);
        self.append(&label);
        self.append(&count);
        self.append(&header_buttons);
        self.append(&menu);
        set_part(&imp.icon, icon);
        set_part(&imp.label, label);
        set_part(&imp.count, count);
        set_part(&imp.header_buttons, header_buttons);
        set_part(&imp.menu, menu);
        self.open_menu_on_secondary_click();
    }

    fn open_menu_on_secondary_click(&self) {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |gesture, _, x, y| {
                if row.show_menu(x, y) {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            }
        ));
        self.add_controller(click);

        let long_press = gtk::GestureLongPress::new();
        long_press.connect_pressed(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |gesture, x, y| {
                if row.show_menu(x, y) {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            }
        ));
        self.add_controller(long_press);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer coordinates inside a sidebar row are small"
    )]
    fn show_menu(&self, x: f64, y: f64) -> bool {
        let Some(collection) = self.collection() else {
            return false;
        };
        let menu = part(&self.imp().menu);
        menu.set_menu_model(Some(&collection_menu(collection)));
        menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        menu.popup();
        true
    }
}

impl Default for PigouneSidebarRow {
    fn default() -> Self {
        Self::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("sidebar rows build their parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("sidebar rows build their parts once");
    }
}

fn collection_menu(collection: CollectionId) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("New Sub-collection…")),
        Some(&format!("win.new-subcollection::{collection}")),
    );
    menu.append(
        Some(&gettext("Rename…")),
        Some(&format!("win.rename-collection::{collection}")),
    );
    menu
}

fn collection_sort_menu() -> gio::Menu {
    let criteria = gio::Menu::new();
    for (label, target) in [
        (gettext("Name"), "name"),
        (gettext("Date Created"), "created"),
        (gettext("Custom Order"), "custom"),
    ] {
        criteria.append(
            Some(&label),
            Some(&format!("win.collection-sort::{target}")),
        );
    }
    let direction = gio::Menu::new();
    direction.append(
        Some(&gettext("Reverse Order")),
        Some("win.collection-sort-reversed"),
    );
    let menu = gio::Menu::new();
    menu.append_section(Some(&gettext("Sort Collections By")), &criteria);
    menu.append_section(None, &direction);
    menu
}
