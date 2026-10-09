use std::collections::HashSet;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{
    AssetId, AssetView, CollectionId, SmartCollection, SmartCollectionId, Tag, TagId, ViewCounts,
};

use crate::asset_grid::MENU_KEYS;
use crate::collection_drop::CollectionDrop;
use crate::collection_looks;
use crate::collection_sort::CollectionTree;
use crate::drop_action::DropOffer;
use crate::dropped_content::Dropped;
use crate::found_flash;
use crate::sidebar_item::{PigouneSidebarItem, SidebarEntry, SidebarItemData};
use crate::sidebar_row::PigouneSidebarRow;
use crate::sidebar_tag_cloud::{PigouneSidebarTagCloud, SharedCloudPlace, TagPill};

const ALL_ICON: &str = "view-grid-symbolic";
const UNCLASSIFIED_ICON: &str = "image-x-generic-symbolic";
const FAVORITES_ICON: &str = "starred-symbolic";
const SMART_COLLECTION_ICON: &str = "media-playlist-shuffle-symbolic";
const TRASH_ICON: &str = "user-trash-symbolic";
const DRAG_OVER: &str = "drag-over";

type ViewChangedCallback = Box<dyn Fn(AssetView)>;
type ContentDroppedCallback = Box<dyn Fn(AssetView, Dropped)>;
type AssetsDroppedCallback = Box<dyn Fn(AssetView, Vec<AssetId>, bool)>;
type CollectionDroppedCallback = Box<dyn Fn(CollectionId, CollectionDrop)>;
type TagDroppedCallback = Box<dyn Fn(TagId, Option<TagId>)>;
type AssetsHoveredCallback = Box<dyn Fn(Option<HoveredDrop>)>;
type DropOfferCallback = Box<dyn Fn(AssetView) -> DropOffer>;
type SmartCollectionDroppedCallback = Box<dyn Fn(SmartCollectionId, SmartCollectionId, bool)>;
type SectionToggledCallback = Box<dyn Fn(SidebarEntry)>;

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
    pub smart_collections: Vec<SmartCollection>,
    pub show_smart_collections: bool,
    pub folded: FoldedSections,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FoldedSections {
    pub collections: bool,
    pub smart_collections: bool,
    pub tags: bool,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{
        AssetsDroppedCallback, AssetsHoveredCallback, CollectionDroppedCallback,
        ContentDroppedCallback, DropOfferCallback, SectionToggledCallback,
        SmartCollectionDroppedCallback, TagDroppedCallback, ViewChangedCallback,
    };

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/sidebar.ui")]
    pub struct PigouneSidebar {
        #[template_child]
        pub list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub trash_view: TemplateChild<gtk::ListView>,
        pub on_section_toggled: RefCell<Option<SectionToggledCallback>>,
        pub refocused_header: Cell<Option<super::SidebarEntry>>,
        pub chosen_tag: Cell<Option<pigoune_core::TagId>>,
        pub rebuilding: Cell<bool>,
        pub cloud_place: crate::sidebar_tag_cloud::SharedCloudPlace,
        pub scroll_keeper: RefCell<Option<(gtk::Adjustment, glib::SignalHandlerId)>>,
        pub on_view_changed: RefCell<Option<ViewChangedCallback>>,
        pub on_content_dropped: RefCell<Option<ContentDroppedCallback>>,
        pub on_assets_dropped: RefCell<Option<AssetsDroppedCallback>>,
        pub on_collection_dropped: RefCell<Option<CollectionDroppedCallback>>,
        pub on_tag_dropped: RefCell<Option<TagDroppedCallback>>,
        pub on_assets_hovered: RefCell<Option<AssetsHoveredCallback>>,
        pub on_drop_offer: RefCell<Option<DropOfferCallback>>,
        pub on_smart_collection_dropped: RefCell<Option<SmartCollectionDroppedCallback>>,
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
            sidebar.skip_headers_with_arrows();
            sidebar.quiet_hover_while_dragging();
            sidebar.toggle_sections_on_activation();
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

    pub fn connect_content_dropped(&self, callback: impl Fn(AssetView, Dropped) + 'static) {
        self.imp()
            .on_content_dropped
            .replace(Some(Box::new(callback)));
    }

    pub fn content_dropped(&self, view: AssetView, dropped: Dropped) {
        if let Some(on_content_dropped) = self.imp().on_content_dropped.borrow().as_ref() {
            on_content_dropped(view, dropped);
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

    pub fn connect_drop_offer(&self, callback: impl Fn(AssetView) -> DropOffer + 'static) {
        self.imp().on_drop_offer.replace(Some(Box::new(callback)));
    }

    pub fn drop_offer(&self, view: AssetView) -> DropOffer {
        self.imp()
            .on_drop_offer
            .borrow()
            .as_ref()
            .map_or(DropOffer::COPY_ONLY, |offer| offer(view))
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

    pub fn connect_tag_dropped(&self, callback: impl Fn(TagId, Option<TagId>) + 'static) {
        self.imp().on_tag_dropped.replace(Some(Box::new(callback)));
    }

    pub fn tag_dropped(&self, dragged: TagId, target: Option<TagId>) {
        if let Some(on_tag_dropped) = self.imp().on_tag_dropped.borrow().as_ref() {
            on_tag_dropped(dragged, target);
        }
    }

    pub fn collection_dropped(&self, dragged: CollectionId, drop: CollectionDrop) {
        if let Some(on_collection_dropped) = self.imp().on_collection_dropped.borrow().as_ref() {
            on_collection_dropped(dragged, drop);
        }
    }

    pub fn connect_smart_collection_dropped(
        &self,
        callback: impl Fn(SmartCollectionId, SmartCollectionId, bool) + 'static,
    ) {
        self.imp()
            .on_smart_collection_dropped
            .replace(Some(Box::new(callback)));
    }

    pub fn smart_collection_dropped(
        &self,
        dragged: SmartCollectionId,
        target: SmartCollectionId,
        after: bool,
    ) {
        if let Some(on_dropped) = self.imp().on_smart_collection_dropped.borrow().as_ref() {
            on_dropped(dragged, target, after);
        }
    }

    pub fn connect_section_toggled(&self, callback: impl Fn(SidebarEntry) + 'static) {
        self.imp()
            .on_section_toggled
            .replace(Some(Box::new(callback)));
    }

    pub fn section_toggled(&self, entry: SidebarEntry) {
        if self.focus_is_inside() {
            self.imp().refocused_header.set(Some(entry));
        }
        if let Some(on_section_toggled) = self.imp().on_section_toggled.borrow().as_ref() {
            on_section_toggled(entry);
        }
    }

    fn toggle_sections_on_activation(&self) {
        self.imp().list_view.connect_activate(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            move |list_view, position| {
                let header = list_view
                    .model()
                    .and_then(|model| item_at(model.item(position)))
                    .filter(PigouneSidebarItem::is_header);
                if let Some(header) = header {
                    sidebar.section_toggled(header.entry());
                }
            }
        ));
    }

    pub fn show_content(&self, content: &SidebarContent) {
        let imp = self.imp();
        let mut expanded = self.expanded_collections();
        expanded.extend(content.reveal.iter().copied());
        let counts = content.show_counts.then_some(&content.counts);

        let mut root = Vec::new();
        for (view, label, icon) in [
            (AssetView::All, gettext("All"), ALL_ICON),
            (AssetView::Favorites, gettext("Favorites"), FAVORITES_ICON),
            (
                AssetView::Unclassified,
                gettext("Unclassified"),
                UNCLASSIFIED_ICON,
            ),
        ] {
            root.push(view_item(view, label, icon, counts, None));
        }
        let folded = content.folded;
        let chosen_tag = match content.selected {
            AssetView::Tag(id) => Some(id),
            _ => None,
        };
        imp.chosen_tag.set(chosen_tag);
        let collections = collection_items(&content.tree, None, counts);
        root.push(header_item(
            SidebarEntry::CollectionsHeader,
            gettext("Collections"),
            folded.collections.then_some(0),
        ));
        if !folded.collections {
            root.extend(collections);
        }
        if content.show_smart_collections {
            root.push(header_item(
                SidebarEntry::SmartCollectionsHeader,
                gettext("Smart Collections"),
                folded.smart_collections.then_some(0),
            ));
            if !folded.smart_collections {
                for collection in &content.smart_collections {
                    root.push(view_item(
                        AssetView::Smart(collection.id),
                        collection.name.clone(),
                        SMART_COLLECTION_ICON,
                        counts,
                        None,
                    ));
                }
            }
        }
        if !content.tags.is_empty() {
            root.push(header_item(
                SidebarEntry::TagsHeader,
                gettext("Tags"),
                folded.tags.then_some(content.tags.len()),
            ));
            if !folded.tags {
                root.push(tag_cloud_item(
                    &content.tags,
                    counts,
                    chosen_tag,
                    &imp.cloud_place,
                ));
            }
        }
        let trash = view_item(AssetView::Trash, gettext("Trash"), TRASH_ICON, counts, None);
        let (tree_model, selection) = self.models_of(&imp.list_view);
        let (_, trash_selection) = self.models_of(&imp.trash_view);
        let scrolled_to = imp
            .list_view
            .vadjustment()
            .map(|adjustment| adjustment.value());

        imp.rebuilding.set(true);
        let resized = update_in_place(&tree_model, &root);
        let _ = update_in_place(&trash_selection_tree(&trash_selection), &[trash]);
        expand(&tree_model, &expanded);
        if content.selected == AssetView::Trash {
            selection.set_selected(gtk::INVALID_LIST_POSITION);
            trash_selection.set_selected(0);
        } else if chosen_tag.is_some() {
            selection.set_selected(gtk::INVALID_LIST_POSITION);
            trash_selection.set_selected(gtk::INVALID_LIST_POSITION);
        } else {
            let position =
                position_of(&tree_model, content.selected).unwrap_or(gtk::INVALID_LIST_POSITION);
            selection.set_selected(position);
            trash_selection.set_selected(gtk::INVALID_LIST_POSITION);
        }
        imp.rebuilding.set(false);
        self.forget_scroll_keeper();
        if let Some(header) = imp.refocused_header.take()
            && let Some(position) = position_of_entry(&tree_model, header)
        {
            imp.list_view
                .scroll_to(position, gtk::ListScrollFlags::FOCUS, None);
        } else if resized
            && content.reveal.is_empty()
            && let Some(scrolled_to) = scrolled_to
        {
            self.keep_position_through_resize(scrolled_to);
        }
    }

    fn keep_position_through_resize(&self, wanted: f64) {
        let Some(adjustment) = self.imp().list_view.vadjustment() else {
            return;
        };
        let handler = adjustment.connect_changed(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            move |adjustment| {
                let reachable = wanted.min(adjustment.upper() - adjustment.page_size());
                if (adjustment.value() - reachable).abs() >= 1.0 {
                    adjustment.set_value(reachable);
                }
                sidebar.forget_scroll_keeper();
            }
        ));
        self.imp()
            .scroll_keeper
            .replace(Some((adjustment, handler)));
        let Some(clock) = self.imp().list_view.frame_clock() else {
            self.forget_scroll_keeper();
            return;
        };
        let after_paint = std::rc::Rc::new(std::cell::Cell::new(None));
        after_paint.set(Some(clock.connect_after_paint(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            #[strong]
            after_paint,
            move |clock| {
                sidebar.forget_scroll_keeper();
                if let Some(handler) = after_paint.take() {
                    clock.disconnect(handler);
                }
            }
        ))));
    }

    fn forget_scroll_keeper(&self) {
        if let Some((adjustment, handler)) = self.imp().scroll_keeper.take() {
            adjustment.disconnect(handler);
        }
    }

    fn models_of(&self, list_view: &gtk::ListView) -> (gtk::TreeListModel, gtk::SingleSelection) {
        if let Some(selection) = list_view.model().and_downcast::<gtk::SingleSelection>()
            && let Some(tree_model) = selection.model().and_downcast::<gtk::TreeListModel>()
        {
            return (tree_model, selection);
        }
        let tree_model = tree_of(gio::ListStore::new::<PigouneSidebarItem>());
        let selection = self.selection_of(&tree_model);
        list_view.set_model(Some(&selection));
        (tree_model, selection)
    }

    fn selection_of(&self, tree_model: &gtk::TreeListModel) -> gtk::SingleSelection {
        let selection = gtk::SingleSelection::new(Some(tree_model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        selection.connect_selected_item_notify(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            move |selection| sidebar.announce_selection(selection)
        ));
        selection
    }

    pub fn point_out(&self, view: AssetView) {
        if let AssetView::Tag(tag) = view {
            let sidebar = self.downgrade();
            found_flash::flash_once_shown(move || {
                sidebar.upgrade().is_none_or(|sidebar| {
                    sidebar
                        .tag_cloud()
                        .and_then(|cloud| cloud.pill_of(tag))
                        .filter(WidgetExt::is_mapped)
                        .is_some_and(|pill| {
                            found_flash::flash(pill.upcast_ref());
                            true
                        })
                })
            });
            return;
        }
        if view == AssetView::Trash {
            let sidebar = self.downgrade();
            found_flash::flash_once_shown(move || {
                sidebar
                    .upgrade()
                    .is_none_or(|sidebar| Self::flash_row_of(&sidebar.imp().trash_view, view))
            });
            return;
        }
        let Some(position) = self
            .tree_model()
            .and_then(|tree_model| position_of(&tree_model, view))
        else {
            return;
        };
        self.imp()
            .list_view
            .scroll_to(position, gtk::ListScrollFlags::NONE, None);
        let sidebar = self.downgrade();
        found_flash::flash_once_shown(move || {
            sidebar
                .upgrade()
                .is_none_or(|sidebar| Self::flash_row_of(&sidebar.imp().list_view, view))
        });
    }

    fn flash_row_of(list: &gtk::ListView, view: AssetView) -> bool {
        let mut child = list.first_child();
        while let Some(row) = child {
            let shows_view = row
                .first_child()
                .and_downcast::<gtk::TreeExpander>()
                .and_then(|expander| expander.child())
                .and_downcast::<PigouneSidebarRow>()
                .is_some_and(|sidebar_row| sidebar_row.view() == Some(view));
            if shows_view && row.is_mapped() {
                found_flash::flash(&row);
                return true;
            }
            child = row.next_sibling();
        }
        false
    }

    fn announce_selection(&self, selection: &gtk::SingleSelection) {
        let imp = self.imp();
        if imp.rebuilding.get() {
            return;
        }
        let Some(view) = item_at(selection.selected_item()).and_then(|item| item.view()) else {
            return;
        };
        self.unselect_other_list(selection);
        imp.chosen_tag.set(None);
        if let Some(cloud) = self.tag_cloud() {
            cloud.highlight(None);
        }
        if let Some(on_view_changed) = imp.on_view_changed.borrow().as_ref() {
            on_view_changed(view);
        }
    }

    pub fn choose_view(&self, view: AssetView) {
        let (tree_model, selection) = self.models_of(&self.imp().list_view);
        if let Some(position) = position_of(&tree_model, view) {
            selection.set_selected(position);
        }
    }

    pub fn choose_tag(&self, tag: TagId) {
        let imp = self.imp();
        imp.chosen_tag.set(Some(tag));
        imp.rebuilding.set(true);
        for list in [&*imp.list_view, &*imp.trash_view] {
            if let Some(selection) = list.model().and_downcast::<gtk::SingleSelection>() {
                selection.set_selected(gtk::INVALID_LIST_POSITION);
            }
        }
        imp.rebuilding.set(false);
        if let Some(cloud) = self.tag_cloud() {
            cloud.highlight(Some(tag));
        }
        if let Some(on_view_changed) = imp.on_view_changed.borrow().as_ref() {
            on_view_changed(AssetView::Tag(tag));
        }
    }

    fn tag_cloud(&self) -> Option<PigouneSidebarTagCloud> {
        let mut child = self.imp().list_view.first_child();
        while let Some(row) = child {
            let cloud = row
                .first_child()
                .and_downcast::<gtk::TreeExpander>()
                .and_then(|expander| expander.child())
                .and_downcast::<PigouneSidebarRow>()
                .and_then(|sidebar_row| sidebar_row.tag_cloud());
            if cloud.is_some() {
                return cloud;
            }
            child = row.next_sibling();
        }
        None
    }

    fn unselect_other_list(&self, chosen: &gtk::SingleSelection) {
        let imp = self.imp();
        imp.rebuilding.set(true);
        for list in [&*imp.list_view, &*imp.trash_view] {
            if let Some(other) = list.model().and_downcast::<gtk::SingleSelection>()
                && &other != chosen
            {
                other.set_selected(gtk::INVALID_LIST_POSITION);
            }
        }
        imp.rebuilding.set(false);
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
                    (gdk::Key::F2, Some(AssetView::Smart(id))) => {
                        ("win.edit-smart-collection", id.to_string())
                    }
                    (gdk::Key::Delete, Some(AssetView::Smart(id))) => {
                        ("win.delete-smart-collection", id.to_string())
                    }
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

        let menu_keys = gtk::ShortcutController::new();
        menu_keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string(MENU_KEYS),
            Some(gtk::CallbackAction::new(glib::clone!(
                #[weak(rename_to = sidebar)]
                self,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    if sidebar.focused_row().is_some_and(|row| row.open_menu()) {
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
            ))),
        ));
        self.imp().list_view.add_controller(menu_keys);
    }

    fn quiet_hover_while_dragging(&self) {
        let imp = self.imp();
        for list_view in [imp.list_view.get(), imp.trash_view.get()] {
            let motion = gtk::DropControllerMotion::new();
            motion.connect_enter(glib::clone!(
                #[weak]
                list_view,
                move |_, _, _| list_view.add_css_class(DRAG_OVER)
            ));
            motion.connect_leave(glib::clone!(
                #[weak]
                list_view,
                move |_| list_view.remove_css_class(DRAG_OVER)
            ));
            list_view.add_controller(motion);
        }
    }

    fn skip_headers_with_arrows(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = sidebar)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if !modifiers.is_empty() || !sidebar.focus_is_on_an_entry() {
                    return glib::Propagation::Proceed;
                }
                match key {
                    gdk::Key::Up | gdk::Key::KP_Up => sidebar.move_to_next_entry(-1),
                    gdk::Key::Down | gdk::Key::KP_Down => sidebar.move_to_next_entry(1),
                    gdk::Key::Right | gdk::Key::KP_Right => sidebar.expand_selected(),
                    gdk::Key::Left | gdk::Key::KP_Left => sidebar.collapse_selected(),
                    _ => return glib::Propagation::Proceed,
                }
                glib::Propagation::Stop
            }
        ));
        self.imp().list_view.add_controller(keys);
    }

    fn focus_is_inside(&self) -> bool {
        self.root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is_ancestor(&*self.imp().list_view))
    }

    fn focus_is_on_an_entry(&self) -> bool {
        self.root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| {
                focus.is_ancestor(&*self.imp().list_view)
                    && focus
                        .first_child()
                        .is_some_and(|child| child.is::<gtk::TreeExpander>())
            })
    }

    fn move_to_next_entry(&self, step: i64) {
        let Some(selection) = self
            .imp()
            .list_view
            .model()
            .and_downcast::<gtk::SingleSelection>()
        else {
            return;
        };
        let Some(current) = self.focused_position().or_else(|| {
            Some(selection.selected()).filter(|selected| *selected != gtk::INVALID_LIST_POSITION)
        }) else {
            return;
        };
        let target = i64::from(current) + step;
        let Ok(target) = u32::try_from(target) else {
            return;
        };
        let Some(item) = item_at(selection.item(target)) else {
            return;
        };
        let flags = if item.is_header() || item.is_tag_cloud() {
            gtk::ListScrollFlags::FOCUS
        } else {
            gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT
        };
        self.imp().list_view.scroll_to(target, flags, None);
    }

    fn focused_position(&self) -> Option<u32> {
        let focus = self.root()?.focus()?;
        if !focus.is_ancestor(&*self.imp().list_view) {
            return None;
        }
        let expander = focus
            .first_child()
            .and_downcast::<gtk::TreeExpander>()
            .or_else(|| {
                focus
                    .ancestor(gtk::TreeExpander::static_type())
                    .and_downcast()
            })?;
        Some(expander.list_row()?.position())
    }

    fn selected_tree_row(&self) -> Option<gtk::TreeListRow> {
        self.imp()
            .list_view
            .model()
            .and_downcast::<gtk::SingleSelection>()?
            .selected_item()
            .and_downcast::<gtk::TreeListRow>()
    }

    fn expand_selected(&self) {
        if let Some(row) = self.selected_tree_row()
            && row.is_expandable()
        {
            row.set_expanded(true);
        }
    }

    fn collapse_selected(&self) {
        let Some(row) = self.selected_tree_row() else {
            return;
        };
        if row.is_expanded() {
            row.set_expanded(false);
        } else if let Some(parent) = row.parent() {
            self.imp().list_view.scroll_to(
                parent.position(),
                gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT,
                None,
            );
        }
    }

    fn focused_row(&self) -> Option<PigouneSidebarRow> {
        let focus = self.root()?.focus()?;
        if !focus.is_ancestor(&*self.imp().list_view) {
            return None;
        }
        focus
            .first_child()
            .and_downcast::<gtk::TreeExpander>()?
            .child()
            .and_downcast::<PigouneSidebarRow>()
    }

    fn selected_view(&self) -> Option<AssetView> {
        let imp = self.imp();
        if let Some(tag) = imp.chosen_tag.get() {
            return Some(AssetView::Tag(tag));
        }
        [&*imp.list_view, &*imp.trash_view]
            .into_iter()
            .find_map(|list| {
                let selection = list.model().and_downcast::<gtk::SingleSelection>()?;
                item_at(selection.selected_item())?.view()
            })
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
            list_item.set_selectable(!is_header && !item.is_tag_cloud());
            list_item.set_activatable(is_header);
            let spoken_label = item.spoken_label();
            list_item.set_accessible_label(&spoken_label);
            if is_header {
                expander.update_state(&[gtk::accessible::State::Expanded(Some(!item.folded()))]);
            }
            expander.set_list_row(Some(&row));
            expander.set_indent_for_icon(collection_of(&row).is_some());
            if let Some(row) = expander.child().and_downcast::<PigouneSidebarRow>() {
                row.show(&item);
            }
            expander.reset_relation(gtk::AccessibleRelation::LabelledBy);
            expander.update_property(&[gtk::accessible::Property::Label(&spoken_label)]);
        });
        self.imp().list_view.set_factory(Some(&factory));
        self.imp().trash_view.set_factory(Some(&factory));
    }
}

impl Default for PigouneSidebar {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn header_item(entry: SidebarEntry, label: String, folded: Option<usize>) -> PigouneSidebarItem {
    PigouneSidebarItem::new(SidebarItemData {
        entry,
        label,
        icon_name: "",
        color_class: None,
        count: folded,
        folded: folded.is_some(),
        tag_pills: Vec::new(),
        selected_tag: None,
        cloud_place: None,
        children: None,
    })
}

fn trash_selection_tree(selection: &gtk::SingleSelection) -> gtk::TreeListModel {
    selection
        .model()
        .and_downcast::<gtk::TreeListModel>()
        .expect("the trash list shows a tree model")
}

fn update_in_place(tree_model: &gtk::TreeListModel, items: &[PigouneSidebarItem]) -> bool {
    let Some(store) = tree_model.model().downcast::<gio::ListStore>().ok() else {
        return false;
    };
    let current: Vec<PigouneSidebarItem> = (0..store.n_items())
        .filter_map(|position| store.item(position).and_downcast())
        .collect();
    let kept_before = current
        .iter()
        .zip(items)
        .take_while(|(old, new)| old.same_as(new))
        .count();
    let kept_after = current[kept_before..]
        .iter()
        .rev()
        .zip(items[kept_before..].iter().rev())
        .take_while(|(old, new)| old.same_as(new))
        .count();
    let removed = current.len() - kept_before - kept_after;
    let added = &items[kept_before..items.len() - kept_after];
    if removed == 0 && added.is_empty() {
        return false;
    }
    store.splice(
        u32::try_from(kept_before).unwrap_or(u32::MAX),
        u32::try_from(removed).unwrap_or(u32::MAX),
        added,
    );
    true
}

fn tree_of(root: gio::ListStore) -> gtk::TreeListModel {
    gtk::TreeListModel::new(root, false, false, |item| {
        item.downcast_ref::<PigouneSidebarItem>()
            .and_then(PigouneSidebarItem::children)
            .map(Cast::upcast)
    })
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
        color_class: None,
        count: counts.map(|counts| counts.of(view)),
        folded: false,
        tag_pills: Vec::new(),
        selected_tag: None,
        cloud_place: None,
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
            let view = AssetView::Collection(collection.id);
            PigouneSidebarItem::new(SidebarItemData {
                entry: SidebarEntry::View(view),
                label: collection.name.clone(),
                icon_name: collection_looks::icon_name(&collection.look),
                color_class: collection_looks::color_class(&collection.look),
                count: counts.map(|counts| counts.of(view)),
                folded: false,
                tag_pills: Vec::new(),
                selected_tag: None,
                cloud_place: None,
                children: store,
            })
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
        | AssetView::Smart(_)
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

fn position_of_entry(tree_model: &gtk::TreeListModel, entry: SidebarEntry) -> Option<u32> {
    (0..tree_model.n_items()).find(|position| {
        tree_model
            .row(*position)
            .and_then(|row| row.item().and_downcast::<PigouneSidebarItem>())
            .is_some_and(|item| item.entry() == entry)
    })
}

fn tag_cloud_item(
    tags: &[Tag],
    counts: Option<&ViewCounts>,
    chosen: Option<TagId>,
    place: &SharedCloudPlace,
) -> PigouneSidebarItem {
    PigouneSidebarItem::new(SidebarItemData {
        entry: SidebarEntry::TagCloud,
        label: gettext("Tags"),
        icon_name: "",
        color_class: None,
        count: None,
        folded: false,
        tag_pills: tags
            .iter()
            .map(|tag| TagPill {
                id: tag.id,
                name: tag.name.clone(),
                parent: tag.parent,
                count: counts.map(|counts| counts.of(AssetView::Tag(tag.id))),
            })
            .collect(),
        selected_tag: chosen,
        cloud_place: Some(place.clone()),
        children: None,
    })
}
