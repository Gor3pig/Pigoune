use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{AssetView, TagId};

use crate::sidebar_tag_cloud::TagPill;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarEntry {
    View(AssetView),
    CollectionsHeader,
    SmartCollectionsHeader,
    TagsHeader,
    TagCloud,
}

pub struct SidebarItemData {
    pub entry: SidebarEntry,
    pub label: String,
    pub icon_name: &'static str,
    pub color_class: Option<&'static str>,
    pub count: Option<usize>,
    pub folded: bool,
    pub tag_pills: Vec<TagPill>,
    pub selected_tag: Option<TagId>,
    pub children: Option<gio::ListStore>,
}

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::SidebarItemData;

    #[derive(Default)]
    pub struct PigouneSidebarItem {
        pub data: OnceCell<SidebarItemData>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSidebarItem {
        const NAME: &'static str = "PigouneSidebarItem";
        type Type = super::PigouneSidebarItem;
    }

    impl ObjectImpl for PigouneSidebarItem {}
}

glib::wrapper! {
    pub struct PigouneSidebarItem(ObjectSubclass<imp::PigouneSidebarItem>);
}

impl PigouneSidebarItem {
    pub fn new(data: SidebarItemData) -> Self {
        let item: Self = glib::Object::new();
        if item.imp().data.set(data).is_err() {
            unreachable!("a new sidebar item has no data yet");
        }
        item
    }

    fn data(&self) -> &SidebarItemData {
        self.imp()
            .data
            .get()
            .expect("sidebar items are created with their data")
    }

    pub fn entry(&self) -> SidebarEntry {
        self.data().entry
    }

    pub fn is_header(&self) -> bool {
        matches!(
            self.entry(),
            SidebarEntry::CollectionsHeader
                | SidebarEntry::SmartCollectionsHeader
                | SidebarEntry::TagsHeader
        )
    }

    pub fn view(&self) -> Option<AssetView> {
        match self.entry() {
            SidebarEntry::View(view) => Some(view),
            SidebarEntry::CollectionsHeader
            | SidebarEntry::SmartCollectionsHeader
            | SidebarEntry::TagsHeader
            | SidebarEntry::TagCloud => None,
        }
    }

    pub fn label(&self) -> &str {
        &self.data().label
    }

    pub fn icon_name(&self) -> &'static str {
        self.data().icon_name
    }

    pub fn color_class(&self) -> Option<&'static str> {
        self.data().color_class
    }

    pub fn count(&self) -> Option<usize> {
        self.data().count
    }

    pub fn folded(&self) -> bool {
        self.data().folded
    }

    pub fn is_tag_cloud(&self) -> bool {
        self.entry() == SidebarEntry::TagCloud
    }

    pub fn tag_pills(&self) -> Vec<TagPill> {
        self.data().tag_pills.clone()
    }

    pub fn selected_tag(&self) -> Option<TagId> {
        self.data().selected_tag
    }

    pub fn children(&self) -> Option<gio::ListStore> {
        self.data().children.clone()
    }

    pub fn spoken_label(&self) -> String {
        let name = match self.view() {
            Some(AssetView::Tag(_)) => gettext("Tag {name}").replace("{name}", self.label()),
            Some(AssetView::Smart(_)) => {
                gettext("Smart collection {name}").replace("{name}", self.label())
            }
            _ => self.label().to_owned(),
        };
        let Some(count) = self.count() else {
            return name;
        };
        ngettext(
            "{name}, {count} asset",
            "{name}, {count} assets",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{name}", &name)
        .replace("{count}", &count.to_string())
    }
}
