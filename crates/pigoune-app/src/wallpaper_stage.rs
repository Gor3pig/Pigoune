use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::desktop_bars::{Bars, Desktop};
use crate::desktop_frame::{self, Frame};
use crate::wallpaper_framing::{
    ACTUAL_SCALE, BLUR_BRIGHTNESS, BLUR_SHARE, Backdrop, Framing, Guides,
};
use crate::zoom_math::{Point, Size};

const CUT_OPACITY: f64 = 0.3;
const WHEEL_ZOOM_FACTOR: f64 = 1.1;
const KEY_ZOOM_FACTOR: f64 = 1.25;
const SMALL_STEP: f64 = 10.0;
const LARGE_STEP: f64 = 100.0;
const MOVABLE_CURSOR: &str = "grab";
const MOVING_CURSOR: &str = "grabbing";
const GSK_BLUR_PER_DEVIATION: f64 = 2.0;
const COLOR_CHANNEL_MAX: f32 = 255.0;
const SNAP_PIXELS: f64 = 12.0;
const THIRDS: [f32; 2] = [1.0 / 3.0, 2.0 / 3.0];
const GRID_LINE: f32 = 1.0;
const GRID_OPACITY: f32 = 0.7;
const GUIDE_LINE: f32 = 2.0;
const GUIDE_COLOR: (f32, f32, f32) = (0.208, 0.518, 0.894);

type ChangedCallback = Box<dyn Fn()>;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ChangedCallback;
    use crate::desktop_bars::Desktop;
    use crate::wallpaper_framing::{Backdrop, Framing, Guides};
    use crate::zoom_math::{Point, Size};

    pub struct PigouneWallpaperStage {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub image: Cell<Size>,
        pub screen: Cell<Size>,
        pub monitor_scale: Cell<f64>,
        pub framing: Cell<Framing>,
        pub desktop: Cell<Desktop>,
        pub backdrop: Cell<Backdrop>,
        pub shows_bar: Cell<bool>,
        pub margin: Cell<f64>,
        pub mirrored: Cell<bool>,
        pub darkness: Cell<f64>,
        pub shows_thirds: Cell<bool>,
        pub snaps: Cell<bool>,
        pub guides: Cell<Guides>,
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
                backdrop: Cell::new(Backdrop::Color([0, 0, 0])),
                shows_bar: Cell::new(true),
                margin: Cell::new(crate::desktop_frame::MARGIN),
                mirrored: Cell::new(false),
                darkness: Cell::new(0.0),
                shows_thirds: Cell::new(false),
                snaps: Cell::new(true),
                guides: Cell::new(Guides::default()),
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

    pub fn set_screen(&self, screen: Size, monitor_scale: f64) {
        let imp = self.imp();
        imp.screen.set(screen);
        imp.monitor_scale.set(monitor_scale);
        self.fill_screen();
    }

    pub fn framing(&self) -> Framing {
        self.imp().framing.get()
    }

    pub fn set_framing(&self, framing: Framing) {
        self.change(framing);
    }

    pub fn mirrored(&self) -> bool {
        self.imp().mirrored.get()
    }

    pub fn set_mirrored(&self, mirrored: bool) {
        self.imp().mirrored.set(mirrored);
        self.queue_draw();
    }

    pub fn darkness(&self) -> f64 {
        self.imp().darkness.get()
    }

    pub fn set_darkness(&self, darkness: f64) {
        self.imp().darkness.set(darkness);
        self.queue_draw();
    }

    pub fn set_shows_thirds(&self, shows_thirds: bool) {
        self.imp().shows_thirds.set(shows_thirds);
        self.queue_draw();
    }

    pub fn set_snaps(&self, snaps: bool) {
        self.imp().snaps.set(snaps);
    }

    pub fn set_edge_to_edge(&self) {
        self.imp().margin.set(0.0);
        self.queue_draw();
    }

    pub fn copy_from(&self, other: &Self) {
        let (imp, source) = (self.imp(), other.imp());
        imp.texture.replace(source.texture.borrow().clone());
        imp.image.set(source.image.get());
        imp.screen.set(source.screen.get());
        imp.monitor_scale.set(source.monitor_scale.get());
        imp.desktop.set(source.desktop.get());
        imp.backdrop.set(source.backdrop.get());
        imp.shows_bar.set(source.shows_bar.get());
        imp.mirrored.set(source.mirrored.get());
        imp.darkness.set(source.darkness.get());
        imp.shows_thirds.set(source.shows_thirds.get());
        imp.snaps.set(source.snaps.get());
        self.set_framing(other.framing());
    }

    pub fn set_desktop(&self, desktop: Desktop) {
        self.imp().desktop.set(desktop);
        self.queue_draw();
    }

    pub fn set_backdrop(&self, backdrop: Backdrop) {
        self.imp().backdrop.set(backdrop);
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
            self.imp().margin.get(),
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
        let mirrored = imp.mirrored.get();
        snapshot.push_opacity(CUT_OPACITY);
        append_image(
            snapshot,
            &texture,
            &image_rect,
            mirrored,
            gsk::ScalingFilter::Trilinear,
        );
        snapshot.pop();
        snapshot.push_clip(&screen_rect);
        self.draw_backdrop(snapshot, &texture, (&screen_rect, &image_rect), scale);
        append_image(
            snapshot,
            &texture,
            &image_rect,
            mirrored,
            gsk::ScalingFilter::Trilinear,
        );
        let darkness = imp.darkness.get();
        if darkness > 0.0 {
            snapshot.append_color(
                &gdk::RGBA::new(0.0, 0.0, 0.0, opacity_of(darkness)),
                &screen_rect,
            );
        }
        if imp.shows_thirds.get() {
            draw_thirds(snapshot, &screen_rect);
        }
        self.draw_guides(snapshot, &screen_rect, scale);
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

    fn draw_backdrop(
        &self,
        snapshot: &gtk::Snapshot,
        texture: &gdk::Texture,
        (screen_rect, image_rect): (&graphene::Rect, &graphene::Rect),
        scale: f64,
    ) {
        let imp = self.imp();
        match imp.backdrop.get() {
            Backdrop::Color(channels) => {
                snapshot.append_color(&rgba_of(channels), screen_rect);
            }
            Backdrop::Gradient(start, end, angle) => {
                let (from, to) = gradient_ends(screen_rect, f64::from(angle));
                snapshot.append_linear_gradient(
                    screen_rect,
                    &from,
                    &to,
                    &[
                        gsk::ColorStop::new(0.0, rgba_of(start)),
                        gsk::ColorStop::new(1.0, rgba_of(end)),
                    ],
                );
            }
            Backdrop::Mosaic => {
                snapshot.append_color(&gdk::RGBA::BLACK, screen_rect);
                snapshot.push_repeat(screen_rect, Some(image_rect));
                append_image(
                    snapshot,
                    texture,
                    image_rect,
                    imp.mirrored.get(),
                    gsk::ScalingFilter::Trilinear,
                );
                snapshot.pop();
            }
            Backdrop::Blur => {
                let screen = imp.screen.get();
                let cover =
                    Framing::filling(imp.image.get(), screen).image_rect(imp.image.get(), screen);
                let frame = self.screen_frame();
                let cover_rect = rect_of(Frame {
                    x: frame.x + cover.x * scale,
                    y: frame.y + cover.y * scale,
                    width: cover.width * scale,
                    height: cover.height * scale,
                });
                snapshot.append_color(&gdk::RGBA::BLACK, screen_rect);
                snapshot.push_opacity(BLUR_BRIGHTNESS);
                snapshot.push_blur(screen.width * BLUR_SHARE * scale * GSK_BLUR_PER_DEVIATION);
                append_image(
                    snapshot,
                    texture,
                    &cover_rect,
                    imp.mirrored.get(),
                    gsk::ScalingFilter::Linear,
                );
                snapshot.pop();
                snapshot.pop();
            }
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "drawing coordinates fit easily in f32"
    )]
    fn draw_guides(&self, snapshot: &gtk::Snapshot, screen: &graphene::Rect, scale: f64) {
        let guides = self.imp().guides.get();
        let (red, green, blue) = GUIDE_COLOR;
        let color = gdk::RGBA::new(red, green, blue, 1.0);
        if let Some(across) = guides.across {
            let x = screen.x() + (across * scale) as f32 - GUIDE_LINE / 2.0;
            snapshot.append_color(
                &color,
                &graphene::Rect::new(x, screen.y(), GUIDE_LINE, screen.height()),
            );
        }
        if let Some(down) = guides.down {
            let y = screen.y() + (down * scale) as f32 - GUIDE_LINE / 2.0;
            snapshot.append_color(
                &color,
                &graphene::Rect::new(screen.x(), y, screen.width(), GUIDE_LINE),
            );
        }
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
                let imp = stage.imp();
                let start = imp.drag_start.get();
                let scale = stage.view_scale();
                let pulled = Framing {
                    offset: Point {
                        x: start.x + offset_x / scale,
                        y: start.y + offset_y / scale,
                    },
                    ..stage.framing()
                };
                let (framing, guides) = if imp.snaps.get() {
                    pulled.snapped(imp.image.get(), imp.screen.get(), SNAP_PIXELS / scale)
                } else {
                    (pulled, Guides::default())
                };
                imp.guides.set(guides);
                stage.change(framing);
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = stage)]
            self,
            move |_, _, _| {
                stage.imp().guides.set(Guides::default());
                stage.set_cursor_from_name(Some(MOVABLE_CURSOR));
                stage.queue_draw();
            }
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
fn gradient_ends(area: &graphene::Rect, angle_degrees: f64) -> (graphene::Point, graphene::Point) {
    let angle = angle_degrees.to_radians();
    let (across, down) = (angle.sin(), -angle.cos());
    let (width, height) = (f64::from(area.width()), f64::from(area.height()));
    let half = f64::midpoint(width * across.abs(), height * down.abs());
    let center = area.center();
    let (center_x, center_y) = (f64::from(center.x()), f64::from(center.y()));
    (
        graphene::Point::new(
            (center_x - across * half) as f32,
            (center_y - down * half) as f32,
        ),
        graphene::Point::new(
            (center_x + across * half) as f32,
            (center_y + down * half) as f32,
        ),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "an opacity between 0 and 1 fits easily in f32"
)]
fn opacity_of(darkness: f64) -> f32 {
    darkness.clamp(0.0, 1.0) as f32
}

fn rgba_of(channels: [u8; 3]) -> gdk::RGBA {
    let [red, green, blue] = channels.map(|channel| f32::from(channel) / COLOR_CHANNEL_MAX);
    gdk::RGBA::new(red, green, blue, 1.0)
}

fn append_image(
    snapshot: &gtk::Snapshot,
    texture: &gdk::Texture,
    area: &graphene::Rect,
    mirrored: bool,
    filter: gsk::ScalingFilter,
) {
    if !mirrored {
        snapshot.append_scaled_texture(texture, filter, area);
        return;
    }
    let center = area.center();
    snapshot.save();
    snapshot.translate(&center);
    snapshot.scale(-1.0, 1.0);
    snapshot.translate(&graphene::Point::new(-center.x(), -center.y()));
    snapshot.append_scaled_texture(texture, filter, area);
    snapshot.restore();
}

fn draw_thirds(snapshot: &gtk::Snapshot, screen: &graphene::Rect) {
    let color = gdk::RGBA::WHITE.with_alpha(GRID_OPACITY);
    for share in THIRDS {
        let x = screen.x() + screen.width() * share - GRID_LINE / 2.0;
        let y = screen.y() + screen.height() * share - GRID_LINE / 2.0;
        snapshot.append_color(
            &color,
            &graphene::Rect::new(x, screen.y(), GRID_LINE, screen.height()),
        );
        snapshot.append_color(
            &color,
            &graphene::Rect::new(screen.x(), y, screen.width(), GRID_LINE),
        );
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
