use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::Dimensions;

use crate::desktop_bars::Desktop;
use crate::thumbnails;
use crate::wallpaper_framing::{ACTUAL_SCALE, Backdrop, Framing};
use crate::wallpaper_stage::PigouneWallpaperStage;
use crate::zoom_math::Size;

const SHOWN_VECTOR_PIXELS: u32 = 2048;
const BUTTON_ZOOM_FACTOR: f64 = 1.25;
const BLUR: &str = "blur";
const SHARPNESS_TOLERANCE: f64 = 0.005;
const FULL_SCREEN_STYLE: &str = "wallpaper-full-screen";
const NOTICE_STYLE: &str = "wallpaper-notice";
const SIMULATION_ICON: &str = "view-fullscreen-symbolic";
const NOTICE_MARGIN: i32 = 24;
const NOTICE_DURATION: Duration = Duration::from_secs(4);
const COLOR_CHANNEL_MAX: f32 = 255.0;

type SetCallback = Box<dyn Fn(WallpaperChoice)>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallpaperChoice {
    pub framing: Framing,
    pub backdrop: Backdrop,
    pub adds_to_library: bool,
}

pub struct WallpaperSource {
    pub file: PathBuf,
    pub natural: (u32, u32),
    pub name: String,
    pub is_vector: bool,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use super::SetCallback;
    use crate::wallpaper_stage::PigouneWallpaperStage;

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
        pub color_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub screen_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub sharpness_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub sharpness_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub actual_button: TemplateChild<gtk::Button>,
        pub is_vector: Cell<bool>,
        #[template_child]
        pub color_button: TemplateChild<gtk::ColorDialogButton>,
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
        screen: Dimensions,
        monitor_scale: f64,
        on_set: impl Fn(WallpaperChoice) + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.window_title.set_subtitle(&source.name);
        imp.hint_label.set_label(&gettext(
            "Drag the image to move it, use the mouse wheel to zoom and the arrow keys to adjust it",
        ));
        imp.screen_group.set_description(Some(&screen_text(screen)));
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
        imp.color_button.set_rgba(&gtk::gdk::RGBA::BLACK);
        imp.on_set.replace(Some(Box::new(on_set)));
        imp.stage.connect_changed(glib::clone!(
            #[weak]
            dialog,
            move || dialog.show_zoom()
        ));
        let screen_size = Size {
            width: f64::from(screen.width()),
            height: f64::from(screen.height()),
        };
        glib::spawn_future_local(glib::clone!(
            #[weak]
            dialog,
            async move {
                dialog.load(&source, screen_size, monitor_scale).await;
            }
        ));
        dialog
    }

    async fn load(&self, source: &WallpaperSource, screen: Size, monitor_scale: f64) {
        let Some(image) = thumbnails::load_detailed(&source.file, SHOWN_VECTOR_PIXELS).await else {
            return;
        };
        let imp = self.imp();
        let natural = Size {
            width: f64::from(source.natural.0.max(1)),
            height: f64::from(source.natural.1.max(1)),
        };
        imp.stage
            .show(&image.texture, natural, screen, monitor_scale);
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

    fn backdrop(&self) -> Backdrop {
        let imp = self.imp();
        if imp.backdrop_toggles.active_name().as_deref() == Some(BLUR) {
            return Backdrop::Blur;
        }
        let color = imp.color_button.rgba();
        Backdrop::Color([color.red(), color.green(), color.blue()].map(channel_byte))
    }

    #[template_callback]
    fn on_backdrop_changed(&self) {
        let imp = self.imp();
        let backdrop = self.backdrop();
        imp.color_list
            .set_visible(matches!(backdrop, Backdrop::Color(_)));
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
        let choice = WallpaperChoice {
            framing: self.imp().stage.framing(),
            backdrop: self.backdrop(),
            adds_to_library: self.imp().add_check.is_active(),
        };
        self.close();
        if let Some(on_set) = self.imp().on_set.borrow().as_ref() {
            on_set(choice);
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is rounded and clamped to the range of a byte"
)]
fn channel_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * COLOR_CHANNEL_MAX).round() as u8
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

fn percent_text(scale: f64) -> String {
    gettext("{percent}%").replace("{percent}", &format!("{:.0}", scale * 100.0))
}

fn screen_text(screen: Dimensions) -> String {
    gettext("{width} × {height} pixels, the screen where Pigoune is shown")
        .replace("{width}", &screen.width().to_string())
        .replace("{height}", &screen.height().to_string())
}
