use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const FADE_IN_MILLISECONDS: u32 = 200;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-tile.ui")]
    pub struct PigouneAssetTile {
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetTile {
        const NAME: &'static str = "PigouneAssetTile";
        type Type = super::PigouneAssetTile;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneAssetTile {}
    impl WidgetImpl for PigouneAssetTile {}
    impl BoxImpl for PigouneAssetTile {}
}

glib::wrapper! {
    pub struct PigouneAssetTile(ObjectSubclass<imp::PigouneAssetTile>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneAssetTile {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn picture(&self) -> gtk::Picture {
        self.imp().picture.get()
    }

    pub fn show_asset(&self, asset: &PigouneAssetObject, cache: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        imp.name_label.set_label(asset.display_name());
        self.set_tooltip_text(Some(asset.display_name()));

        if let Some(texture) = cache.remembered(asset.id()) {
            imp.picture.set_paintable(Some(&texture));
            imp.picture.set_opacity(1.0);
            return;
        }

        imp.picture.set_paintable(None::<&gdk::Paintable>);
        imp.picture.set_opacity(0.0);
        let id = asset.id();
        let file = asset.file().to_path_buf();
        let thumbnail_file = asset.thumbnail_file().to_path_buf();
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            #[strong]
            cache,
            async move {
                if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file, &tile).await {
                    cache.remember(id, texture.clone());
                    tile.fade_in(&texture);
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }

    pub fn forget_asset(&self) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        imp.picture.set_paintable(None::<&gdk::Paintable>);
    }

    fn fade_in(&self, texture: &gdk::Texture) {
        let picture = &self.imp().picture;
        picture.set_paintable(Some(texture));
        let target = adw::PropertyAnimationTarget::new(&**picture, "opacity");
        adw::TimedAnimation::new(&**picture, 0.0, 1.0, FADE_IN_MILLISECONDS, target).play();
    }
}

impl Default for PigouneAssetTile {
    fn default() -> Self {
        Self::new()
    }
}
