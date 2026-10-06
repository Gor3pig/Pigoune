use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::desktop_bars::{Bars, Desktop};
use crate::desktop_frame::{self, Frame};
use crate::wallpaper_framing::{ACTUAL_SCALE, Framing};
use crate::zoom_math::{Point, Size};

const CUT_OPACITY: f64 = 0.3;
const WHEEL_ZOOM_FACTOR: f64 = 1.1;
const KEY_ZOOM_FACTOR: f64 = 1.25;
const SMALL_STEP: f64 = 10.0;
const LARGE_STEP: f64 = 100.0;
const MOVABLE_CURSOR: &str = "grab";
const MOVING_CURSOR: &str = "grabbing";

type ChangedCallback = Box<dyn Fn()>;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ChangedCallback;
    use crate::desktop_bars::Desktop;
    use crate::wallpaper_framing::Framing;
    use crate::zoom_math::{Point, Size};

    pub struct PigouneWallpaperStage {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub image: Cell<Size>,
        pub screen: Cell<Size>,
        pub monitor_scale: Cell<f64>,
        pub framing: Cell<Framing>,
        pub desktop: Cell<Desktop>,
        pub shows_bar: Cell<bool>,
        pub drag_start: Cell<Point>,
        pub pointer: Cell<Option<Point>>,
        pub on_changed: RefCell<Option<ChangedCallback>>,
    }

    impl Default for PigouneWallpaperStage {
        fn default() -> Self {
            let unit = Size {
                width: 1.0,
                height: 1.0,
            };
            Self {
                texture: RefCell::default(),
                image: Cell::new(unit),
                screen: Cell::new(unit),
                monitor_scale: Cell::new(1.0),
                framing: Cell::new(Framing {
                    scale: 1.0,
                    offset: Point { x: 0.0, y: 0.0 },
                }),
                desktop: Cell::new(Desktop::Gnome),
                shows_bar: Cell::new(true),
                drag_start: Cell::new(Point { x: 0.0, y: 0.0 }),
                pointer: Cell::default(),
                on_changed: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWallpaperStage {
        const NAME: &'static str = "PigouneWallpaperStage";
        type Type = super::PigouneWallpaperStage;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PigouneWallpaperStage {
        fn constructed(&self) {
            self.parent_constructed();
            let stage = self.obj();
            stage.set_overflow(gtk::Overflow::Hidden);
            stage.set_hexpand(true);
            stage.set_vexpand(true);
            stage.set_focusable(true);
            stage.set_cursor_from_name(Some(super::MOVABLE_CURSOR));
            stage.listen_to_drag();
            stage.listen_to_wheel();
            stage.listen_to_keys();
        }
    }

    impl WidgetImpl for PigouneWallpaperStage {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct PigouneWallpaperStage(ObjectSubclass<imp::PigouneWallpaperStage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneWallpaperStage {
    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.imp().on_changed.replace(Some(Box::new(callback)));
    }

    pub fn show(&self, texture: &gdk::Texture, image: Size, screen: Size, monitor_scale: f64) {
        let imp = self.imp();
        imp.texture.replace(Some(texture.clone()));
        imp.image.set(image);
        imp.screen.set(screen);
        imp.monitor_scale.set(monitor_scale);
        self.fill_screen();
    }

    pub fn framing(&self) -> Framing {
        self.imp().framing.get()
    }

    pub fn set_desktop(&self, desktop: Desktop) {
        self.imp().desktop.set(desktop);
        self.queue_draw();
    }

    pub fn set_shows_bar(&self, shows_bar: bool) {
        self.imp().shows_bar.set(shows_bar);
        self.queue_draw();
    }

    pub fn fill_screen(&self) {
        let imp = self.imp();
        self.change(Framing::filling(imp.image.get(), imp.screen.get()));
    }

    pub fn show_whole_image(&self) {
        let imp = self.imp();
        self.change(Framing::whole(imp.image.get(), imp.screen.get()));
    }

    pub fn show_actual_size(&self) {
        self.zoom_to(ACTUAL_SCALE, None);
    }

    pub fn zoom_to(&self, scale: f64, anchor: Option<Point>) {
        let screen = self.imp().screen.get();
        let anchor = anchor.unwrap_or(Point {
            x: screen.width / 2.0,
            y: screen.height / 2.0,
        });
        self.change(self.framing().zoomed_to(scale, anchor, screen));
    }

    fn change(&self, framing: Framing) {
        self.imp().framing.set(framing);
        self.queue_draw();
        if let Some(callback) = self.imp().on_changed.borrow().as_ref() {
            callback();
        }
    }

    fn screen_frame(&self) -> Frame {
        desktop_frame::screen_frame(
            Size {
                width: f64::from(self.width()),
                height: f64::from(self.height()),
            },
            self.imp().screen.get(),
        )
    }

    fn view_scale(&self) -> f64 {
        self.screen_frame().width / self.imp().screen.get().width.max(1.0)
    }

    fn on_screen(&self, point: Point) -> Point {
        let frame = self.screen_frame();
        let scale = self.view_scale();
        Point {
            x: (point.x - frame.x) / scale,
            y: (point.y - frame.y) / scale,
        }
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(texture) = imp.texture.borrow().clone() else {
            return;
        };
        let frame = self.screen_frame();
        let scale = self.view_scale();
        let image = self.framing().image_rect(imp.image.get(), imp.screen.get());
        let image_rect = rect_of(Frame {
            x: frame.x + image.x * scale,
            y: frame.y + image.y * scale,
            width: image.width * scale,
            height: image.height * scale,
        });
        let screen_rect = rect_of(frame);
        snapshot.push_opacity(CUT_OPACITY);
        snapshot.append_scaled_texture(&texture, gsk::ScalingFilter::Trilinear, &image_rect);
        snapshot.pop();
        snapshot.append_color(&gdk::RGBA::BLACK, &screen_rect);
        snapshot.push_clip(&screen_rect);
        snapshot.append_scaled_texture(&texture, gsk::ScalingFilter::Trilinear, &image_rect);
        if imp.shows_bar.get() {
            Bars {
                widget: self.upcast_ref(),
                screen: screen_rect,
                points: scale * imp.monitor_scale.get(),
            }
            .draw(snapshot, imp.desktop.get());
        }
        snapshot.pop();
    }

    fn listen_to_drag(&self) {
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_, _, _| {
                stage.imp().drag_start.set(stage.framing().offset);
                stage.set_cursor_from_name(Some(MOVING_CURSOR));
                stage.grab_focus();
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_, offset_x, offset_y| {
                let start = stage.imp().drag_start.get();
                let scale = stage.view_scale();
                stage.change(Framing {
                    offset: Point {
                        x: start.x + offset_x / scale,
                        y: start.y + offset_y / scale,
                    },
                    ..stage.framing()
                });
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_, _, _| stage.set_cursor_from_name(Some(MOVABLE_CURSOR))
        ));
        self.add_controller(drag);
    }

    fn listen_to_wheel(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_, x, y| stage.imp().pointer.set(Some(Point { x, y }))
        ));
        motion.connect_leave(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_| stage.imp().pointer.set(None)
        ));
        self.add_controller(motion);
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.connect_scroll(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, vertical| {
                let anchor = stage
                    .imp()
                    .pointer
                    .get()
                    .map(|pointer| stage.on_screen(pointer));
                let factor = WHEEL_ZOOM_FACTOR.powf(-vertical);
                stage.zoom_to(stage.framing().scale * factor, anchor);
                glib::Propagation::Stop
            }
        ));
        self.add_controller(wheel);
    }

    fn listen_to_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| stage.follow_key(key, modifiers)
        ));
        self.add_controller(keys);
    }

    fn follow_key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let step = if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
            LARGE_STEP
        } else {
            SMALL_STEP
        };
        let by = match key {
            gdk::Key::Left => Point { x: -step, y: 0.0 },
            gdk::Key::Right => Point { x: step, y: 0.0 },
            gdk::Key::Up => Point { x: 0.0, y: -step },
            gdk::Key::Down => Point { x: 0.0, y: step },
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => {
                self.zoom_to(self.framing().scale * KEY_ZOOM_FACTOR, None);
                return glib::Propagation::Stop;
            }
            gdk::Key::minus | gdk::Key::KP_Subtract => {
                self.zoom_to(self.framing().scale / KEY_ZOOM_FACTOR, None);
                return glib::Propagation::Stop;
            }
            _ => return glib::Propagation::Proceed,
        };
        self.change(self.framing().moved(by));
        glib::Propagation::Stop
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
fn rect_of(frame: Frame) -> graphene::Rect {
    graphene::Rect::new(
        frame.x as f32,
        frame.y as f32,
        frame.width as f32,
        frame.height as f32,
    )
}

impl Default for PigouneWallpaperStage {
    fn default() -> Self {
        glib::Object::new()
    }
}
