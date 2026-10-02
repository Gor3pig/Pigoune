use std::path::PathBuf;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetView, CollectionId};

use crate::sidebar::PigouneSidebar;
use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry};

const DROP_HIGHLIGHT: &str = "drop-highlight";
const SIDEBAR_ROW: &str = "sidebar-row";

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
            AssetView::All | AssetView::Favorites | AssetView::Unclassified | AssetView::Tag(_) => {
                None
            }
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
        self.accept_dropped_files();
    }

    fn accept_dropped_files(&self) {
        self.add_css_class(SIDEBAR_ROW);
        let drop_target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop_target.connect_enter(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, _| {
                row.highlight_list_row(true);
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_leave(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |_| row.highlight_list_row(false)
        ));
        drop_target.connect_accept(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            false,
            move |_, _| row.view().is_some()
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                row.highlight_list_row(false);
                let (Some(view), Ok(files)) = (row.view(), value.get::<gdk::FileList>()) else {
                    return false;
                };
                let paths: Vec<PathBuf> =
                    files.files().iter().filter_map(gio::File::path).collect();
                let Some(sidebar) = row
                    .ancestor(PigouneSidebar::static_type())
                    .and_downcast::<PigouneSidebar>()
                else {
                    return false;
                };
                sidebar.files_dropped(view, paths);
                true
            }
        ));
        self.add_controller(drop_target);
    }

    fn highlight_list_row(&self, highlighted: bool) {
        let Some(list_row) = self
            .ancestor(gtk::TreeExpander::static_type())
            .and_then(|expander| expander.parent())
        else {
            return;
        };
        if highlighted {
            list_row.add_css_class(DROP_HIGHLIGHT);
        } else {
            list_row.remove_css_class(DROP_HIGHLIGHT);
        }
    }

    fn view(&self) -> Option<AssetView> {
        self.imp().item.borrow().as_ref()?.view()
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
