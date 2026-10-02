use std::collections::HashSet;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use pigoune_core::{AssetView, CollectionId, ViewCounts};

use crate::collection_sort::CollectionTree;
use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry, SidebarItemData};

const ALL_ICON: &str = "view-grid-symbolic";
const UNCLASSIFIED_ICON: &str = "image-x-generic-symbolic";
const COLLECTION_ICON: &str = "folder-symbolic";

type ViewChangedCallback = Box<dyn Fn(AssetView)>;

pub struct SidebarContent {
    pub tree: CollectionTree,
    pub counts: ViewCounts,
    pub show_counts: bool,
    pub selected: AssetView,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::ViewChangedCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/sidebar.ui")]
    pub struct PigouneSidebar {
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        pub rebuilding: Cell<bool>,
        pub on_view_changed: RefCell<Option<ViewChangedCallback>>,
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
            self.obj().set_up_rows();
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

    pub fn show_content(&self, content: &SidebarContent) {
        let imp = self.imp();
        let expanded = self.expanded_collections();
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
            AssetView::Unclassified,
            gettext("Unclassified"),
            UNCLASSIFIED_ICON,
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
            list_item.set_child(Some(&SidebarRow::new().expander));
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
            let is_header = item.entry() == SidebarEntry::CollectionsHeader;
            list_item.set_selectable(!is_header);
            list_item.set_activatable(!is_header);
            expander.set_list_row(Some(&row));
            expander.set_indent_for_icon(collection_of(&row).is_some());
            SidebarRow::from_expander(&expander).show(&item);
        });
        self.imp().list_view.set_factory(Some(&factory));
    }
}

impl Default for PigouneSidebar {
    fn default() -> Self {
        glib::Object::new()
    }
}

struct SidebarRow {
    expander: gtk::TreeExpander,
    icon: gtk::Image,
    label: gtk::Label,
    count: gtk::Label,
    sort_button: gtk::MenuButton,
}

impl SidebarRow {
    fn new() -> Self {
        let icon = gtk::Image::new();
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let count = gtk::Label::builder()
            .css_classes(["dim-label", "numeric", "caption"])
            .build();
        let sort_button = gtk::MenuButton::builder()
            .icon_name("view-sort-descending-symbolic")
            .tooltip_text(gettext("Sort Collections"))
            .menu_model(&collection_sort_menu())
            .css_classes(["flat"])
            .build();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.append(&icon);
        content.append(&label);
        content.append(&count);
        content.append(&sort_button);
        let expander = gtk::TreeExpander::new();
        expander.set_child(Some(&content));
        Self {
            expander,
            icon,
            label,
            count,
            sort_button,
        }
    }

    fn from_expander(expander: &gtk::TreeExpander) -> Self {
        let content = expander
            .child()
            .and_downcast::<gtk::Box>()
            .expect("sidebar rows hold a box");
        let mut children = std::iter::successors(content.first_child(), gtk::Widget::next_sibling);
        let mut next = || children.next().expect("sidebar rows have four parts");
        Self {
            expander: expander.clone(),
            icon: next().downcast().expect("icon"),
            label: next().downcast().expect("label"),
            count: next().downcast().expect("count"),
            sort_button: next().downcast().expect("sort button"),
        }
    }

    fn show(&self, item: &PigouneSidebarItem) {
        let is_header = item.entry() == SidebarEntry::CollectionsHeader;
        self.icon.set_icon_name(Some(item.icon_name()));
        self.icon.set_visible(!is_header);
        self.label.set_label(item.label());
        if is_header {
            self.label.set_css_classes(&["heading", "dim-label"]);
        } else {
            self.label.set_css_classes(&[]);
        }
        self.count.set_visible(item.count().is_some());
        self.count.set_label(
            &item
                .count()
                .map(|count| count.to_string())
                .unwrap_or_default(),
        );
        self.sort_button.set_visible(is_header);
    }
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
        AssetView::All | AssetView::Unclassified => None,
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
