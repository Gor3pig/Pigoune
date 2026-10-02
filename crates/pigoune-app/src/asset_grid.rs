use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::asset_object::PigouneAssetObject;
use crate::asset_tile::PigouneAssetTile;
use crate::thumbnails::ThumbnailCache;

mod imp {
    use std::cell::Cell;
    use std::rc::Rc;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::{gio, glib};

    use super::{DEFAULT_TILE_SIZE, LARGEST_TILE_SIZE, SMALLEST_TILE_SIZE};
    use crate::asset_object::PigouneAssetObject;
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
                assets: gio::ListStore::new::<PigouneAssetObject>(),
                thumbnails: Rc::default(),
            }
        }
    }

    impl PigouneAssetGrid {
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

    pub fn thumbnails(&self) -> Rc<ThumbnailCache> {
        Rc::clone(&self.imp().thumbnails)
    }

    pub fn connect_selected_asset_changed(
        &self,
        callback: impl Fn(Option<PigouneAssetObject>) + 'static,
    ) {
        let Some(selection) = self
            .imp()
            .grid_view
            .model()
            .and_downcast::<gtk::SingleSelection>()
        else {
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
        let selection = gtk::SingleSelection::new(Some(imp.assets.clone()));
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
