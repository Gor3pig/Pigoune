use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib};

use crate::asset_facts;
use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const NOTHING_PAGE: &str = "nothing";
const ASSET_PAGE: &str = "asset";
const PREVIEW_PIXELS: u32 = 512;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-details.ui")]
    pub struct PigouneAssetDetails {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub preview: TemplateChild<gtk::Picture>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub format_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub dimensions_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub size_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub added_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub original_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub animation_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub embedded_row: TemplateChild<adw::ActionRow>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetDetails {
        const NAME: &'static str = "PigouneAssetDetails";
        type Type = super::PigouneAssetDetails;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneAssetDetails {}
    impl WidgetImpl for PigouneAssetDetails {}
    impl BinImpl for PigouneAssetDetails {}
}

glib::wrapper! {
    pub struct PigouneAssetDetails(ObjectSubclass<imp::PigouneAssetDetails>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneAssetDetails {
    pub fn show(&self, selected: Option<&PigouneAssetObject>, thumbnails: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        if let Some(asset) = selected {
            self.describe(asset);
            self.show_preview(asset, thumbnails);
            imp.stack.set_visible_child_name(ASSET_PAGE);
        } else {
            imp.preview.set_paintable(None::<&gdk::Paintable>);
            imp.stack.set_visible_child_name(NOTHING_PAGE);
        }
    }

    fn describe(&self, object: &PigouneAssetObject) {
        let imp = self.imp();
        let asset = object.asset();
        imp.name_label.set_label(&asset.display_name);
        imp.format_row
            .set_subtitle(asset_facts::format_name(asset.format));
        imp.dimensions_row
            .set_subtitle(&asset_facts::dimensions_text(asset.dimensions));
        imp.size_row
            .set_subtitle(&asset_facts::byte_size_text(asset.byte_size));
        imp.added_row
            .set_subtitle(&asset_facts::added_at_text(asset.added_at_unix_ms));
        imp.original_row.set_subtitle(&asset.original_file_name);
        imp.animation_row.set_visible(asset.is_animated);
        imp.embedded_row
            .set_visible(!asset.embedded_sizes.is_empty());
        imp.embedded_row
            .set_subtitle(&asset_facts::embedded_sizes_text(&asset.embedded_sizes));
    }

    fn show_preview(&self, object: &PigouneAssetObject, thumbnails: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        imp.preview
            .set_paintable(thumbnails.remembered(object.id()).as_ref());
        let file = object.file().to_path_buf();
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = details)]
            self,
            async move {
                if let Some(texture) = thumbnails::render(&file, PREVIEW_PIXELS, &details).await {
                    details.imp().preview.set_paintable(Some(&texture));
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }
}
