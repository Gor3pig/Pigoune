use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use pigoune_core::AssetId;

use crate::asset_object::PigouneAssetObject;
use crate::asset_sort::SortedAsset;
use crate::asset_tile::PigouneAssetTile;
use crate::drag_content::DraggedAssets;
use crate::drag_icon;
use crate::found_flash;
use crate::grid_columns;
use crate::thumbnails::ThumbnailCache;

pub const MENU_KEYS: &str = "<Shift>F10|Menu";

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::{gio, glib};

    use super::{DEFAULT_TILE_SIZE, LARGEST_TILE_SIZE, SMALLEST_TILE_SIZE};
    use crate::asset_object::PigouneAssetObject;
    use crate::asset_sort::{SortCriterion, SortOrder};
    use crate::thumbnails::ThumbnailCache;

    #[derive(gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-grid.ui")]
    #[properties(wrapper_type = super::PigouneAssetGrid)]
    pub struct PigouneAssetGrid {
        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub grid_view: TemplateChild<gtk::GridView>,
        #[template_child]
        pub grid_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub nothing_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub context_menu: TemplateChild<gtk::PopoverMenu>,
        #[property(get, set = Self::set_tile_size, minimum = SMALLEST_TILE_SIZE, maximum = LARGEST_TILE_SIZE, default = DEFAULT_TILE_SIZE)]
        pub tile_size: Cell<i32>,
        #[property(get, set = Self::set_sort_criterion)]
        pub sort_criterion: RefCell<String>,
        #[property(get, set = Self::set_sort_reversed)]
        pub sort_reversed: Cell<bool>,
        pub sort_order: Rc<Cell<SortOrder>>,
        pub sorter: RefCell<Option<gtk::CustomSorter>>,
        pub assets: gio::ListStore,
        pub thumbnails: Rc<ThumbnailCache>,
        pub export_copies: RefCell<Option<super::ExportCopies>>,
        pub drag_caption: RefCell<Option<gtk::Label>>,
    }

    impl Default for PigouneAssetGrid {
        fn default() -> Self {
            Self {
                scrolled_window: TemplateChild::default(),
                grid_view: TemplateChild::default(),
                grid_box: TemplateChild::default(),
                content_stack: TemplateChild::default(),
                nothing_page: TemplateChild::default(),
                context_menu: TemplateChild::default(),
                tile_size: Cell::new(DEFAULT_TILE_SIZE),
                sort_criterion: RefCell::default(),
                sort_reversed: Cell::default(),
                sort_order: Rc::default(),
                sorter: RefCell::default(),
                assets: gio::ListStore::new::<PigouneAssetObject>(),
                thumbnails: Rc::default(),
                export_copies: RefCell::default(),
                drag_caption: RefCell::default(),
            }
        }
    }

    impl PigouneAssetGrid {
        fn set_sort_criterion(&self, criterion: String) {
            let order = SortOrder {
                criterion: SortCriterion::from_setting(&criterion),
                ..self.sort_order.get()
            };
            self.sort_criterion.replace(criterion);
            self.apply_sort_order(order);
        }

        fn set_sort_reversed(&self, reversed: bool) {
            let order = SortOrder {
                reversed,
                ..self.sort_order.get()
            };
            self.sort_reversed.set(reversed);
            self.apply_sort_order(order);
        }

        fn apply_sort_order(&self, order: SortOrder) {
            if self.sort_order.replace(order) == order {
                return;
            }
            self.obj().resort();
        }

        fn set_tile_size(&self, size: i32) {
            if self.tile_size.replace(size) != size {
                self.obj().notify_tile_size();
                self.obj().fit_columns();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetGrid {
        const NAME: &'static str = "PigouneAssetGrid";
        type Type = super::PigouneAssetGrid;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneAssetGrid {
        fn constructed(&self) {
            self.parent_constructed();
            let grid = self.obj();
            grid.set_up_grid();
            grid.resize_with_control_scroll();
            grid.unselect_on_empty_click();
            grid.lasso_only_from_empty_space();
            grid.follow_visible_width();
        }
    }

    impl WidgetImpl for PigouneAssetGrid {}
    impl BinImpl for PigouneAssetGrid {}
}

glib::wrapper! {
    pub struct PigouneAssetGrid(ObjectSubclass<imp::PigouneAssetGrid>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

type ExportCopies = Rc<dyn Fn(&[AssetId]) -> Vec<PathBuf>>;

const ASSETS_PAGE: &str = "assets";
const NOTHING_PAGE: &str = "nothing";
const SMALLEST_TILE_SIZE: i32 = 64;
const LARGEST_TILE_SIZE: i32 = 256;
const DEFAULT_TILE_SIZE: i32 = 128;
const TILE_SIZE_STEP: i32 = 32;
const PIXELS_PER_SCROLL_STEP: f64 = 16.0;

impl PigouneAssetGrid {
    pub fn nothing_page(&self) -> adw::StatusPage {
        self.imp().nothing_page.get()
    }

    pub fn show_nothing(&self, nothing: bool) {
        self.imp().content_stack.set_visible_child_name(if nothing {
            NOTHING_PAGE
        } else {
            ASSETS_PAGE
        });
    }

    pub fn enlarge_tiles(&self) {
        self.resize_tiles(next_larger_step(self.tile_size()));
    }

    pub fn shrink_tiles(&self) {
        self.resize_tiles(next_smaller_step(self.tile_size()));
    }

    fn resize_tiles(&self, size: i32) {
        self.set_tile_size(allowed_tile_size(size));
    }

    pub fn show_assets(&self, assets: &[PigouneAssetObject]) {
        let store = &self.imp().assets;
        store.splice(0, store.n_items(), assets);
    }

    pub fn selected_assets(&self) -> Vec<PigouneAssetObject> {
        let Some(selection) = self.selection() else {
            return Vec::new();
        };
        let chosen = selection.selection();
        (0..chosen.size())
            .filter_map(|index| u32::try_from(index).ok())
            .filter_map(|index| selection.item(chosen.nth(index)))
            .filter_map(|item| item.downcast::<PigouneAssetObject>().ok())
            .collect()
    }

    pub fn selected_asset(&self) -> Option<PigouneAssetObject> {
        let mut selected = self.selected_assets();
        if selected.len() == 1 {
            selected.pop()
        } else {
            None
        }
    }

    pub fn visible_assets(&self) -> Vec<PigouneAssetObject> {
        let Some(selection) = self.selection() else {
            return Vec::new();
        };
        (0..selection.n_items())
            .filter_map(|position| {
                selection
                    .item(position)
                    .and_downcast::<PigouneAssetObject>()
            })
            .collect()
    }

    pub fn remove_asset(&self, id: AssetId) {
        let store = &self.imp().assets;
        let position = (0..store.n_items()).find(|position| {
            store
                .item(*position)
                .and_downcast::<PigouneAssetObject>()
                .is_some_and(|asset| asset.id() == id)
        });
        if let Some(position) = position {
            store.remove(position);
        }
    }

    pub fn resort(&self) {
        if let Some(sorter) = self.imp().sorter.borrow().as_ref() {
            sorter.changed(gtk::SorterChange::Different);
        }
        self.follow_selection_later();
    }

    pub fn select_asset(&self, id: AssetId) {
        self.select_assets(&[id]);
    }

    pub fn select_assets(&self, ids: &[AssetId]) {
        let Some(selection) = self.selection() else {
            return;
        };
        let wanted: HashSet<AssetId> = ids.iter().copied().collect();
        let chosen = gtk::Bitset::new_empty();
        for position in 0..selection.n_items() {
            let is_wanted = selection
                .item(position)
                .and_downcast::<PigouneAssetObject>()
                .is_some_and(|asset| wanted.contains(&asset.id()));
            if is_wanted {
                chosen.add(position);
            }
        }
        let everything = gtk::Bitset::new_range(0, selection.n_items());
        selection.set_selection(&chosen, &everything);
        self.follow_selection_later();
    }

    pub fn reveal_asset(&self, id: AssetId) {
        let Some(selection) = self.selection() else {
            return;
        };
        let position = (0..selection.n_items()).find(|position| {
            selection
                .item(*position)
                .and_downcast::<PigouneAssetObject>()
                .is_some_and(|asset| asset.id() == id)
        });
        if let Some(position) = position {
            self.imp()
                .grid_view
                .scroll_to(position, gtk::ListScrollFlags::FOCUS, None);
        }
    }

    pub fn point_out_selected_next(&self) {
        let selected: HashSet<AssetId> = self
            .selected_assets()
            .iter()
            .map(PigouneAssetObject::id)
            .collect();
        let Some(selection) = self.selection() else {
            return;
        };
        let Some(first) =
            (0..selection.n_items()).find(|position| selection.is_selected(*position))
        else {
            return;
        };
        self.imp()
            .grid_view
            .scroll_to(first, gtk::ListScrollFlags::FOCUS, None);
        let grid = self.downgrade();
        found_flash::flash_after_previous(move || {
            grid.upgrade()
                .is_none_or(|grid| grid.flash_tiles_of(&selected))
        });
    }

    fn flash_tiles_of(&self, ids: &HashSet<AssetId>) -> bool {
        let mut flashed = false;
        let mut child = self.imp().grid_view.first_child();
        while let Some(cell) = child {
            let is_wanted = cell
                .first_child()
                .and_downcast::<PigouneAssetTile>()
                .and_then(|tile| tile.asset())
                .is_some_and(|asset| ids.contains(&asset.id()));
            if is_wanted && cell.is_mapped() {
                found_flash::flash(&cell);
                flashed = true;
            }
            child = cell.next_sibling();
        }
        flashed
    }

    fn follow_selection_later(&self) {
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move || grid.follow_selection()
        ));
    }

    fn follow_selection(&self) {
        let grid_view = &self.imp().grid_view;
        let Some(selection) = self.selection() else {
            return;
        };
        if selection.n_items() == 0 {
            return;
        }
        let chosen = selection.selection();
        if chosen.is_empty() {
            grid_view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
        } else {
            grid_view.scroll_to(chosen.minimum(), gtk::ListScrollFlags::FOCUS, None);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.imp().assets.n_items() == 0
    }

    pub fn thumbnails(&self) -> Rc<ThumbnailCache> {
        Rc::clone(&self.imp().thumbnails)
    }

    pub fn connect_selection_changed(&self, callback: impl Fn(Vec<PigouneAssetObject>) + 'static) {
        let Some(selection) = self.selection() else {
            return;
        };
        let callback = Rc::new(callback);
        let on_items = Rc::clone(&callback);
        selection.connect_selection_changed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |_, _, _| callback(grid.selected_assets())
        ));
        selection.connect_items_changed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |_, _, _, _| on_items(grid.selected_assets())
        ));
    }

    pub fn selection(&self) -> Option<gtk::MultiSelection> {
        self.imp()
            .grid_view
            .model()
            .and_downcast::<gtk::MultiSelection>()
    }

    pub fn connect_preview_requested(&self, callback: impl Fn(Option<u32>) + 'static) {
        let callback = Rc::new(callback);
        let grid_view = &self.imp().grid_view;
        let on_activate = Rc::clone(&callback);
        grid_view.connect_activate(move |_, position| on_activate(Some(position)));
        let space = gtk::EventControllerKey::new();
        space.set_propagation_phase(gtk::PropagationPhase::Capture);
        space.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::space && modifiers.is_empty() {
                callback(None);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        grid_view.add_controller(space);
    }

    pub fn connect_rename_requested(&self, callback: impl Fn() + 'static) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                match key {
                    gdk::Key::F2 => callback(),
                    gdk::Key::Escape => {
                        if let Some(selection) = grid.selection() {
                            selection.unselect_all();
                        }
                    }
                    _ => return glib::Propagation::Proceed,
                }
                glib::Propagation::Stop
            }
        ));
        self.imp().grid_view.add_controller(keys);
    }

    pub fn connect_trash_requested(&self, callback: impl Fn() + 'static) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::Delete && modifiers.is_empty() {
                callback();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.imp().grid_view.add_controller(keys);
    }

    pub fn reveal_selected(&self) {
        self.follow_selection();
    }

    pub fn focus_selected_later(&self) {
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move || {
                grid.imp().grid_view.grab_focus();
                grid.follow_selection();
            }
        ));
    }

    pub fn forget_thumbnails(&self) {
        self.imp().thumbnails.forget_all();
    }

    fn unselect_on_empty_click(&self) {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .propagation_phase(gtk::PropagationPhase::Capture)
            .build();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |gesture, _, x, y| {
                let on_empty_space = grid.asset_at(x, y).is_none();
                let extending = gesture
                    .current_event_state()
                    .intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
                if !extending
                    && on_empty_space
                    && let Some(selection) = grid.selection()
                {
                    selection.unselect_all();
                }
            }
        ));
        self.imp().grid_view.add_controller(click);
    }

    fn lasso_only_from_empty_space(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |motion, x, y| {
                let pressing = motion
                    .current_event_state()
                    .intersects(gdk::ModifierType::BUTTON1_MASK);
                let on_empty_space = grid.asset_at(x, y).is_none();
                if !pressing && grid.imp().grid_view.enables_rubberband() != on_empty_space {
                    glib::idle_add_local_once(glib::clone!(
                        #[weak]
                        grid,
                        move || grid.imp().grid_view.set_enable_rubberband(on_empty_space)
                    ));
                }
            }
        ));
        self.imp().grid_view.add_controller(motion);
    }

    fn asset_at(&self, x: f64, y: f64) -> Option<PigouneAssetObject> {
        let widget = self.imp().grid_view.pick(x, y, gtk::PickFlags::DEFAULT)?;
        let tile = match widget.downcast::<PigouneAssetTile>() {
            Ok(tile) => tile,
            Err(widget) => widget
                .ancestor(PigouneAssetTile::static_type())
                .and_downcast::<PigouneAssetTile>()?,
        };
        tile.asset()
    }

    pub fn connect_context_menu_requested(&self, callback: impl Fn() -> gio::MenuModel + 'static) {
        let callback = Rc::new(callback);
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        let on_click = Rc::clone(&callback);
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |gesture, _, x, y| {
                if grid.show_context_menu(x, y, &*on_click) {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            }
        ));
        self.imp().grid_view.add_controller(click);

        let long_press = gtk::GestureLongPress::new();
        let on_long_press = Rc::clone(&callback);
        long_press.connect_pressed(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |gesture, x, y| {
                if grid.show_context_menu(x, y, &*on_long_press) {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            }
        ));
        self.imp().grid_view.add_controller(long_press);

        let menu_keys = gtk::ShortcutController::new();
        menu_keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string(MENU_KEYS),
            Some(gtk::CallbackAction::new(glib::clone!(
                #[weak(rename_to = grid)]
                self,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    if grid.show_context_menu_for_focus(&*callback) {
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
            ))),
        ));
        self.imp().grid_view.add_controller(menu_keys);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer coordinates inside the grid fit in an i32"
    )]
    fn show_context_menu(&self, x: f64, y: f64, menu_model: &dyn Fn() -> gio::MenuModel) -> bool {
        let imp = self.imp();
        let Some(asset) = self.asset_at(x, y) else {
            return false;
        };
        let Some(point) = imp.grid_view.compute_point(
            &*imp.grid_box,
            &gtk::graphene::Point::new(x as f32, y as f32),
        ) else {
            return false;
        };
        let pointing_to = gdk::Rectangle::new(point.x() as i32, point.y() as i32, 1, 1);
        self.pop_up_menu_for(&asset, &pointing_to, menu_model);
        true
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "tile bounds inside the grid fit in an i32"
    )]
    fn show_context_menu_for_focus(&self, menu_model: &dyn Fn() -> gio::MenuModel) -> bool {
        let imp = self.imp();
        let Some(tile) = self.focused_tile() else {
            return false;
        };
        let (Some(asset), Some(bounds)) = (tile.asset(), tile.compute_bounds(&*imp.grid_box))
        else {
            return false;
        };
        let pointing_to = gdk::Rectangle::new(
            bounds.x() as i32,
            bounds.y() as i32,
            bounds.width() as i32,
            bounds.height() as i32,
        );
        self.pop_up_menu_for(&asset, &pointing_to, menu_model);
        true
    }

    fn focused_tile(&self) -> Option<PigouneAssetTile> {
        let focus = self.root()?.focus()?;
        if !focus.is_ancestor(&*self.imp().grid_view) {
            return None;
        }
        focus
            .downcast_ref::<PigouneAssetTile>()
            .cloned()
            .or_else(|| focus.first_child().and_downcast::<PigouneAssetTile>())
    }

    fn pop_up_menu_for(
        &self,
        asset: &PigouneAssetObject,
        pointing_to: &gdk::Rectangle,
        menu_model: &dyn Fn() -> gio::MenuModel,
    ) {
        let is_selected = self
            .selected_assets()
            .iter()
            .any(|selected| selected.id() == asset.id());
        if !is_selected {
            self.select_asset(asset.id());
        }
        let menu = &self.imp().context_menu;
        menu.set_menu_model(Some(&menu_model()));
        menu.set_pointing_to(Some(pointing_to));
        menu.popup();
    }

    fn make_draggable(&self, tile: &PigouneAssetTile) {
        let source = gtk::DragSource::builder()
            .actions(gdk::DragAction::COPY)
            .build();
        source.connect_prepare(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            #[weak]
            tile,
            #[upgrade_or]
            None,
            move |_, _, _| {
                let dragged = grid.assets_dragged_from(&tile.asset()?);
                Some(grid.drag_content(dragged))
            }
        ));
        source.connect_drag_begin(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            #[weak]
            tile,
            move |source, drag| grid.show_drag_icon(source, drag, &tile)
        ));
        source.connect_drag_end(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |_, _, _| {
                grid.imp().drag_caption.take();
            }
        ));
        tile.add_controller(source);
    }

    pub fn connect_export_copies(&self, export: impl Fn(&[AssetId]) -> Vec<PathBuf> + 'static) {
        self.imp().export_copies.replace(Some(Rc::new(export)));
    }

    fn drag_content(&self, dragged: Vec<AssetId>) -> gdk::ContentProvider {
        let export = self.imp().export_copies.borrow().clone();
        let copies: Vec<gio::File> = export
            .map(|export| export(&dragged))
            .unwrap_or_default()
            .into_iter()
            .map(gio::File::for_path)
            .collect();
        let inside = gdk::ContentProvider::for_value(&DraggedAssets(dragged).to_value());
        if copies.is_empty() {
            return inside;
        }
        let outside =
            gdk::ContentProvider::for_value(&gdk::FileList::from_array(&copies).to_value());
        gdk::ContentProvider::new_union(&[inside, outside])
    }

    pub fn show_drag_caption(&self, markup: Option<&str>) {
        if let Some(caption) = self.imp().drag_caption.borrow().as_ref() {
            caption.set_visible(markup.is_some());
            caption.set_markup(markup.unwrap_or_default());
        }
    }

    fn show_drag_icon(&self, source: &gtk::DragSource, drag: &gdk::Drag, tile: &PigouneAssetTile) {
        let picture = tile.picture();
        let grabbed = tile.asset();
        let selected = self.selected_assets();
        let several = selected.len() > 1
            && grabbed
                .as_ref()
                .is_some_and(|grabbed| selected.iter().any(|asset| asset.id() == grabbed.id()));
        let front = picture
            .paintable()
            .unwrap_or_else(|| gtk::WidgetPaintable::new(Some(&picture)).upcast());
        let mut layers = vec![front];
        let mut count = 1;
        if several {
            let thumbnails = &self.imp().thumbnails;
            layers.extend(
                selected
                    .iter()
                    .filter(|asset| {
                        grabbed
                            .as_ref()
                            .is_none_or(|grabbed| grabbed.id() != asset.id())
                    })
                    .filter_map(|asset| thumbnails.remembered(asset.id()))
                    .map(Cast::upcast::<gdk::Paintable>),
            );
            count = selected.len();
        }
        if let Some(icon) = drag_icon::stack_icon(self, &layers, count) {
            let caption = drag_icon::caption_label();
            let widget = drag_icon::icon_widget(&icon, &caption);
            gtk::DragIcon::for_drag(drag).set_child(Some(&widget));
            drag.set_hotspot(drag_icon::hotspot_x(&icon), icon.hot_y);
            self.imp().drag_caption.replace(Some(caption));
        } else {
            let icon = gtk::WidgetPaintable::new(Some(&picture));
            source.set_icon(Some(&icon), picture.width() / 2, picture.height() / 2);
        }
    }

    fn assets_dragged_from(&self, asset: &PigouneAssetObject) -> Vec<AssetId> {
        let selected: Vec<AssetId> = self
            .selected_assets()
            .iter()
            .map(PigouneAssetObject::id)
            .collect();
        if selected.contains(&asset.id()) {
            std::iter::once(asset.id())
                .chain(selected.into_iter().filter(|id| *id != asset.id()))
                .collect()
        } else {
            self.select_asset(asset.id());
            vec![asset.id()]
        }
    }

    fn set_up_grid(&self) {
        let imp = self.imp();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move |_, item| {
                if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                    let tile = PigouneAssetTile::new();
                    for side in ["width-request", "height-request"] {
                        grid.bind_property("tile-size", &tile.picture(), side)
                            .sync_create()
                            .build();
                    }
                    grid.make_draggable(&tile);
                    tile.bind_property("description", item, "accessible-label")
                        .sync_create()
                        .build();
                    item.set_child(Some(&tile));
                }
            }
        ));
        let thumbnails = Rc::clone(&imp.thumbnails);
        factory.connect_bind(move |_, item| {
            let Some((tile, asset)) = tile_and_asset(item) else {
                return;
            };
            tile.show_asset(&asset, &thumbnails);
        });
        factory.connect_unbind(|_, item| {
            if let Some((tile, _)) = tile_and_asset(item) {
                tile.forget_asset();
            }
        });

        imp.grid_view.set_factory(Some(&factory));
        let sort_order = Rc::clone(&imp.sort_order);
        let sorter = gtk::CustomSorter::new(move |first, second| {
            let (Some(first), Some(second)) = (
                first.downcast_ref::<PigouneAssetObject>(),
                second.downcast_ref::<PigouneAssetObject>(),
            ) else {
                return gtk::Ordering::Equal;
            };
            let (first_key, second_key) = (first.name_key(), second.name_key());
            sort_order
                .get()
                .compare(
                    &SortedAsset {
                        asset: first.asset(),
                        name_key: &*first_key,
                    },
                    &SortedAsset {
                        asset: second.asset(),
                        name_key: &*second_key,
                    },
                )
                .into()
        });
        imp.sorter.replace(Some(sorter.clone()));
        let sorted_assets = gtk::SortListModel::new(Some(imp.assets.clone()), Some(sorter));
        let selection = gtk::MultiSelection::new(Some(sorted_assets));
        imp.grid_view.set_enable_rubberband(true);
        imp.grid_view.set_model(Some(&selection));
    }
}

impl PigouneAssetGrid {
    fn follow_visible_width(&self) {
        self.imp()
            .scrolled_window
            .hadjustment()
            .connect_page_size_notify(glib::clone!(
                #[weak(rename_to = grid)]
                self,
                move |_| grid.fit_columns()
            ));
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the visible width of the grid fits in an i32"
    )]
    fn fit_columns(&self) {
        let width = self.imp().scrolled_window.hadjustment().page_size() as i32;
        let wanted = grid_columns::widest_column_count(width, self.tile_size());
        if self.imp().grid_view.max_columns() == wanted {
            return;
        }
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move || grid.imp().grid_view.set_max_columns(wanted)
        ));
    }

    pub fn link_size_adjustment(&self, adjustment: &gtk::Adjustment) {
        self.bind_property("tile-size", adjustment, "value")
            .transform_to(|_, size: i32| Some(f64::from(size)))
            .transform_from(|_, value: f64| Some(whole_pixels(value)))
            .bidirectional()
            .sync_create()
            .build();
    }

    fn resize_with_control_scroll(&self) {
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        scroll.connect_scroll(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |controller, _, vertical| {
                if !controller
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK)
                {
                    return glib::Propagation::Proceed;
                }
                let change = whole_pixels(-vertical * PIXELS_PER_SCROLL_STEP);
                grid.resize_tiles(grid.tile_size() + change);
                glib::Propagation::Stop
            }
        ));
        self.imp().scrolled_window.add_controller(scroll);
    }
}

fn allowed_tile_size(size: i32) -> i32 {
    size.clamp(SMALLEST_TILE_SIZE, LARGEST_TILE_SIZE)
}

fn next_larger_step(size: i32) -> i32 {
    (size / TILE_SIZE_STEP + 1) * TILE_SIZE_STEP
}

fn next_smaller_step(size: i32) -> i32 {
    (size - 1) / TILE_SIZE_STEP * TILE_SIZE_STEP
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "tile sizes and scroll steps are a few hundred pixels at most"
)]
fn whole_pixels(value: f64) -> i32 {
    value.round() as i32
}

fn tile_and_asset(item: &glib::Object) -> Option<(PigouneAssetTile, PigouneAssetObject)> {
    let item = item.downcast_ref::<gtk::ListItem>()?;
    let tile = item.child().and_downcast::<PigouneAssetTile>()?;
    let asset = item.item().and_downcast::<PigouneAssetObject>()?;
    Some((tile, asset))
}

impl Default for PigouneAssetGrid {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{allowed_tile_size, next_larger_step, next_smaller_step};

    #[test]
    fn requested_sizes_stay_within_the_allowed_range() {
        assert_eq!(allowed_tile_size(272), 256);
        assert_eq!(allowed_tile_size(next_larger_step(256)), 256);
        assert_eq!(allowed_tile_size(next_smaller_step(64)), 64);
        assert_eq!(allowed_tile_size(40), 64);
        assert_eq!(allowed_tile_size(150), 150);
    }

    #[test]
    fn steps_land_on_multiples_of_32() {
        assert_eq!(next_larger_step(128), 160);
        assert_eq!(next_larger_step(100), 128);
        assert_eq!(next_smaller_step(128), 96);
        assert_eq!(next_smaller_step(100), 96);
    }
}
