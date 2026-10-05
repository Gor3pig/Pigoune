use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::AssetView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarEntry {
    View(AssetView),
    CollectionsHeader,
    SmartCollectionsHeader,
    TagsHeader,
}

pub struct SidebarItemData {
    pub entry: SidebarEntry,
    pub label: String,
    pub icon_name: &'static str,
    pub count: Option<usize>,
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
            | SidebarEntry::TagsHeader => None,
        }
    }

    pub fn label(&self) -> &str {
        &self.data().label
    }

    pub fn icon_name(&self) -> &'static str {
        self.data().icon_name
    }

    pub fn count(&self) -> Option<usize> {
        self.data().count
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
            "{name}, {count} resource",
            "{name}, {count} resources",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{name}", &name)
        .replace("{count}", &count.to_string())
    }
}
