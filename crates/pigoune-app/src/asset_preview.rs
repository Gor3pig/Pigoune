use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};

use pigoune_core::{AssetFormat, Dimensions};

use crate::animation;
use crate::asset_facts;
use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const BACKGROUNDS: [&str; 5] = ["transparent", "white", "grey", "black", "checkerboard"];
const SMALLEST_RENDER_PIXELS: u32 = 256;
const LARGEST_VECTOR_PIXELS: u32 = 4096;
const SHARPEN_DELAY: Duration = Duration::from_millis(200);
const STEP_BUTTONS_DELAY: Duration = Duration::from_secs(2);
const SHOWN_STEP_BUTTON: &str = "shown";

type ClosedCallback = Box<dyn Fn(Option<PigouneAssetObject>)>;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ClosedCallback;
    use crate::asset_object::PigouneAssetObject;
    use crate::thumbnails::ThumbnailCache;
    use crate::zoom_view::PigouneZoomView;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-preview.ui")]
    #[properties(wrapper_type = super::PigouneAssetPreview)]
    pub struct PigouneAssetPreview {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub preview_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub background_button_swatch: TemplateChild<gtk::Box>,
        #[template_child]
        pub background_popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub background_swatches: TemplateChild<gtk::Box>,
        #[template_child]
        pub surface: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_view: TemplateChild<PigouneZoomView>,
        #[template_child]
        pub zoom_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,
        pub step_buttons_shown: Cell<bool>,
        pub step_buttons_hiding: RefCell<Option<glib::SourceId>>,
        pub pointer: Cell<Option<(f64, f64)>>,
        pub showing: RefCell<Option<PigouneAssetObject>>,
        pub vector_pixels: Cell<u32>,
        pub sharpening: RefCell<Option<glib::SourceId>>,
        #[property(get, set = Self::set_background)]
        pub background: RefCell<String>,
        #[property(get, set = Self::set_compact)]
        pub compact: Cell<bool>,
        pub items: RefCell<Vec<PigouneAssetObject>>,
        pub position: Cell<u32>,
        pub thumbnails: RefCell<Option<Rc<ThumbnailCache>>>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
        pub animation: RefCell<Option<glib::JoinHandle<()>>>,
        pub on_closed: RefCell<Option<ClosedCallback>>,
    }

    impl PigouneAssetPreview {
        fn set_background(&self, background: String) {
            for known in super::BACKGROUNDS {
                self.surface.remove_css_class(known);
                self.background_button_swatch.remove_css_class(known);
            }
            self.surface.add_css_class(&background);
            self.background_button_swatch.add_css_class(&background);
            self.background_popover.popdown();
            self.background.replace(background);
        }

        fn set_compact(&self, compact: bool) {
            if self.compact.replace(compact) == compact {
                return;
            }
            let swatches = self.background_swatches.get();
            if compact {
                self.header_bar.remove(&swatches);
                self.background_popover.set_child(Some(&swatches));
            } else {
                self.background_popover.set_child(None::<&gtk::Widget>);
                self.header_bar.pack_end(&swatches);
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetPreview {
        const NAME: &'static str = "PigouneAssetPreview";
        type Type = super::PigouneAssetPreview;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            PigouneZoomView::ensure_type();
            class.bind_template();
            class.install_action("preview.zoom-fit", None, |preview, _, _| {
                preview.imp().zoom_view.fit_to_view();
            });
            class.install_action("preview.zoom-actual", None, |preview, _, _| {
                preview.imp().zoom_view.show_actual_size();
            });
            for key in [gdk::Key::plus, gdk::Key::equal, gdk::Key::KP_Add] {
                class.add_binding(key, gdk::ModifierType::empty(), |preview| {
                    preview.imp().zoom_view.zoom_in();
                    glib::Propagation::Stop
                });
            }
            for key in [gdk::Key::minus, gdk::Key::KP_Subtract] {
                class.add_binding(key, gdk::ModifierType::empty(), |preview| {
                    preview.imp().zoom_view.zoom_out();
                    glib::Propagation::Stop
                });
            }
            for key in [gdk::Key::_0, gdk::Key::KP_0] {
                class.add_binding(key, gdk::ModifierType::empty(), |preview| {
                    preview.imp().zoom_view.fit_to_view();
                    glib::Propagation::Stop
                });
            }
            for key in [gdk::Key::_1, gdk::Key::KP_1] {
                class.add_binding(key, gdk::ModifierType::empty(), |preview| {
                    preview.imp().zoom_view.show_actual_size();
                    glib::Propagation::Stop
                });
            }
            class.bind_template_instance_callbacks();
            class.add_binding(gdk::Key::Escape, gdk::ModifierType::empty(), |preview| {
                preview.close();
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::space, gdk::ModifierType::empty(), |preview| {
                preview.close();
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Left, gdk::ModifierType::empty(), |preview| {
                preview.step(-1);
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Right, gdk::ModifierType::empty(), |preview| {
                preview.step(1);
                glib::Propagation::Stop
            });
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneAssetPreview {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().follow_zoom();
            self.obj().follow_pointer();
        }
    }
    impl WidgetImpl for PigouneAssetPreview {}
    impl BinImpl for PigouneAssetPreview {}
}

glib::wrapper! {
    pub struct PigouneAssetPreview(ObjectSubclass<imp::PigouneAssetPreview>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PigouneAssetPreview {
    pub fn connect_closed(&self, callback: impl Fn(Option<PigouneAssetObject>) + 'static) {
        self.imp().on_closed.replace(Some(Box::new(callback)));
    }

    pub fn open(
        &self,
        items: Vec<PigouneAssetObject>,
        position: u32,
        thumbnails: Rc<ThumbnailCache>,
    ) {
        let imp = self.imp();
        self.forget_selection();
        imp.thumbnails.replace(Some(thumbnails));
        imp.items.replace(items);
        imp.position.set(position);
        self.show_current();
        self.grab_focus();
    }

    pub fn close(&self) {
        let last = self.imp().showing.borrow().clone();
        self.forget_selection();
        if let Some(on_closed) = self.imp().on_closed.borrow().as_ref() {
            on_closed(last);
        }
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_previous_clicked(&self) {
        self.step(-1);
    }

    #[template_callback]
    fn on_next_clicked(&self) {
        self.step(1);
    }

    fn follow_pointer(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_, x, y| {
                let moved = preview.imp().pointer.replace(Some((x, y))) != Some((x, y));
                if moved {
                    preview.show_step_buttons();
                }
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_| {
                preview.imp().pointer.set(None);
                preview.hide_step_buttons();
            }
        ));
        self.add_controller(motion);
    }

    fn show_step_buttons(&self) {
        let imp = self.imp();
        imp.step_buttons_shown.set(true);
        self.update_step_buttons();
        if let Some(hiding) = imp.step_buttons_hiding.take() {
            hiding.remove();
        }
        let hiding = glib::timeout_add_local_once(
            STEP_BUTTONS_DELAY,
            glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move || {
                    preview.imp().step_buttons_hiding.replace(None);
                    if preview.is_hovering_a_step_button() {
                        preview.show_step_buttons();
                    } else {
                        preview.hide_step_buttons();
                    }
                }
            ),
        );
        imp.step_buttons_hiding.replace(Some(hiding));
    }

    fn hide_step_buttons(&self) {
        let imp = self.imp();
        if let Some(hiding) = imp.step_buttons_hiding.take() {
            hiding.remove();
        }
        imp.step_buttons_shown.set(false);
        self.update_step_buttons();
    }

    fn is_hovering_a_step_button(&self) -> bool {
        let imp = self.imp();
        [&*imp.previous_button, &*imp.next_button]
            .iter()
            .any(|button| button.state_flags().contains(gtk::StateFlags::PRELIGHT))
    }

    fn update_step_buttons(&self) {
        let imp = self.imp();
        let shown = imp.step_buttons_shown.get();
        let position = imp.position.get();
        let count = u32::try_from(imp.items.borrow().len()).unwrap_or(u32::MAX);
        for (button, offset) in [(&*imp.previous_button, -1), (&*imp.next_button, 1)] {
            let available = shown && neighbour(position, offset, count).is_some();
            if available {
                button.add_css_class(SHOWN_STEP_BUTTON);
            } else {
                button.remove_css_class(SHOWN_STEP_BUTTON);
            }
            button.set_can_target(available);
        }
    }

    fn forget_selection(&self) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        self.stop_animation();
        imp.items.replace(Vec::new());
        if let Some(sharpening) = imp.sharpening.take() {
            sharpening.remove();
        }
        imp.showing.replace(None);
        imp.zoom_view.show_image(None, 1, 1, false);
        self.hide_step_buttons();
    }

    fn step(&self, offset: i32) {
        let imp = self.imp();
        let count = u32::try_from(imp.items.borrow().len()).unwrap_or(u32::MAX);
        if let Some(position) = neighbour(imp.position.get(), offset, count) {
            imp.position.set(position);
            self.show_current();
        }
    }

    fn show_current(&self) {
        let imp = self.imp();
        let position = imp.position.get();
        let (asset, count) = {
            let items = imp.items.borrow();
            let asset = usize::try_from(position)
                .ok()
                .and_then(|index| items.get(index))
                .cloned();
            (asset, u32::try_from(items.len()).unwrap_or(u32::MAX))
        };
        let Some(asset) = asset else {
            self.close();
            return;
        };
        imp.preview_title.set_title(&asset.display_name());
        imp.preview_title
            .set_subtitle(&subtitle_text(&asset, position, count));
        self.update_step_buttons();
        let remembered = imp
            .thumbnails
            .borrow()
            .as_ref()
            .and_then(|thumbnails| thumbnails.remembered(asset.id()));
        let (width, height) = placeholder_size(&asset, remembered.as_ref());
        let is_vector = asset.asset().format == AssetFormat::Svg;
        imp.zoom_view
            .show_image(remembered.as_ref(), width, height, is_vector);
        imp.showing.replace(Some(asset.clone()));
        self.stop_animation();
        self.load(&asset, self.render_pixels(), true);
    }

    fn play_animation(&self, asset: &PigouneAssetObject) {
        let zoom_view = self.imp().zoom_view.get();
        let playing = animation::play(asset.file().to_path_buf(), move |frame| {
            zoom_view.replace_texture(frame);
        });
        self.imp().animation.replace(Some(playing));
    }

    fn stop_animation(&self) {
        if let Some(playing) = self.imp().animation.take() {
            playing.abort();
        }
    }

    fn load(&self, asset: &PigouneAssetObject, vector_pixels: u32, first_view: bool) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        let file = asset.file().to_path_buf();
        let animated = asset.asset().is_animated.then(|| asset.clone());
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            async move {
                let Some(detailed) = thumbnails::load_detailed(&file, vector_pixels).await else {
                    return;
                };
                if let Some(animated) = animated.as_ref() {
                    preview.play_animation(animated);
                }
                let imp = preview.imp();
                imp.vector_pixels.set(vector_pixels);
                if first_view {
                    imp.zoom_view.show_image(
                        Some(&detailed.texture),
                        detailed.width,
                        detailed.height,
                        detailed.is_vector,
                    );
                } else {
                    imp.zoom_view.replace_texture(&detailed.texture);
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }

    fn follow_zoom(&self) {
        let imp = self.imp();
        imp.zoom_button.set_label(&zoom_text(1.0));
        imp.zoom_view.connect_zoom_changed(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |zoom| {
                preview.imp().zoom_button.set_label(&zoom_text(zoom));
                preview.sharpen_later(zoom);
            }
        ));
    }

    fn sharpen_later(&self, zoom: f64) {
        let imp = self.imp();
        if let Some(sharpening) = imp.sharpening.take() {
            sharpening.remove();
        }
        let Some(asset) = imp.showing.borrow().clone() else {
            return;
        };
        if asset.asset().format != AssetFormat::Svg {
            return;
        }
        let wanted = vector_pixels_for(asset.asset().dimensions, zoom, self.scale_factor());
        if wanted <= imp.vector_pixels.get() {
            return;
        }
        let source = glib::timeout_add_local_once(
            SHARPEN_DELAY,
            glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move || {
                    preview.imp().sharpening.replace(None);
                    preview.load(&asset, wanted, false);
                }
            ),
        );
        imp.sharpening.replace(Some(source));
    }

    fn render_pixels(&self) -> u32 {
        let longest_side = self
            .root()
            .map_or(0, |root| root.width().max(root.height()));
        u32::try_from(longest_side * self.scale_factor())
            .unwrap_or(0)
            .max(SMALLEST_RENDER_PIXELS)
    }
}

impl Default for PigouneAssetPreview {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn placeholder_size(asset: &PigouneAssetObject, thumbnail: Option<&gdk::Texture>) -> (u32, u32) {
    asset
        .asset()
        .dimensions
        .map(|dimensions| (dimensions.width(), dimensions.height()))
        .or_else(|| {
            thumbnail.map(|texture| {
                (
                    u32::try_from(texture.width()).unwrap_or(1),
                    u32::try_from(texture.height()).unwrap_or(1),
                )
            })
        })
        .unwrap_or((1, 1))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the result is clamped to a few thousand pixels"
)]
fn vector_pixels_for(dimensions: Option<Dimensions>, zoom: f64, scale_factor: i32) -> u32 {
    let longest = dimensions.map_or(SMALLEST_RENDER_PIXELS, |dimensions| {
        dimensions.width().max(dimensions.height())
    });
    let wanted = (f64::from(longest) * zoom * f64::from(scale_factor)).ceil();
    (wanted.max(0.0) as u32).clamp(SMALLEST_RENDER_PIXELS, LARGEST_VECTOR_PIXELS)
}

fn zoom_text(zoom: f64) -> String {
    gettext("{percent}%").replace("{percent}", &format!("{:.0}", zoom * 100.0))
}

fn neighbour(current: u32, offset: i32, count: u32) -> Option<u32> {
    if current == gtk::INVALID_LIST_POSITION {
        return None;
    }
    let target = i64::from(current) + i64::from(offset);
    u32::try_from(target)
        .ok()
        .filter(|position| *position < count)
}

fn subtitle_text(asset: &PigouneAssetObject, position: u32, count: u32) -> String {
    [
        position_text(position, count),
        asset_facts::summary_text(asset.asset(), None),
    ]
    .join(asset_facts::SUMMARY_SEPARATOR)
}

fn position_text(position: u32, count: u32) -> String {
    gettext("{position} of {count}")
        .replace("{position}", &(position + 1).to_string())
        .replace("{count}", &count.to_string())
}

#[cfg(test)]
mod tests {
    use pigoune_core::Dimensions;

    use super::{neighbour, vector_pixels_for};

    #[test]
    fn drawings_are_redrawn_for_the_zoom_within_limits() {
        let icon = Dimensions::new(100, 50);
        assert_eq!(vector_pixels_for(icon, 8.0, 1), 800);
        assert_eq!(vector_pixels_for(icon, 8.0, 2), 1600);
        assert_eq!(vector_pixels_for(icon, 1.0, 1), 256);
        assert_eq!(vector_pixels_for(icon, 100.0, 1), 4096);
    }

    #[test]
    fn navigation_moves_by_one_inside_the_grid() {
        assert_eq!(neighbour(3, 1, 10), Some(4));
        assert_eq!(neighbour(3, -1, 10), Some(2));
    }

    #[test]
    fn navigation_stops_at_both_ends() {
        assert_eq!(neighbour(0, -1, 10), None);
        assert_eq!(neighbour(9, 1, 10), None);
    }

    #[test]
    fn navigation_needs_a_selection() {
        assert_eq!(neighbour(gtk::INVALID_LIST_POSITION, 1, 10), None);
    }
}
