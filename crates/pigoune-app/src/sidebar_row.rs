use std::path::PathBuf;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetView, CollectionId};

use crate::collection_drop::{self, CollectionDrop, DropZone};
use crate::drag_content::{DraggedAssets, DraggedCollection};
use crate::sidebar::PigouneSidebar;
use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry};

const DROP_HIGHLIGHT: &str = "drop-highlight";
const DROP_BEFORE: &str = "drop-before";
const DROP_AFTER: &str = "drop-after";
const SIDEBAR_ROW: &str = "sidebar-row";

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use crate::collection_drop::DropZone;
    use crate::sidebar_item::PigouneSidebarItem;

    #[derive(Default)]
    pub struct PigouneSidebarRow {
        pub icon: OnceCell<gtk::Image>,
        pub label: OnceCell<gtk::Label>,
        pub count: OnceCell<gtk::Label>,
        pub header_buttons: OnceCell<gtk::Box>,
        pub hash: OnceCell<gtk::Label>,
        pub menu: OnceCell<gtk::PopoverMenu>,
        pub item: RefCell<Option<PigouneSidebarItem>>,
        pub drop_zone: Cell<DropZone>,
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
        let is_header = item.is_header();
        let is_tag = matches!(item.view(), Some(AssetView::Tag(_)));
        let icon = part(&imp.icon);
        icon.set_icon_name(Some(item.icon_name()));
        icon.set_visible(!is_header && !is_tag);
        part(&imp.hash).set_visible(is_tag);
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
        part(&imp.header_buttons).set_visible(item.entry() == SidebarEntry::CollectionsHeader);
        imp.item.replace(Some(item.clone()));
    }

    fn build(&self) {
        let imp = self.imp();
        let icon = gtk::Image::new();
        let hash = gtk::Label::builder()
            .label("#")
            .width_chars(2)
            .css_classes(["dim-label"])
            .build();
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
        self.append(&hash);
        self.append(&label);
        self.append(&count);
        self.append(&header_buttons);
        self.append(&menu);
        set_part(&imp.icon, icon);
        set_part(&imp.hash, hash);
        set_part(&imp.label, label);
        set_part(&imp.count, count);
        set_part(&imp.header_buttons, header_buttons);
        set_part(&imp.menu, menu);
        self.open_menu_on_secondary_click();
        self.accept_drops();
        self.offer_collection_drag();
    }

    fn accept_drops(&self) {
        self.add_css_class(SIDEBAR_ROW);
        let drop_target = gtk::DropTarget::new(glib::Type::INVALID, gdk::DragAction::COPY);
        drop_target.set_types(&[
            gdk::FileList::static_type(),
            DraggedAssets::static_type(),
            DraggedCollection::static_type(),
        ]);
        drop_target.connect_enter(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |target, _, y| {
                row.follow_pointer(target, y);
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_motion(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |target, _, y| {
                row.follow_pointer(target, y);
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_leave(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |_| row.show_drop_zone(None)
        ));
        drop_target.connect_accept(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            false,
            move |target, drop| target
                .formats()
                .is_some_and(|wanted| row.accepts(&wanted, &drop.formats()))
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                row.show_drop_zone(None);
                row.receive(value)
            }
        ));
        self.add_controller(drop_target);
    }

    fn follow_pointer(&self, target: &gtk::DropTarget, y: f64) {
        let carries_collection = target.current_drop().is_some_and(|drop| {
            drop.formats()
                .contains_type(DraggedCollection::static_type())
        });
        let zone = if carries_collection && self.view().is_some() {
            collection_drop::zone_at(y, f64::from(self.height()))
        } else {
            DropZone::Into
        };
        self.imp().drop_zone.set(zone);
        self.show_drop_zone(Some(zone));
    }

    fn accepts(&self, wanted: &gdk::ContentFormats, offered: &gdk::ContentFormats) -> bool {
        if !wanted.match_(offered) {
            return false;
        }
        let entry = self
            .imp()
            .item
            .borrow()
            .as_ref()
            .map(PigouneSidebarItem::entry);
        if offered.contains_type(DraggedCollection::static_type()) {
            return matches!(
                entry,
                Some(
                    SidebarEntry::CollectionsHeader | SidebarEntry::View(AssetView::Collection(_))
                )
            );
        }
        match entry {
            Some(SidebarEntry::View(AssetView::Collection(_) | AssetView::Tag(_))) => true,
            Some(SidebarEntry::View(_)) => !offered.contains_type(DraggedAssets::static_type()),
            Some(SidebarEntry::CollectionsHeader | SidebarEntry::TagsHeader) | None => false,
        }
    }

    fn receive(&self, value: &glib::Value) -> bool {
        let Some(sidebar) = self
            .ancestor(PigouneSidebar::static_type())
            .and_downcast::<PigouneSidebar>()
        else {
            return false;
        };
        if let Ok(dragged) = value.get::<DraggedCollection>() {
            let Some(drop) = self.collection_drop() else {
                return false;
            };
            sidebar.collection_dropped(dragged.0, drop);
            return true;
        }
        let Some(view) = self.view() else {
            return false;
        };
        if let Ok(dragged) = value.get::<DraggedAssets>() {
            sidebar.assets_dropped(view, dragged.0, control_is_held(self));
            return true;
        }
        let Ok(files) = value.get::<gdk::FileList>() else {
            return false;
        };
        let paths: Vec<PathBuf> = files.files().iter().filter_map(gio::File::path).collect();
        sidebar.files_dropped(view, paths);
        true
    }

    fn collection_drop(&self) -> Option<CollectionDrop> {
        match self.view() {
            Some(AssetView::Collection(id)) => Some(match self.imp().drop_zone.get() {
                DropZone::Before => CollectionDrop::Before(id),
                DropZone::Into => CollectionDrop::Into(Some(id)),
                DropZone::After => CollectionDrop::After(id),
            }),
            Some(_) => None,
            None => Some(CollectionDrop::Into(None)),
        }
    }

    fn show_drop_zone(&self, zone: Option<DropZone>) {
        let Some(list_row) = self
            .ancestor(gtk::TreeExpander::static_type())
            .and_then(|expander| expander.parent())
        else {
            return;
        };
        for (class, shown) in [
            (DROP_BEFORE, zone == Some(DropZone::Before)),
            (DROP_HIGHLIGHT, zone == Some(DropZone::Into)),
            (DROP_AFTER, zone == Some(DropZone::After)),
        ] {
            if shown {
                list_row.add_css_class(class);
            } else {
                list_row.remove_css_class(class);
            }
        }
    }

    fn offer_collection_drag(&self) {
        let source = gtk::DragSource::builder()
            .actions(gdk::DragAction::COPY)
            .build();
        source.connect_prepare(glib::clone!(
            #[weak(rename_to = row)]
            self,
            #[upgrade_or]
            None,
            move |_, _, _| match row.view()? {
                AssetView::Collection(id) => Some(gdk::ContentProvider::for_value(
                    &DraggedCollection(id).to_value(),
                )),
                _ => None,
            }
        ));
        source.connect_drag_begin(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |source, _| {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&row))), 0, 0);
            }
        ));
        self.add_controller(source);
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
        let model = match self.view() {
            Some(AssetView::Collection(id)) => collection_menu(id),
            Some(AssetView::Tag(id)) => tag_menu(&id.to_string()),
            _ => return false,
        };
        let menu = part(&self.imp().menu);
        menu.set_menu_model(Some(&model));
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

fn tag_menu(tag: &str) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("Rename…")),
        Some(&format!("win.rename-tag::{tag}")),
    );
    menu.append(
        Some(&gettext("Delete…")),
        Some(&format!("win.delete-tag::{tag}")),
    );
    menu
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

fn control_is_held(widget: &impl IsA<gtk::Widget>) -> bool {
    widget
        .display()
        .default_seat()
        .and_then(|seat| seat.keyboard())
        .is_some_and(|keyboard| {
            keyboard
                .modifier_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
        })
}
