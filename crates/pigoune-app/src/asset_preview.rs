use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use pigoune_core::{Asset, AssetFormat, Dimensions};

use crate::animation;
use crate::asset_facts;
use crate::asset_object::PigouneAssetObject;
use crate::swipe_steps;
use crate::thumbnails::{self, ThumbnailCache};

const BACKGROUNDS: [&str; 5] = ["transparent", "white", "grey", "black", "checkerboard"];
const SMALLEST_RENDER_PIXELS: u32 = 256;
const LARGEST_VECTOR_PIXELS: u32 = 4096;
const SHARPEN_DELAY: Duration = Duration::from_millis(200);
const CONTROLS_DELAY: Duration = Duration::from_secs(2);
const SHOWN_STEP_BUTTON: &str = "shown";
const FAVORITE_STYLE: &str = "favorite";
const FLOATING_HEADER: &str = "floating";
const MOUSE_BACK_BUTTON: u32 = 8;
const MOUSE_FORWARD_BUTTON: u32 = 9;

type ClosedCallback = Box<dyn Fn(Option<PigouneAssetObject>)>;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ClosedCallback;
    use crate::asset_object::PigouneAssetObject;
    use crate::swipe_steps::SwipeSteps;
    use crate::thumbnails::ThumbnailCache;
    use crate::zoom_view::PigouneZoomView;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-preview.ui")]
    #[properties(wrapper_type = super::PigouneAssetPreview)]
    pub struct PigouneAssetPreview {
        #[template_child]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,
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
        pub favorite_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub icon_sizes: TemplateChild<gtk::Box>,
        #[template_child]
        pub context_menu: TemplateChild<gtk::PopoverMenu>,
        pub swipe: RefCell<SwipeSteps>,
        pub watched_favorite: RefCell<Option<(PigouneAssetObject, glib::SignalHandlerId)>>,
        #[property(get, set)]
        pub actionable: Cell<bool>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,
        pub controls_shown: Cell<bool>,
        pub follows_fullscreen: Cell<bool>,
        pub controls_hiding: RefCell<Option<glib::SourceId>>,
        pub pointer: Cell<Option<(f64, f64)>>,
        pub showing: RefCell<Option<PigouneAssetObject>>,
        pub vector_pixels: Cell<u32>,
        pub sharpening: RefCell<Option<glib::SourceId>>,
        #[property(get, set = Self::set_background)]
        pub background: RefCell<String>,
        #[property(get, set = Self::set_compact)]
        pub compact: Cell<bool>,
        #[property(get, set = Self::set_show_bounds)]
        pub show_bounds: Cell<bool>,
        #[property(get, set = Self::set_show_pixel_grid)]
        pub show_pixel_grid: Cell<bool>,
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
            self.zoom_view.queue_draw();
            self.background_popover.popdown();
            self.background.replace(background);
        }

        fn set_show_bounds(&self, show_bounds: bool) {
            self.show_bounds.set(show_bounds);
            self.zoom_view.set_shows_bounds(show_bounds);
        }

        fn set_show_pixel_grid(&self, show_pixel_grid: bool) {
            self.show_pixel_grid.set(show_pixel_grid);
            self.zoom_view.set_shows_pixel_grid(show_pixel_grid);
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
            class.install_action("preview.zoom-in", None, |preview, _, _| {
                preview.imp().zoom_view.zoom_in();
            });
            class.install_action("preview.zoom-out", None, |preview, _, _| {
                preview.imp().zoom_view.zoom_out();
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
                if preview.is_fullscreen() {
                    preview.set_fullscreen(false);
                } else {
                    preview.close();
                }
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::F11, gdk::ModifierType::empty(), |preview| {
                preview.set_fullscreen(!preview.is_fullscreen());
                glib::Propagation::Stop
            });
            class.install_action("preview.fullscreen", None, |preview, _, _| {
                preview.set_fullscreen(!preview.is_fullscreen());
            });
            class.add_binding(gdk::Key::space, gdk::ModifierType::empty(), |preview| {
                preview.close();
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Menu, gdk::ModifierType::empty(), |preview| {
                preview.show_context_menu(None);
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::F10, gdk::ModifierType::SHIFT_MASK, |preview| {
                preview.show_context_menu(None);
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Home, gdk::ModifierType::empty(), |preview| {
                preview.show_position(0);
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::End, gdk::ModifierType::empty(), |preview| {
                preview.show_last();
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
            self.obj().listen_to_menu_requests();
            self.obj().listen_to_navigation();
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
        self.follow_fullscreen();
        self.grab_focus();
    }

    pub fn shown_asset(&self) -> Option<PigouneAssetObject> {
        self.imp().showing.borrow().clone()
    }

    pub fn close(&self) {
        self.set_fullscreen(false);
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

    fn offer_icon_sizes(&self, asset: &PigouneAssetObject) {
        let sizes_box = &*self.imp().icon_sizes;
        while let Some(button) = sizes_box.first_child() {
            sizes_box.remove(&button);
        }
        let sizes = &asset.asset().embedded_sizes;
        sizes_box.set_visible(sizes.len() > 1);
        if sizes.len() < 2 {
            return;
        }
        let largest = sizes.last().copied();
        let mut group: Option<gtk::ToggleButton> = None;
        for size in sizes.iter().copied() {
            let button = gtk::ToggleButton::builder()
                .label(icon_size_label(size))
                .tooltip_text(asset_facts::dimensions_text(Some(size)))
                .focus_on_click(false)
                .can_focus(false)
                .css_classes(["osd", "numeric", "icon-size"])
                .active(Some(size) == largest)
                .build();
            button.set_group(group.as_ref());
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = preview)]
                self,
                #[weak]
                asset,
                move |button| {
                    if button.is_active() {
                        preview.show_icon_size(&asset, size);
                    }
                }
            ));
            sizes_box.append(&button);
            group.get_or_insert(button);
        }
    }

    fn show_icon_size(&self, asset: &PigouneAssetObject, size: Dimensions) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        let file = asset.file().to_path_buf();
        let shown = asset_at_size(asset.asset(), size);
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            async move {
                let Some(icon) = thumbnails::load_icon_size(&file, size).await else {
                    return;
                };
                let imp = preview.imp();
                imp.zoom_view
                    .show_image(Some(&icon.texture), icon.width, icon.height, false);
                imp.preview_title.set_subtitle(&subtitle_text(
                    &shown,
                    imp.position.get(),
                    preview.count(),
                ));
            }
        ));
        imp.loading.replace(Some(loading));
    }

    fn watch_favorite(&self, asset: &PigouneAssetObject) {
        self.unwatch_favorite();
        let handler = asset.connect_notify_local(
            Some("favorite"),
            glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move |asset, _| preview.show_favorite(asset.favorite())
            ),
        );
        self.imp()
            .watched_favorite
            .replace(Some((asset.clone(), handler)));
        self.show_favorite(asset.favorite());
    }

    fn unwatch_favorite(&self) {
        if let Some((asset, handler)) = self.imp().watched_favorite.take() {
            asset.disconnect(handler);
        }
    }

    fn show_favorite(&self, favorite: bool) {
        let button = &self.imp().favorite_button;
        if favorite {
            button.set_icon_name("starred-symbolic");
            button.set_tooltip_text(Some(&gettext("Remove from Favorites")));
            button.add_css_class(FAVORITE_STYLE);
        } else {
            button.set_icon_name("non-starred-symbolic");
            button.set_tooltip_text(Some(&gettext("Add to Favorites")));
            button.remove_css_class(FAVORITE_STYLE);
        }
    }

    fn listen_to_menu_requests(&self) {
        let zoom_view = &*self.imp().zoom_view;
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_, _, x, y| preview.show_context_menu(Some((x, y)))
        ));
        zoom_view.add_controller(click);
        let long_press = gtk::GestureLongPress::builder().touch_only(true).build();
        long_press.connect_pressed(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_, x, y| preview.show_context_menu(Some((x, y)))
        ));
        zoom_view.add_controller(long_press);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "points inside the preview fit in an i32"
    )]
    fn show_context_menu(&self, point: Option<(f64, f64)>) {
        let imp = self.imp();
        let Some(asset) = imp.showing.borrow().clone() else {
            return;
        };
        if !imp.actionable.get() {
            return;
        }
        let zoom_view = &*imp.zoom_view;
        let (x, y) = point.unwrap_or_else(|| {
            (
                f64::from(zoom_view.width()) / 2.0,
                f64::from(zoom_view.height()) / 2.0,
            )
        });
        let Some(inside) = zoom_view.compute_point(
            &*imp.surface,
            &gtk::graphene::Point::new(x as f32, y as f32),
        ) else {
            return;
        };
        let pointing_to = gdk::Rectangle::new(inside.x() as i32, inside.y() as i32, 1, 1);
        imp.context_menu
            .set_menu_model(Some(&context_menu_model(asset.favorite())));
        imp.context_menu.set_pointing_to(Some(&pointing_to));
        imp.context_menu.popup();
    }

    fn follow_pointer(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_, x, y| {
                let moved = preview.imp().pointer.replace(Some((x, y))) != Some((x, y));
                if moved {
                    preview.show_controls();
                }
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_| {
                preview.imp().pointer.set(None);
                preview.hide_controls();
            }
        ));
        self.add_controller(motion);
    }

    fn show_controls(&self) {
        let imp = self.imp();
        imp.controls_shown.set(true);
        self.update_controls();
        if let Some(hiding) = imp.controls_hiding.take() {
            hiding.remove();
        }
        let hiding = glib::timeout_add_local_once(
            CONTROLS_DELAY,
            glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move || {
                    preview.imp().controls_hiding.replace(None);
                    if preview.is_holding_controls() {
                        preview.show_controls();
                    } else {
                        preview.hide_controls();
                    }
                }
            ),
        );
        imp.controls_hiding.replace(Some(hiding));
    }

    fn hide_controls(&self) {
        let imp = self.imp();
        if let Some(hiding) = imp.controls_hiding.take() {
            hiding.remove();
        }
        imp.controls_shown.set(false);
        self.update_controls();
    }

    fn is_holding_controls(&self) -> bool {
        let imp = self.imp();
        let hovered = [
            imp.previous_button.upcast_ref::<gtk::Widget>(),
            imp.next_button.upcast_ref(),
            imp.header_bar.upcast_ref(),
        ]
        .iter()
        .any(|widget| widget.state_flags().contains(gtk::StateFlags::PRELIGHT));
        let menu_open = [
            imp.zoom_button.popover(),
            Some(imp.background_popover.get().upcast()),
            Some(imp.context_menu.get().upcast()),
        ]
        .iter()
        .flatten()
        .any(WidgetExt::is_visible);
        hovered || menu_open
    }

    fn window(&self) -> Option<gtk::Window> {
        self.root().and_downcast::<gtk::Window>()
    }

    fn is_fullscreen(&self) -> bool {
        self.window().is_some_and(|window| window.is_fullscreen())
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        let Some(window) = self.window() else {
            return;
        };
        if fullscreen {
            window.fullscreen();
        } else if window.is_fullscreen() {
            window.unfullscreen();
        }
    }

    fn float_header(&self, floating: bool) {
        let imp = self.imp();
        imp.toolbar_view.set_extend_content_to_top_edge(floating);
        if floating {
            imp.header_bar.add_css_class(FLOATING_HEADER);
        } else {
            imp.header_bar.remove_css_class(FLOATING_HEADER);
        }
        self.update_controls();
    }

    fn follow_fullscreen(&self) {
        let imp = self.imp();
        if imp.follows_fullscreen.replace(true) {
            return;
        }
        if let Some(window) = self.window() {
            window.connect_fullscreened_notify(glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move |window| preview.float_header(window.is_fullscreen())
            ));
        }
    }

    fn update_controls(&self) {
        let imp = self.imp();
        let shown = imp.controls_shown.get();
        imp.toolbar_view
            .set_reveal_top_bars(shown || !self.is_fullscreen());
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
        self.hide_controls();
        self.unwatch_favorite();
    }

    fn step(&self, offset: i32) {
        let count = self.count();
        if let Some(position) = neighbour(self.imp().position.get(), offset, count) {
            self.show_position(position);
        }
    }

    fn show_last(&self) {
        if let Some(last) = self.count().checked_sub(1) {
            self.show_position(last);
        }
    }

    fn show_position(&self, position: u32) {
        let imp = self.imp();
        if position < self.count() && position != imp.position.get() {
            imp.position.set(position);
            self.show_current();
        }
    }

    fn count(&self) -> u32 {
        u32::try_from(self.imp().items.borrow().len()).unwrap_or(u32::MAX)
    }

    fn listen_to_navigation(&self) {
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        scroll.connect_scroll_begin(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_| preview.imp().swipe.borrow_mut().ended()
        ));
        scroll.connect_scroll_end(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |_| preview.imp().swipe.borrow_mut().ended()
        ));
        scroll.connect_scroll(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |controller, horizontal, vertical| {
                if horizontal.abs() <= vertical.abs() {
                    return glib::Propagation::Proceed;
                }
                let offset = if controller.unit() == gdk::ScrollUnit::Wheel {
                    swipe_steps::wheel_tilt_step(horizontal)
                } else {
                    preview.imp().swipe.borrow_mut().swiped(horizontal)
                };
                if let Some(offset) = offset {
                    preview.step(offset);
                }
                glib::Propagation::Stop
            }
        ));
        self.add_controller(scroll);
        for (button, offset) in [(MOUSE_BACK_BUTTON, -1), (MOUSE_FORWARD_BUTTON, 1)] {
            let click = gtk::GestureClick::builder().button(button).build();
            click.connect_pressed(glib::clone!(
                #[weak(rename_to = preview)]
                self,
                move |_, _, _, _| preview.step(offset)
            ));
            self.add_controller(click);
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
            .set_subtitle(&subtitle_text(asset.asset(), position, count));
        self.update_controls();
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
        self.offer_icon_sizes(&asset);
        self.watch_favorite(&asset);
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

fn context_menu_model(favorite: bool) -> gio::MenuModel {
    let sharing = gio::Menu::new();
    let copy = gio::MenuItem::new(Some(&gettext("Copy")), Some("win.copy-selected"));
    copy.set_attribute_value("accel", Some(&"<Control>c".to_variant()));
    sharing.append_item(&copy);
    sharing.append(Some(&gettext("Open With…")), Some("win.open-with"));
    sharing.append(Some(&gettext("Export To…")), Some("win.export-selected"));
    let organizing = gio::Menu::new();
    let favorite_label = if favorite {
        gettext("Remove from Favorites")
    } else {
        gettext("Add to Favorites")
    };
    organizing.append(Some(&favorite_label), Some("win.toggle-favorite"));
    let menu = gio::Menu::new();
    menu.append_section(None, &sharing);
    menu.append_section(None, &organizing);
    menu.upcast()
}

fn icon_size_label(size: Dimensions) -> String {
    if size.width() == size.height() {
        size.width().to_string()
    } else {
        format!("{} × {}", size.width(), size.height())
    }
}

fn subtitle_text(asset: &Asset, position: u32, count: u32) -> String {
    [
        position_text(position, count),
        asset_facts::summary_text(asset, None),
    ]
    .join(asset_facts::SUMMARY_SEPARATOR)
}

fn asset_at_size(asset: &Asset, size: Dimensions) -> Asset {
    Asset {
        dimensions: Some(size),
        ..asset.clone()
    }
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
