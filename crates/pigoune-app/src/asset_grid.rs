use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use crate::asset_object::PigouneAssetObject;
use crate::asset_tile::PigouneAssetTile;

mod imp {
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::{gio, glib};

    use crate::asset_object::PigouneAssetObject;
    use crate::thumbnails::ThumbnailCache;

    #[derive(gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-grid.ui")]
    pub struct PigouneAssetGrid {
        #[template_child]
        pub grid_view: TemplateChild<gtk::GridView>,
        pub assets: gio::ListStore,
        pub thumbnails: Rc<ThumbnailCache>,
    }

    impl Default for PigouneAssetGrid {
        fn default() -> Self {
            Self {
                grid_view: TemplateChild::default(),
                assets: gio::ListStore::new::<PigouneAssetObject>(),
                thumbnails: Rc::default(),
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

    impl ObjectImpl for PigouneAssetGrid {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_up_grid();
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

impl PigouneAssetGrid {
    pub fn show_assets(&self, assets: &[PigouneAssetObject]) {
        let store = &self.imp().assets;
        store.splice(0, store.n_items(), assets);
    }

    pub fn forget_thumbnails(&self) {
        self.imp().thumbnails.forget_all();
    }

    fn set_up_grid(&self) {
        let imp = self.imp();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(Some(&PigouneAssetTile::new()));
            }
        });
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
        imp.grid_view
            .set_model(Some(&gtk::NoSelection::new(Some(imp.assets.clone()))));
    }
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
