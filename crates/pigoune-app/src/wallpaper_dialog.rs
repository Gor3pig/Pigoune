use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::{AssetColor, Dimensions, Rgb};

use crate::asset_colors;
use crate::desktop_bars::Desktop;
use crate::screen_size::{self, ScreenChoice};
use crate::thumbnails;
use crate::wallpaper_framing::{ACTUAL_SCALE, Backdrop, Framing, LARGEST_DARKNESS, Look};
use crate::wallpaper_stage::PigouneWallpaperStage;
use crate::wallpaper_swatches::ColorSwatches;
use crate::zoom_math::Size;

const SHOWN_VECTOR_PIXELS: u32 = 2048;
const BUTTON_ZOOM_FACTOR: f64 = 1.25;
const GRADIENT: &str = "gradient";
const BLUR: &str = "blur";
const MOSAIC: &str = "mosaic";
const BLACK: Rgb = Rgb::new(0, 0, 0);
const CHANNEL_MAX: f32 = 255.0;
const FULL_TURN: f64 = 360.0;
const FULL_PERCENT: f64 = 100.0;
const MANY_SCREENS: usize = 2;
const WHITE: Rgb = Rgb::new(255, 255, 255);
const SHARPNESS_TOLERANCE: f64 = 0.005;
const FULL_SCREEN_STYLE: &str = "wallpaper-full-screen";
const NOTICE_STYLE: &str = "wallpaper-notice";
const SIMULATION_ICON: &str = "view-fullscreen-symbolic";
const NOTICE_MARGIN: i32 = 24;
const NOTICE_DURATION: Duration = Duration::from_secs(4);

type SetCallback = Box<dyn Fn(WallpaperChoice)>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallpaperChoice {
    pub screen: Dimensions,
    pub framing: Framing,
    pub look: Look,
    pub adds_to_library: bool,
}

pub struct WallpaperSource {
    pub file: PathBuf,
    pub natural: (u32, u32),
    pub name: String,
    pub is_vector: bool,
    pub colors: Vec<(Rgb, String)>,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use pigoune_core::Rgb;

    use super::SetCallback;
    use crate::wallpaper_stage::PigouneWallpaperStage;
    use crate::wallpaper_swatches::ColorSwatches;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/wallpaper-dialog.ui")]
    pub struct PigouneWallpaperDialog {
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub set_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub stage: TemplateChild<PigouneWallpaperStage>,
        #[template_child]
        pub zoom_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub zoom_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub backdrop_toggles: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub swatches_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub swatches_hint: TemplateChild<gtk::Label>,
        pub swatches: OnceCell<Rc<ColorSwatches>>,
        pub color: Cell<Option<Rgb>>,
        pub gradient: Cell<Option<(Rgb, Rgb)>>,
        #[template_child]
        pub gradient_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub top_button: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub bottom_button: TemplateChild<gtk::ColorDialogButton>,
        pub showing_gradient: Cell<bool>,
        #[template_child]
        pub angle_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub angle_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub screen_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub sharpness_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub sharpness_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub actual_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub mirror_button: TemplateChild<gtk::ToggleButton>,
        pub screens: RefCell<Vec<crate::screen_size::ScreenChoice>>,
        pub chosen_screen: Cell<usize>,
        #[template_child]
        pub darkness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub darkness_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub thirds_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub snap_button: TemplateChild<gtk::ToggleButton>,
        pub is_vector: Cell<bool>,
        #[template_child]
        pub desktop_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub bar_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub hint_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub add_check: TemplateChild<gtk::CheckButton>,
        pub showing_zoom: Cell<bool>,
        pub on_set: RefCell<Option<SetCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWallpaperDialog {
        const NAME: &'static str = "PigouneWallpaperDialog";
        type Type = super::PigouneWallpaperDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            PigouneWallpaperStage::ensure_type();
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneWallpaperDialog {}
    impl WidgetImpl for PigouneWallpaperDialog {}
    impl AdwDialogImpl for PigouneWallpaperDialog {}
}

glib::wrapper! {
    pub struct PigouneWallpaperDialog(ObjectSubclass<imp::PigouneWallpaperDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneWallpaperDialog {
    pub fn new(
        source: WallpaperSource,
        (screens, current): (Vec<ScreenChoice>, usize),
        on_set: impl Fn(WallpaperChoice) + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.window_title.set_subtitle(&source.name);
        imp.hint_label.set_label(&gettext(
            "Drag the image to move it, use the mouse wheel to zoom and the arrow keys to adjust it",
        ));
        let shown = screens
            .get(current)
            .or_else(|| screens.first())
            .cloned()
            .expect("at least one screen is offered");
        dialog.offer_screens(&screens, current);
        imp.screens.replace(screens);
        imp.actual_button.set_label(&percent_text(1.0));
        imp.is_vector.set(source.is_vector);
        dialog.listen_to_full_screen_key();
        let names: Vec<&str> = Desktop::ALL.iter().map(|desktop| desktop.name()).collect();
        imp.desktop_row
            .set_model(Some(&gtk::StringList::new(&names)));
        let detected = Desktop::detected();
        let position = Desktop::ALL
            .iter()
            .position(|desktop| *desktop == detected)
            .unwrap_or(0);
        imp.desktop_row
            .set_selected(u32::try_from(position).unwrap_or(0));
        imp.stage.set_desktop(detected);
        dialog.offer_colors(&source.colors);
        dialog.on_darkness_changed();
        imp.on_set.replace(Some(Box::new(on_set)));
        imp.stage.connect_changed(glib::clone!(
            #[weak]
            dialog,
            move || dialog.show_zoom()
        ));
        glib::spawn_future_local(glib::clone!(
            #[weak]
            dialog,
            async move {
                dialog.load(&source, &shown).await;
            }
        ));
        dialog
    }

    async fn load(&self, source: &WallpaperSource, screen: &ScreenChoice) {
        let Some(image) = thumbnails::load_detailed(&source.file, SHOWN_VECTOR_PIXELS).await else {
            return;
        };
        let imp = self.imp();
        let natural = Size {
            width: f64::from(source.natural.0.max(1)),
            height: f64::from(source.natural.1.max(1)),
        };
        imp.stage
            .show(&image.texture, natural, size_of(screen.size), screen.scale);
        imp.set_button.set_sensitive(true);
        imp.stage.grab_focus();
    }

    fn show_zoom(&self) {
        let imp = self.imp();
        let scale = imp.stage.framing().scale;
        imp.zoom_label.set_label(&percent_text(scale));
        let enlarged = !imp.is_vector.get() && scale > ACTUAL_SCALE + SHARPNESS_TOLERANCE;
        if enlarged {
            imp.sharpness_label.set_label(
                &gettext("Image enlarged to {percent}: it will look blurry on this screen")
                    .replace("{percent}", &percent_text(scale)),
            );
        }
        imp.sharpness_revealer.set_reveal_child(enlarged);
        imp.showing_zoom.set(true);
        imp.zoom_scale.set_value(scale.ln());
        imp.showing_zoom.set(false);
    }

    #[template_callback]
    fn on_zoom_scale_changed(&self) {
        let imp = self.imp();
        if imp.showing_zoom.get() {
            return;
        }
        imp.stage.zoom_to(imp.zoom_scale.value().exp(), None);
    }

    #[template_callback]
    fn on_zoom_in_clicked(&self) {
        let stage = &self.imp().stage;
        stage.zoom_to(stage.framing().scale * BUTTON_ZOOM_FACTOR, None);
        self.imp().stage.grab_focus();
    }

    #[template_callback]
    fn on_zoom_out_clicked(&self) {
        let stage = &self.imp().stage;
        stage.zoom_to(stage.framing().scale / BUTTON_ZOOM_FACTOR, None);
        self.imp().stage.grab_focus();
    }

    #[template_callback]
    fn on_fill_clicked(&self) {
        self.imp().stage.fill_screen();
        self.imp().stage.grab_focus();
    }

    #[template_callback]
    fn on_whole_clicked(&self) {
        self.imp().stage.show_whole_image();
        self.imp().stage.grab_focus();
    }

    #[template_callback]
    fn on_actual_clicked(&self) {
        self.imp().stage.show_actual_size();
        self.imp().stage.grab_focus();
    }

    fn offer_colors(&self, image_colors: &[(Rgb, String)]) {
        let imp = self.imp();
        let mut colors: Vec<(Rgb, String)> = image_colors.to_vec();
        for (extra, family) in [(BLACK, AssetColor::Black), (WHITE, AssetColor::White)] {
            if !colors.iter().any(|(color, _)| *color == extra) {
                colors.push((extra, asset_colors::color_name(family)));
            }
        }
        let top = image_colors.first().map_or(BLACK, |(color, _)| *color);
        let bottom = image_colors.get(1).map_or(WHITE, |(color, _)| *color);
        imp.color.set(Some(BLACK));
        imp.gradient.set(Some((top, bottom)));
        let swatches = ColorSwatches::new(&colors);
        imp.swatches_box.append(swatches.widget());
        swatches.connect_picked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |color| dialog.pick_color(color)
        ));
        if imp.swatches.set(swatches).is_err() {
            unreachable!("colors are offered once");
        }
        self.on_backdrop_changed();
    }

    fn pick_color(&self, color: Rgb) {
        self.imp().color.set(Some(color));
        self.on_backdrop_changed();
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the angle is rounded and kept between 0 and 360 degrees"
    )]
    fn gradient_angle(&self) -> u16 {
        self.imp().angle_scale.value().round().clamp(0.0, FULL_TURN) as u16
    }

    #[template_callback]
    fn on_gradient_color_changed(&self) {
        let imp = self.imp();
        if imp.showing_gradient.get() {
            return;
        }
        imp.gradient.set(Some((
            rgb_of(&imp.top_button.rgba()),
            rgb_of(&imp.bottom_button.rgba()),
        )));
        self.on_backdrop_changed();
    }

    #[template_callback]
    fn on_swap_clicked(&self) {
        let imp = self.imp();
        let (top, bottom) = imp.gradient.get().unwrap_or((BLACK, WHITE));
        imp.gradient.set(Some((bottom, top)));
        self.on_backdrop_changed();
    }

    fn backdrop(&self) -> Backdrop {
        let imp = self.imp();
        let (top, bottom) = imp.gradient.get().unwrap_or((BLACK, WHITE));
        match imp.backdrop_toggles.active_name().as_deref() {
            Some(GRADIENT) => {
                Backdrop::Gradient(channels(top), channels(bottom), self.gradient_angle())
            }
            Some(BLUR) => Backdrop::Blur,
            Some(MOSAIC) => Backdrop::Mosaic,
            _ => Backdrop::Color(channels(imp.color.get().unwrap_or(BLACK))),
        }
    }

    #[template_callback]
    fn on_backdrop_changed(&self) {
        let imp = self.imp();
        let backdrop = self.backdrop();
        let color = imp.color.get().unwrap_or(BLACK);
        let (top, bottom) = imp.gradient.get().unwrap_or((BLACK, WHITE));
        imp.swatches_box
            .set_visible(matches!(backdrop, Backdrop::Color(_)));
        imp.gradient_box
            .set_visible(matches!(backdrop, Backdrop::Gradient(..)));
        imp.swatches_hint
            .set_label(&gettext("Colors of the image, or another color"));
        if let Some(swatches) = imp.swatches.get() {
            swatches.show_chosen(&[color]);
        }
        imp.angle_label
            .set_label(&gettext("{angle}°").replace("{angle}", &self.gradient_angle().to_string()));
        imp.showing_gradient.set(true);
        imp.top_button.set_rgba(&rgba_of(top));
        imp.bottom_button.set_rgba(&rgba_of(bottom));
        imp.showing_gradient.set(false);
        imp.stage.set_backdrop(backdrop);
    }

    #[template_callback]
    fn on_desktop_changed(&self) {
        let imp = self.imp();
        let selected = usize::try_from(imp.desktop_row.selected()).unwrap_or(0);
        if let Some(desktop) = Desktop::ALL.get(selected) {
            imp.stage.set_desktop(*desktop);
        }
    }

    #[template_callback]
    fn on_bar_changed(&self) {
        let imp = self.imp();
        imp.stage.set_shows_bar(imp.bar_row.is_active());
    }

    fn offer_screens(&self, screens: &[ScreenChoice], current: usize) {
        let imp = self.imp();
        imp.chosen_screen.set(current);
        if screens.len() < MANY_SCREENS {
            if let Some(screen) = screens.get(current) {
                imp.screen_group
                    .set_description(Some(&screen_text(screen.size)));
            }
            return;
        }
        imp.screen_group.set_description(Some(&gettext(
            "GNOME shows the same wallpaper on every screen: it is prepared for the one chosen here",
        )));
        let labels: Vec<String> = screens.iter().map(screen_size::screen_label).collect();
        let names: Vec<&str> = labels.iter().map(String::as_str).collect();
        let row = adw::ComboRow::builder()
            .title(gettext("Prepare For"))
            .model(&gtk::StringList::new(&names))
            .selected(u32::try_from(current).unwrap_or(0))
            .build();
        row.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |row| dialog.choose_screen(usize::try_from(row.selected()).unwrap_or(0))
        ));
        imp.screen_group.add(&row);
    }

    fn chosen_screen(&self) -> ScreenChoice {
        let imp = self.imp();
        let screens = imp.screens.borrow();
        screens
            .get(imp.chosen_screen.get())
            .or_else(|| screens.first())
            .cloned()
            .expect("at least one screen is offered")
    }

    fn choose_screen(&self, position: usize) {
        let imp = self.imp();
        imp.chosen_screen.set(position);
        let screen = self.chosen_screen();
        imp.stage.set_screen(size_of(screen.size), screen.scale);
    }

    #[template_callback]
    fn on_darkness_changed(&self) {
        let imp = self.imp();
        let share = imp.darkness_scale.value() / FULL_PERCENT;
        imp.darkness_label.set_label(&percent_text(share));
        imp.stage.set_darkness(share.clamp(0.0, LARGEST_DARKNESS));
    }

    #[template_callback]
    fn on_mirror_toggled(&self) {
        let imp = self.imp();
        imp.stage.set_mirrored(imp.mirror_button.is_active());
        imp.stage.grab_focus();
    }

    #[template_callback]
    fn on_thirds_toggled(&self) {
        let imp = self.imp();
        imp.stage.set_shows_thirds(imp.thirds_button.is_active());
        imp.stage.grab_focus();
    }

    #[template_callback]
    fn on_snap_toggled(&self) {
        let imp = self.imp();
        imp.stage.set_snaps(imp.snap_button.is_active());
        imp.stage.grab_focus();
    }

    #[template_callback]
    fn on_full_screen_clicked(&self) {
        self.show_full_screen();
    }

    fn listen_to_full_screen_key(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key == gdk::Key::F11 {
                    dialog.show_full_screen();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        ));
        self.add_controller(keys);
    }

    fn show_full_screen(&self) {
        let imp = self.imp();
        if !imp.set_button.is_sensitive() {
            return;
        }
        let stage = PigouneWallpaperStage::default();
        stage.copy_from(&imp.stage);
        stage.set_edge_to_edge();
        let notice = full_screen_notice();
        let overlay = gtk::Overlay::builder().child(&stage).build();
        overlay.add_overlay(&notice);
        let window = gtk::Window::builder()
            .child(&overlay)
            .modal(true)
            .css_classes([FULL_SCREEN_STYLE])
            .build();
        window.set_transient_for(self.root().and_downcast_ref::<gtk::Window>());
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            window,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if matches!(key, gdk::Key::Escape | gdk::Key::F11) {
                    window.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        ));
        window.add_controller(keys);
        follow_pointer(&window, &notice);
        window.connect_close_request(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            #[weak]
            stage,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_| {
                let shown = &dialog.imp().stage;
                shown.set_framing(stage.framing());
                shown.grab_focus();
                glib::Propagation::Proceed
            }
        ));
        window.fullscreen();
        window.present();
        stage.grab_focus();
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_set_clicked(&self) {
        let imp = self.imp();
        let choice = WallpaperChoice {
            screen: self.chosen_screen().size,
            framing: imp.stage.framing(),
            look: Look {
                backdrop: self.backdrop(),
                mirrored: imp.stage.mirrored(),
                darkness: imp.stage.darkness(),
            },
            adds_to_library: imp.add_check.is_active(),
        };
        self.close();
        if let Some(on_set) = self.imp().on_set.borrow().as_ref() {
            on_set(choice);
        }
    }
}

fn rgba_of(color: Rgb) -> gdk::RGBA {
    let [red, green, blue] = channels(color).map(|channel| f32::from(channel) / CHANNEL_MAX);
    gdk::RGBA::new(red, green, blue, 1.0)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is rounded and clamped to the range of a byte"
)]
fn rgb_of(rgba: &gdk::RGBA) -> Rgb {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * CHANNEL_MAX).round() as u8;
    Rgb::new(byte(rgba.red()), byte(rgba.green()), byte(rgba.blue()))
}

fn channels(color: Rgb) -> [u8; 3] {
    [color.red, color.green, color.blue]
}

fn full_screen_notice() -> gtk::Revealer {
    let label = gtk::Label::builder()
        .label(gettext(
            "Full screen simulation: the wallpaper is not set yet. Press Esc to come back.",
        ))
        .wrap(true)
        .build();
    let content = gtk::Box::builder()
        .spacing(8)
        .css_classes(["osd", NOTICE_STYLE])
        .build();
    content.append(&gtk::Image::from_icon_name(SIMULATION_ICON));
    content.append(&label);
    gtk::Revealer::builder()
        .child(&content)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .margin_bottom(NOTICE_MARGIN)
        .margin_start(NOTICE_MARGIN)
        .margin_end(NOTICE_MARGIN)
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .can_target(false)
        .reveal_child(true)
        .build()
}

fn follow_pointer(window: &gtk::Window, notice: &gtk::Revealer) {
    let hiding: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let show = glib::clone!(
        #[weak]
        notice,
        #[strong]
        hiding,
        move || {
            notice.set_reveal_child(true);
            if let Some(source) = hiding.take() {
                source.remove();
            }
            let later = glib::clone!(
                #[weak]
                notice,
                #[strong]
                hiding,
                move || {
                    hiding.take();
                    notice.set_reveal_child(false);
                }
            );
            hiding.replace(Some(glib::timeout_add_local_once(NOTICE_DURATION, later)));
        }
    );
    show();
    let motion = gtk::EventControllerMotion::new();
    let last: Rc<Cell<Option<(f64, f64)>>> = Rc::default();
    motion.connect_motion(move |_, x, y| {
        if last
            .replace(Some((x, y)))
            .is_some_and(|before| before != (x, y))
        {
            show();
        }
    });
    window.add_controller(motion);
}

fn size_of(screen: Dimensions) -> Size {
    Size {
        width: f64::from(screen.width()),
        height: f64::from(screen.height()),
    }
}

fn percent_text(scale: f64) -> String {
    gettext("{percent}%").replace("{percent}", &format!("{:.0}", scale * 100.0))
}

fn screen_text(screen: Dimensions) -> String {
    gettext("{width} × {height} pixels, the screen where Pigoune is shown")
        .replace("{width}", &screen.width().to_string())
        .replace("{height}", &screen.height().to_string())
}
