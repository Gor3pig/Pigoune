use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib, graphene};

use crate::animation;
use crate::asset_facts;
use crate::asset_object::PigouneAssetObject;
use crate::square_space::PigouneSquareSpace;
use crate::thumbnails::{self, ThumbnailCache};

const FADE_IN_MILLISECONDS: u32 = 200;
const FAVORITE_BADGE_BASE_SIZE: i32 = 10;
const TILE_SIZE_PER_BADGE_PIXEL: i32 = 16;

pub fn favorite_badge_size(tile_size: i32) -> i32 {
    FAVORITE_BADGE_BASE_SIZE + tile_size.max(0) / TILE_SIZE_PER_BADGE_PIXEL
}

fn is_whole_image(thumbnail_width: i32, thumbnail_height: i32) -> bool {
    let reduced_side = i32::try_from(thumbnails::THUMBNAIL_PIXELS).unwrap_or(i32::MAX);
    thumbnail_width.max(thumbnail_height) < reduced_side
}

fn content_fit_for(thumbnail: &gdk::Texture) -> gtk::ContentFit {
    if is_whole_image(thumbnail.width(), thumbnail.height()) {
        gtk::ContentFit::ScaleDown
    } else {
        gtk::ContentFit::Contain
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::{gdk, glib};

    use crate::asset_object::PigouneAssetObject;
    use crate::square_space::PigouneSquareSpace;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-tile.ui")]
    #[properties(wrapper_type = super::PigouneAssetTile)]
    pub struct PigouneAssetTile {
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub thumbnail_space: TemplateChild<PigouneSquareSpace>,
        #[template_child]
        pub frame: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub animation_badge: TemplateChild<gtk::Label>,
        #[template_child]
        pub favorite_badge: TemplateChild<gtk::Image>,
        pub favorite_binding: RefCell<Option<glib::Binding>>,
        pub name_bindings: RefCell<Vec<glib::Binding>>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
        pub animation: RefCell<Option<glib::JoinHandle<()>>>,
        pub asset: RefCell<Option<PigouneAssetObject>>,
        pub still: RefCell<Option<gdk::Texture>>,
        #[property(get, set)]
        pub description: RefCell<String>,
        #[property(get, set)]
        pub animates_on_hover: Cell<bool>,
        #[property(get, set = Self::set_shows_format)]
        pub shows_format: Cell<bool>,
    }

    impl PigouneAssetTile {
        fn set_shows_format(&self, shows: bool) {
            self.shows_format.set(shows);
            self.obj().show_badge();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetTile {
        const NAME: &'static str = "PigouneAssetTile";
        type Type = super::PigouneAssetTile;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            PigouneSquareSpace::ensure_type();
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneAssetTile {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().animate_on_hover();
            self.obj().describe_for_screen_readers();
        }
    }
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

    pub fn frame(&self) -> gtk::Overlay {
        self.imp().frame.get()
    }

    pub fn thumbnail_space(&self) -> PigouneSquareSpace {
        self.imp().thumbnail_space.get()
    }

    pub fn name_label(&self) -> gtk::Label {
        self.imp().name_label.get()
    }

    pub fn favorite_badge(&self) -> gtk::Image {
        self.imp().favorite_badge.get()
    }

    fn show_badge(&self) {
        let imp = self.imp();
        let shown = imp.asset.borrow().clone();
        let Some(shown) = shown else {
            imp.animation_badge.set_visible(false);
            return;
        };
        let asset = shown.asset();
        let label = if asset.is_animated {
            asset_facts::animation_badge_text(asset.format)
        } else {
            asset_facts::format_name(asset.format)
        };
        imp.animation_badge.set_label(label);
        imp.animation_badge
            .set_visible(asset.is_animated || imp.shows_format.get());
    }

    pub fn show_asset(&self, asset: &PigouneAssetObject, cache: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        self.follow_name(asset);
        imp.asset.replace(Some(asset.clone()));
        self.show_badge();
        self.unbind_favorite();
        let binding = asset
            .bind_property("favorite", &*imp.favorite_badge, "visible")
            .sync_create()
            .build();
        imp.favorite_binding.replace(Some(binding));
        imp.asset.replace(Some(asset.clone()));
        self.update_description();

        if let Some(texture) = cache.remembered(asset.id()) {
            imp.picture.set_content_fit(content_fit_for(&texture));
            imp.picture.set_paintable(Some(&texture));
            imp.still.replace(Some(texture));
            imp.picture.set_opacity(1.0);
            return;
        }

        imp.picture.set_paintable(None::<&gdk::Paintable>);
        imp.picture.set_opacity(0.0);
        imp.still.replace(None);
        let id = asset.id();
        let file = asset.file().to_path_buf();
        let thumbnail_file = asset.thumbnail_file().to_path_buf();
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            #[strong]
            cache,
            async move {
                if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file).await {
                    cache.remember(id, texture.clone());
                    tile.fade_in(&texture);
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "thumbnail sides are small enough to be exact in f32"
    )]
    pub fn drawn_thumbnail(&self, target: &impl IsA<gtk::Widget>) -> Option<graphene::Rect> {
        let picture = self.imp().picture.get();
        let paintable = picture.paintable()?;
        let space = picture.compute_bounds(target)?;
        let width = paintable.intrinsic_width().max(1) as f32;
        let height = paintable.intrinsic_height().max(1) as f32;
        let scale = (space.width() / width)
            .min(space.height() / height)
            .min(1.0);
        Some(graphene::Rect::new(
            space.x() + (space.width() - width * scale) / 2.0,
            space.y() + (space.height() - height * scale) / 2.0,
            width * scale,
            height * scale,
        ))
    }

    pub fn asset(&self) -> Option<PigouneAssetObject> {
        self.imp().asset.borrow().clone()
    }

    pub fn forget_asset(&self) {
        let imp = self.imp();
        self.stop_animation();
        self.unbind_favorite();
        imp.asset.replace(None);
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        imp.picture.set_paintable(None::<&gdk::Paintable>);
    }

    fn follow_name(&self, asset: &PigouneAssetObject) {
        let imp = self.imp();
        for binding in imp.name_bindings.take() {
            binding.unbind();
        }
        let label = asset
            .bind_property("display-name", &*imp.name_label, "label")
            .sync_create()
            .build();
        let tooltip = asset
            .bind_property("display-name", self, "tooltip-text")
            .sync_create()
            .build();
        imp.name_bindings.replace(vec![label, tooltip]);
    }

    fn describe_for_screen_readers(&self) {
        let imp = self.imp();
        imp.name_label.connect_label_notify(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            move |_| tile.update_description()
        ));
        imp.favorite_badge.connect_visible_notify(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            move |_| tile.update_description()
        ));
    }

    fn update_description(&self) {
        let imp = self.imp();
        let Some(asset) = imp.asset.borrow().clone() else {
            return;
        };
        let mut parts = vec![
            imp.name_label.label().to_string(),
            asset_facts::format_name(asset.asset().format).to_owned(),
        ];
        if imp.favorite_badge.is_visible() {
            parts.push(gettext("favorite"));
        }
        self.set_description(parts.join(", "));
    }

    fn unbind_favorite(&self) {
        if let Some(binding) = self.imp().favorite_binding.take() {
            binding.unbind();
        }
    }

    fn animate_on_hover(&self) {
        let hover = gtk::EventControllerMotion::new();
        hover.connect_enter(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            move |_, _, _| tile.start_animation()
        ));
        hover.connect_leave(glib::clone!(
            #[weak(rename_to = tile)]
            self,
            move |_| tile.stop_animation()
        ));
        self.add_controller(hover);
    }

    fn start_animation(&self) {
        let imp = self.imp();
        let Some(asset) = imp.asset.borrow().clone() else {
            return;
        };
        let system_allows =
            gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
        if !imp.animates_on_hover.get()
            || !system_allows
            || !asset.asset().is_animated
            || imp.animation.borrow().is_some()
        {
            return;
        }
        let picture = imp.picture.get();
        let playing = animation::play(asset.file().to_path_buf(), move |frame| {
            picture.set_paintable(Some(frame));
            picture.set_opacity(1.0);
        });
        imp.animation.replace(Some(playing));
    }

    fn stop_animation(&self) {
        let imp = self.imp();
        let Some(playing) = imp.animation.take() else {
            return;
        };
        playing.abort();
        imp.picture.set_paintable(imp.still.borrow().as_ref());
    }

    fn fade_in(&self, texture: &gdk::Texture) {
        let imp = self.imp();
        imp.still.replace(Some(texture.clone()));
        if imp.animation.borrow().is_some() {
            return;
        }
        let picture = &imp.picture;
        picture.set_content_fit(content_fit_for(texture));
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

#[cfg(test)]
mod tests {
    use super::{favorite_badge_size, is_whole_image};

    #[test]
    fn small_images_are_shown_whole_and_never_enlarged() {
        assert!(is_whole_image(16, 16));
        assert!(is_whole_image(200, 120));
    }

    #[test]
    fn reduced_copies_of_large_images_fill_their_frame() {
        assert!(!is_whole_image(256, 40));
        assert!(!is_whole_image(128, 256));
    }

    #[test]
    fn the_favorite_badge_grows_with_the_tiles() {
        assert_eq!(favorite_badge_size(64), 14);
        assert_eq!(favorite_badge_size(128), 18);
        assert_eq!(favorite_badge_size(256), 26);
    }

    #[test]
    fn the_favorite_badge_never_vanishes() {
        assert_eq!(favorite_badge_size(0), 10);
        assert_eq!(favorite_badge_size(-32), 10);
    }
}
