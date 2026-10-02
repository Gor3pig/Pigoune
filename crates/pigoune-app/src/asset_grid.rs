use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gdk, glib};

use pigoune_core::AssetId;

use crate::asset_object::PigouneAssetObject;
use crate::asset_sort::SortedAsset;
use crate::asset_tile::PigouneAssetTile;
use crate::thumbnails::ThumbnailCache;

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
        pub size_adjustment: TemplateChild<gtk::Adjustment>,
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
    }

    impl Default for PigouneAssetGrid {
        fn default() -> Self {
            Self {
                scrolled_window: TemplateChild::default(),
                grid_view: TemplateChild::default(),
                size_adjustment: TemplateChild::default(),
                tile_size: Cell::new(DEFAULT_TILE_SIZE),
                sort_criterion: RefCell::default(),
                sort_reversed: Cell::default(),
                sort_order: Rc::default(),
                sorter: RefCell::default(),
                assets: gio::ListStore::new::<PigouneAssetObject>(),
                thumbnails: Rc::default(),
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
            grid.link_size_scale();
            grid.resize_with_control_scroll();
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

const SMALLEST_TILE_SIZE: i32 = 64;
const LARGEST_TILE_SIZE: i32 = 256;
const DEFAULT_TILE_SIZE: i32 = 128;
const TILE_SIZE_STEP: i32 = 32;
const PIXELS_PER_SCROLL_STEP: f64 = 16.0;

impl PigouneAssetGrid {
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

    pub fn selected_asset(&self) -> Option<PigouneAssetObject> {
        self.selection()?
            .selected_item()
            .and_downcast::<PigouneAssetObject>()
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
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = grid)]
            self,
            move || grid.follow_selection()
        ));
    }

    pub fn select_asset(&self, id: AssetId) {
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
            selection.set_selected(position);
        }
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
        let selected = selection.selected();
        if selected == gtk::INVALID_LIST_POSITION {
            grid_view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
        } else {
            grid_view.scroll_to(selected, gtk::ListScrollFlags::FOCUS, None);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.imp().assets.n_items() == 0
    }

    pub fn thumbnails(&self) -> Rc<ThumbnailCache> {
        Rc::clone(&self.imp().thumbnails)
    }

    pub fn connect_selected_asset_changed(
        &self,
        callback: impl Fn(Option<PigouneAssetObject>) + 'static,
    ) {
        let Some(selection) = self.selection() else {
            return;
        };
        selection.connect_selected_item_notify(move |selection| {
            callback(
                selection
                    .selected_item()
                    .and_downcast::<PigouneAssetObject>(),
            );
        });
    }

    pub fn selection(&self) -> Option<gtk::SingleSelection> {
        self.imp()
            .grid_view
            .model()
            .and_downcast::<gtk::SingleSelection>()
    }

    pub fn connect_preview_requested(&self, callback: impl Fn() + 'static) {
        let callback = Rc::new(callback);
        let grid_view = &self.imp().grid_view;
        let on_activate = Rc::clone(&callback);
        grid_view.connect_activate(move |grid_view, position| {
            if let Some(selection) = grid_view.model().and_downcast::<gtk::SingleSelection>() {
                selection.set_selected(position);
            }
            on_activate();
        });
        let space = gtk::EventControllerKey::new();
        space.set_propagation_phase(gtk::PropagationPhase::Capture);
        space.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::space && modifiers.is_empty() {
                callback();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        grid_view.add_controller(space);
    }

    pub fn connect_rename_requested(&self, callback: impl Fn() + 'static) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::F2 && modifiers.is_empty() {
                callback();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.imp().grid_view.add_controller(keys);
    }

    pub fn reveal_selected(&self) {
        let grid_view = &self.imp().grid_view;
        if let Some(selection) = self.selection() {
            let position = selection.selected();
            if position != gtk::INVALID_LIST_POSITION {
                grid_view.scroll_to(position, gtk::ListScrollFlags::FOCUS, None);
            }
        }
    }

    pub fn forget_thumbnails(&self) {
        self.imp().thumbnails.forget_all();
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
        let selection = gtk::SingleSelection::new(Some(sorted_assets));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        selection.set_selected(gtk::INVALID_LIST_POSITION);
        imp.grid_view.set_model(Some(&selection));
    }
}

impl PigouneAssetGrid {
    fn link_size_scale(&self) {
        self.bind_property("tile-size", &*self.imp().size_adjustment, "value")
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
