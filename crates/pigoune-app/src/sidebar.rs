use std::collections::HashSet;
use std::path::PathBuf;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetId, AssetView, CollectionId, Tag, ViewCounts};

use crate::collection_drop::CollectionDrop;
use crate::collection_sort::CollectionTree;
use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry, SidebarItemData};
use crate::sidebar_row::PigouneSidebarRow;

const ALL_ICON: &str = "view-grid-symbolic";
const UNCLASSIFIED_ICON: &str = "image-x-generic-symbolic";
const FAVORITES_ICON: &str = "starred-symbolic";
const COLLECTION_ICON: &str = "folder-symbolic";
const TRASH_ICON: &str = "user-trash-symbolic";

type ViewChangedCallback = Box<dyn Fn(AssetView)>;
type FilesDroppedCallback = Box<dyn Fn(AssetView, Vec<PathBuf>)>;
type AssetsDroppedCallback = Box<dyn Fn(AssetView, Vec<AssetId>, bool)>;
type CollectionDroppedCallback = Box<dyn Fn(CollectionId, CollectionDrop)>;
type AssetsHoveredCallback = Box<dyn Fn(Option<HoveredDrop>)>;

pub struct HoveredDrop {
    pub view: AssetView,
    pub assets: Vec<AssetId>,
    pub keep_source: bool,
}

pub struct SidebarContent {
    pub tree: CollectionTree,
    pub counts: ViewCounts,
    pub show_counts: bool,
    pub selected: AssetView,
    pub reveal: Vec<CollectionId>,
    pub tags: Vec<Tag>,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{
        AssetsDroppedCallback, AssetsHoveredCallback, CollectionDroppedCallback,
        FilesDroppedCallback, ViewChangedCallback,
    };

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/sidebar.ui")]
    pub struct PigouneSidebar {
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        pub rebuilding: Cell<bool>,
        pub on_view_changed: RefCell<Option<ViewChangedCallback>>,
        pub on_files_dropped: RefCell<Option<FilesDroppedCallback>>,
        pub on_assets_dropped: RefCell<Option<AssetsDroppedCallback>>,
        pub on_collection_dropped: RefCell<Option<CollectionDroppedCallback>>,
        pub on_assets_hovered: RefCell<Option<AssetsHoveredCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSidebar {
        const NAME: &'static str = "PigouneSidebar";
        type Type = super::PigouneSidebar;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneSidebar {
        fn constructed(&self) {
            self.parent_constructed();
            let sidebar = self.obj();
            sidebar.set_up_rows();
            sidebar.act_on_keys();
        }
    }

    impl WidgetImpl for PigouneSidebar {}
    impl BinImpl for PigouneSidebar {}
}

glib::wrapper! {
    pub struct PigouneSidebar(ObjectSubclass<imp::PigouneSidebar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneSidebar {
    pub fn connect_view_changed(&self, callback: impl Fn(AssetView) + 'static) {
        self.imp().on_view_changed.replace(Some(Box::new(callback)));
    }

    pub fn connect_files_dropped(&self, callback: impl Fn(AssetView, Vec<PathBuf>) + 'static) {
        self.imp()
            .on_files_dropped
            .replace(Some(Box::new(callback)));
    }

    pub fn files_dropped(&self, view: AssetView, paths: Vec<PathBuf>) {
        if let Some(on_files_dropped) = self.imp().on_files_dropped.borrow().as_ref() {
            on_files_dropped(view, paths);
        }
    }

    pub fn connect_assets_dropped(
        &self,
        callback: impl Fn(AssetView, Vec<AssetId>, bool) + 'static,
    ) {
        self.imp()
            .on_assets_dropped
            .replace(Some(Box::new(callback)));
    }

    pub fn assets_dropped(&self, view: AssetView, assets: Vec<AssetId>, keep_source: bool) {
        if let Some(on_assets_dropped) = self.imp().on_assets_dropped.borrow().as_ref() {
            on_assets_dropped(view, assets, keep_source);
        }
    }

    pub fn connect_assets_hovered(&self, callback: impl Fn(Option<HoveredDrop>) + 'static) {
        self.imp()
            .on_assets_hovered
            .replace(Some(Box::new(callback)));
    }

    pub fn assets_hovered(&self, hovered: Option<HoveredDrop>) {
        if let Some(on_assets_hovered) = self.imp().on_assets_hovered.borrow().as_ref() {
            on_assets_hovered(hovered);
        }
    }

    pub fn connect_collection_dropped(
        &self,
        callback: impl Fn(CollectionId, CollectionDrop) + 'static,
    ) {
        self.imp()
            .on_collection_dropped
            .replace(Some(Box::new(callback)));
    }

    pub fn collection_dropped(&self, dragged: CollectionId, drop: CollectionDrop) {
        if let Some(on_collection_dropped) = self.imp().on_collection_dropped.borrow().as_ref() {
            on_collection_dropped(dragged, drop);
        }
    }

    pub fn show_content(&self, content: &SidebarContent) {
        let imp = self.imp();
        let mut expanded = self.expanded_collections();
        expanded.extend(content.reveal.iter().copied());
        let counts = content.show_counts.then_some(&content.counts);

        let root = gio::ListStore::new::<PigouneSidebarItem>();
        root.append(&view_item(
            AssetView::All,
            gettext("All"),
            ALL_ICON,
            counts,
            None,
        ));
        root.append(&view_item(
            AssetView::Favorites,
            gettext("Favorites"),
            FAVORITES_ICON,
            counts,
            None,
        ));
        root.append(&view_item(
            AssetView::Unclassified,
            gettext("Unclassified"),
            UNCLASSIFIED_ICON,
            counts,
            None,
        ));
        root.append(&view_item(
            AssetView::Trash,
            gettext("Trash"),
            TRASH_ICON,
            counts,
            None,
        ));
        root.append(&PigouneSidebarItem::new(SidebarItemData {
            entry: SidebarEntry::CollectionsHeader,
            label: gettext("Collections"),
            icon_name: "",
            count: None,
            children: None,
        }));
        for item in collection_items(&content.tree, None, counts) {
            root.append(&item);
        }
        if !content.tags.is_empty() {
            root.append(&PigouneSidebarItem::new(SidebarItemData {
                entry: SidebarEntry::TagsHeader,
                label: gettext("Tags"),
                icon_name: "",
                count: None,
                children: None,
            }));
            for tag in &content.tags {
                root.append(&view_item(
                    AssetView::Tag(tag.id),
                    tag.name.clone(),
                    "",
                    counts,
                    None,
                ));
            }
        }

        let tree_model = gtk::TreeListModel::new(root, false, false, |item| {
            item.downcast_ref::<PigouneSidebarItem>()
                .and_then(PigouneSidebarItem::children)
                .map(Cast::upcast)
        });
        let selection = gtk::SingleSelection::new(Some(tree_model.clone()));
        selection.set_autoselect(false);
        selection.connect_selected_item_notify(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            move |selection| sidebar.announce_selection(selection)
        ));

        imp.rebuilding.set(true);
        imp.list_view.set_model(Some(&selection));
        expand(&tree_model, &expanded);
        let position = position_of(&tree_model, content.selected).unwrap_or(0);
        selection.set_selected(position);
        imp.rebuilding.set(false);
    }

    fn announce_selection(&self, selection: &gtk::SingleSelection) {
        let imp = self.imp();
        if imp.rebuilding.get() {
            return;
        }
        let Some(view) = item_at(selection.selected_item()).and_then(|item| item.view()) else {
            return;
        };
        if let Some(on_view_changed) = imp.on_view_changed.borrow().as_ref() {
            on_view_changed(view);
        }
    }

    fn act_on_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let (action, id) = match (key, sidebar.selected_view()) {
                    (gdk::Key::F2, Some(AssetView::Collection(id))) => {
                        ("win.rename-collection", id.to_string())
                    }
                    (gdk::Key::F2, Some(AssetView::Tag(id))) => ("win.rename-tag", id.to_string()),
                    (gdk::Key::Delete, Some(AssetView::Collection(id))) => {
                        ("win.delete-collection", id.to_string())
                    }
                    _ => return glib::Propagation::Proceed,
                };
                let _ = sidebar.activate_action(action, Some(&id.to_variant()));
                glib::Propagation::Stop
            }
        ));
        self.imp().list_view.add_controller(keys);
    }

    fn selected_view(&self) -> Option<AssetView> {
        let selection = self
            .imp()
            .list_view
            .model()
            .and_downcast::<gtk::SingleSelection>()?;
        item_at(selection.selected_item())?.view()
    }

    fn expanded_collections(&self) -> HashSet<CollectionId> {
        let Some(tree_model) = self.tree_model() else {
            return HashSet::new();
        };
        (0..tree_model.n_items())
            .filter_map(|position| tree_model.row(position))
            .filter(gtk::TreeListRow::is_expanded)
            .filter_map(|row| collection_of(&row))
            .collect()
    }

    fn tree_model(&self) -> Option<gtk::TreeListModel> {
        self.imp()
            .list_view
            .model()
            .and_downcast::<gtk::SingleSelection>()?
            .model()
            .and_downcast::<gtk::TreeListModel>()
    }

    fn set_up_rows(&self) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let Some(list_item) = list_item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let expander = gtk::TreeExpander::new();
            expander.set_child(Some(&PigouneSidebarRow::new()));
            list_item.set_child(Some(&expander));
        });
        factory.connect_bind(|_, list_item| {
            let Some(list_item) = list_item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(row) = list_item.item().and_downcast::<gtk::TreeListRow>() else {
                return;
            };
            let Some(item) = row.item().and_downcast::<PigouneSidebarItem>() else {
                return;
            };
            let Some(expander) = list_item.child().and_downcast::<gtk::TreeExpander>() else {
                return;
            };
            let is_header = item.is_header();
            list_item.set_selectable(!is_header);
            list_item.set_activatable(!is_header);
            expander.set_list_row(Some(&row));
            expander.set_indent_for_icon(collection_of(&row).is_some());
            if let Some(row) = expander.child().and_downcast::<PigouneSidebarRow>() {
                row.show(&item);
            }
        });
        self.imp().list_view.set_factory(Some(&factory));
    }
}

impl Default for PigouneSidebar {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn view_item(
    view: AssetView,
    label: String,
    icon_name: &'static str,
    counts: Option<&ViewCounts>,
    children: Option<gio::ListStore>,
) -> PigouneSidebarItem {
    PigouneSidebarItem::new(SidebarItemData {
        entry: SidebarEntry::View(view),
        label,
        icon_name,
        count: counts.map(|counts| counts.of(view)),
        children,
    })
}

fn collection_items(
    tree: &CollectionTree,
    parent: Option<CollectionId>,
    counts: Option<&ViewCounts>,
) -> Vec<PigouneSidebarItem> {
    tree.children_of(parent)
        .iter()
        .map(|collection| {
            let children = collection_items(tree, Some(collection.id), counts);
            let store = (!children.is_empty()).then(|| {
                let store = gio::ListStore::new::<PigouneSidebarItem>();
                for child in &children {
                    store.append(child);
                }
                store
            });
            view_item(
                AssetView::Collection(collection.id),
                collection.name.clone(),
                COLLECTION_ICON,
                counts,
                store,
            )
        })
        .collect()
}

fn item_at(object: Option<glib::Object>) -> Option<PigouneSidebarItem> {
    object
        .and_downcast::<gtk::TreeListRow>()?
        .item()
        .and_downcast::<PigouneSidebarItem>()
}

fn collection_of(row: &gtk::TreeListRow) -> Option<CollectionId> {
    match row.item().and_downcast::<PigouneSidebarItem>()?.view()? {
        AssetView::Collection(id) => Some(id),
        AssetView::All
        | AssetView::Favorites
        | AssetView::Unclassified
        | AssetView::Tag(_)
        | AssetView::Trash => None,
    }
}

fn expand(tree_model: &gtk::TreeListModel, expanded: &HashSet<CollectionId>) {
    let mut position = 0;
    while position < tree_model.n_items() {
        if let Some(row) = tree_model.row(position)
            && collection_of(&row).is_some_and(|id| expanded.contains(&id))
        {
            row.set_expanded(true);
        }
        position += 1;
    }
}

fn position_of(tree_model: &gtk::TreeListModel, view: AssetView) -> Option<u32> {
    (0..tree_model.n_items()).find(|position| {
        tree_model
            .row(*position)
            .and_then(|row| row.item().and_downcast::<PigouneSidebarItem>())
            .and_then(|item| item.view())
            == Some(view)
    })
}
