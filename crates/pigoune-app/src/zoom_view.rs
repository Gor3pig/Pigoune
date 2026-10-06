use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

use crate::desktop_frame::{self, Frame};
use crate::zoom_math::{self, Point, Size};

const WHEEL_ZOOM_FACTOR: f64 = 1.25;
const KEY_ZOOM_FACTOR: f64 = 1.25;
const MOVABLE_CURSOR: &str = "grab";
const MOVING_CURSOR: &str = "grabbing";
const BOUNDS_WIDTH: f32 = 1.0;
const BOUNDS_DASH: [f32; 2] = [3.0, 3.0];
const BOUNDS_OPACITY: f32 = 0.35;
const PIXEL_GRID_WIDTH: f32 = 1.0;
const PIXEL_GRID_OPACITY: f32 = 0.15;
const SPRING_BACK_MILLISECONDS: u32 = 300;
const CUT_OPACITY: f64 = 0.28;
const TOP_BAR_SHARE: f64 = 32.0 / 1080.0;
const TOP_BAR_TEXT_SHARE: f64 = 0.42;
const TOP_BAR_PADDING_SHARE: f64 = 0.4;
const WORKSPACES_WIDTH_SHARE: f64 = 1.5;
const WORKSPACES_HEIGHT_SHARE: f64 = 0.5;
const WORKSPACES_OPACITY: f32 = 0.2;
const STATUS_DOT_SHARE: f64 = 0.24;
const STATUS_DOTS: u32 = 3;

type ZoomChangedCallback = Box<dyn Fn(f64)>;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ZoomChangedCallback;
    use crate::zoom_math::{Point, Size};

    pub struct PigouneZoomView {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub image: Cell<Size>,
        pub is_vector: Cell<bool>,
        pub fits: Cell<bool>,
        pub zoom: Cell<f64>,
        pub center: Cell<Point>,
        pub pointer: Cell<Option<Point>>,
        pub drag_start: Cell<Point>,
        pub dragging: Cell<bool>,
        pub shows_bounds: Cell<bool>,
        pub shows_pixel_grid: Cell<bool>,
        pub desktop: Cell<Option<Size>>,
        pub pinch_start: Cell<f64>,
        pub on_zoom_changed: RefCell<Option<ZoomChangedCallback>>,
        pub spring: RefCell<Option<adw::TimedAnimation>>,
    }

    impl Default for PigouneZoomView {
        fn default() -> Self {
            Self {
                texture: RefCell::default(),
                image: Cell::new(Size {
                    width: 1.0,
                    height: 1.0,
                }),
                is_vector: Cell::default(),
                fits: Cell::new(true),
                zoom: Cell::new(1.0),
                center: Cell::new(Point { x: 0.5, y: 0.5 }),
                pointer: Cell::default(),
                drag_start: Cell::new(Point { x: 0.0, y: 0.0 }),
                dragging: Cell::default(),
                shows_bounds: Cell::default(),
                shows_pixel_grid: Cell::default(),
                desktop: Cell::default(),
                pinch_start: Cell::new(1.0),
                on_zoom_changed: RefCell::default(),
                spring: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneZoomView {
        const NAME: &'static str = "PigouneZoomView";
        type Type = super::PigouneZoomView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PigouneZoomView {
        fn constructed(&self) {
            self.parent_constructed();
            let view = self.obj();
            view.set_overflow(gtk::Overflow::Hidden);
            view.set_hexpand(true);
            view.set_vexpand(true);
            view.listen_to_gestures();
        }
    }

    impl WidgetImpl for PigouneZoomView {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().settle_after_resize();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct PigouneZoomView(ObjectSubclass<imp::PigouneZoomView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneZoomView {
    pub fn connect_zoom_changed(&self, callback: impl Fn(f64) + 'static) {
        self.imp().on_zoom_changed.replace(Some(Box::new(callback)));
    }

    pub fn zoom(&self) -> f64 {
        self.imp().zoom.get()
    }

    pub fn show_image(
        &self,
        texture: Option<&gdk::Texture>,
        width: u32,
        height: u32,
        is_vector: bool,
    ) {
        let imp = self.imp();
        imp.texture.replace(texture.cloned());
        imp.image.set(Size {
            width: f64::from(width.max(1)),
            height: f64::from(height.max(1)),
        });
        imp.is_vector.set(is_vector);
        self.fit_to_view();
    }

    pub fn texture(&self) -> Option<gdk::Texture> {
        self.imp().texture.borrow().clone()
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "drawing coordinates fit easily in f32"
    )]
    pub fn image_bounds(&self) -> graphene::Rect {
        let imp = self.imp();
        if let Some(screen) = imp.desktop.get() {
            return rect_of(self.desktop_image_frame(screen));
        }
        let zoom = imp.zoom.get();
        let image = imp.image.get();
        let center = imp.center.get();
        let view_center = self.view_center();
        graphene::Rect::new(
            (view_center.x - center.x * zoom) as f32,
            (view_center.y - center.y * zoom) as f32,
            (image.width * zoom) as f32,
            (image.height * zoom) as f32,
        )
    }

    pub fn set_shows_bounds(&self, shows_bounds: bool) {
        self.imp().shows_bounds.set(shows_bounds);
        self.queue_draw();
    }

    pub fn set_desktop(&self, screen: Option<Size>) {
        self.imp().desktop.set(screen);
        self.queue_draw();
    }

    pub fn set_shows_pixel_grid(&self, shows_pixel_grid: bool) {
        self.imp().shows_pixel_grid.set(shows_pixel_grid);
        self.queue_draw();
    }

    pub fn replace_texture(&self, texture: &gdk::Texture) {
        self.imp().texture.replace(Some(texture.clone()));
        self.queue_draw();
    }

    pub fn fit_to_view(&self) {
        let imp = self.imp();
        imp.fits.set(true);
        self.apply_zoom(self.fit(), self.image_center());
    }

    pub fn show_actual_size(&self) {
        self.zoom_to(zoom_math::ACTUAL_SIZE, self.view_center());
    }

    pub fn zoom_in(&self) {
        self.zoom_to(self.zoom() * KEY_ZOOM_FACTOR, self.view_center());
    }

    pub fn zoom_out(&self) {
        self.zoom_to(self.zoom() / KEY_ZOOM_FACTOR, self.view_center());
    }

    fn zoom_to(&self, zoom: f64, anchor: Point) {
        let imp = self.imp();
        let old_zoom = self.zoom();
        let new_zoom = zoom_math::allowed_zoom(zoom, self.fit());
        imp.fits.set(false);
        let view_center = self.view_center();
        let anchor_offset = Point {
            x: anchor.x - view_center.x,
            y: anchor.y - view_center.y,
        };
        let center = zoom_math::zoom_around(imp.center.get(), anchor_offset, old_zoom, new_zoom);
        self.apply_zoom(new_zoom, center);
    }

    fn stop_spring(&self) {
        let spring = self.imp().spring.take();
        if let Some(spring) = spring {
            spring.pause();
        }
    }

    fn spring_back(&self) {
        let imp = self.imp();
        let from = imp.center.get();
        let to = zoom_math::clamp_center(from, self.view_size(), imp.image.get(), imp.zoom.get());
        if from == to {
            return;
        }
        let target = adw::CallbackAnimationTarget::new(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |progress| {
                view.imp().center.set(Point {
                    x: from.x + (to.x - from.x) * progress,
                    y: from.y + (to.y - from.y) * progress,
                });
                view.queue_draw();
            }
        ));
        let spring = adw::TimedAnimation::new(self, 0.0, 1.0, SPRING_BACK_MILLISECONDS, target);
        spring.set_easing(adw::Easing::EaseOutCubic);
        imp.spring.replace(Some(spring.clone()));
        spring.play();
    }

    fn apply_zoom(&self, zoom: f64, center: Point) {
        self.stop_spring();
        let imp = self.imp();
        let changed = (imp.zoom.get() - zoom).abs() > f64::EPSILON;
        imp.zoom.set(zoom);
        imp.center.set(zoom_math::clamp_center(
            center,
            self.view_size(),
            imp.image.get(),
            zoom,
        ));
        self.queue_draw();
        self.update_cursor();
        if changed && let Some(on_zoom_changed) = imp.on_zoom_changed.borrow().as_ref() {
            on_zoom_changed(zoom);
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "pixel counts and drawing coordinates fit easily"
    )]
    fn draw_pixel_grid(&self, snapshot: &gtk::Snapshot, bounds: &graphene::Rect, zoom: f64) {
        let image = self.imp().image.get();
        let view = self.view_size();
        let columns = image.width.round() as u32;
        let rows = image.height.round() as u32;
        let builder = gsk::PathBuilder::new();
        for column in
            zoom_math::visible_grid_lines(f64::from(bounds.x()), zoom, columns, view.width)
        {
            let x = bounds.x() + (f64::from(column) * zoom) as f32;
            builder.move_to(x, bounds.y().max(0.0));
            builder.line_to(x, (bounds.y() + bounds.height()).min(view.height as f32));
        }
        for row in zoom_math::visible_grid_lines(f64::from(bounds.y()), zoom, rows, view.height) {
            let y = bounds.y() + (f64::from(row) * zoom) as f32;
            builder.move_to(bounds.x().max(0.0), y);
            builder.line_to((bounds.x() + bounds.width()).min(view.width as f32), y);
        }
        snapshot.append_stroke(
            &builder.to_path(),
            &gsk::Stroke::new(PIXEL_GRID_WIDTH),
            &self.color().with_alpha(PIXEL_GRID_OPACITY),
        );
    }

    fn update_cursor(&self) {
        let imp = self.imp();
        let cursor = match (imp.texture.borrow().is_some(), imp.dragging.get()) {
            (false, _) => None,
            (true, false) => Some(MOVABLE_CURSOR),
            (true, true) => Some(MOVING_CURSOR),
        };
        self.set_cursor_from_name(cursor);
    }

    fn settle_after_resize(&self) {
        let imp = self.imp();
        if imp.fits.get() {
            self.apply_zoom(self.fit(), self.image_center());
        } else {
            self.apply_zoom(imp.zoom.get(), imp.center.get());
        }
    }

    fn fit(&self) -> f64 {
        let imp = self.imp();
        zoom_math::fit_zoom(self.view_size(), imp.image.get(), imp.is_vector.get())
    }

    fn view_size(&self) -> Size {
        Size {
            width: f64::from(self.width()),
            height: f64::from(self.height()),
        }
    }

    fn view_center(&self) -> Point {
        let size = self.view_size();
        Point {
            x: size.width / 2.0,
            y: size.height / 2.0,
        }
    }

    fn image_center(&self) -> Point {
        let image = self.imp().image.get();
        Point {
            x: image.width / 2.0,
            y: image.height / 2.0,
        }
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(texture) = imp.texture.borrow().clone() else {
            return;
        };
        if let Some(screen) = imp.desktop.get() {
            self.draw_desktop(snapshot, &texture, screen);
            return;
        }
        let zoom = imp.zoom.get();
        let image = imp.image.get();
        let bounds = self.image_bounds();
        let filter = if zoom_math::shows_sharp_pixels(
            zoom,
            image.width,
            f64::from(texture.width()),
            imp.is_vector.get(),
        ) {
            gsk::ScalingFilter::Nearest
        } else {
            gsk::ScalingFilter::Trilinear
        };
        snapshot.append_scaled_texture(&texture, filter, &bounds);
        if imp.shows_pixel_grid.get() && zoom_math::shows_pixel_grid(zoom, imp.is_vector.get()) {
            self.draw_pixel_grid(snapshot, &bounds, zoom);
        }
        if imp.shows_bounds.get() {
            draw_bounds(snapshot, &bounds, &self.color());
        }
    }

    fn desktop_screen_frame(&self, screen: Size) -> Frame {
        desktop_frame::screen_frame(self.view_size(), screen)
    }

    fn desktop_image_frame(&self, screen: Size) -> Frame {
        desktop_frame::covering_frame(self.desktop_screen_frame(screen), self.imp().image.get())
    }

    fn draw_desktop(&self, snapshot: &gtk::Snapshot, texture: &gdk::Texture, screen: Size) {
        let screen_rect = rect_of(self.desktop_screen_frame(screen));
        let image_rect = rect_of(self.desktop_image_frame(screen));
        snapshot.push_opacity(CUT_OPACITY);
        snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Trilinear, &image_rect);
        snapshot.pop();
        snapshot.append_color(&gdk::RGBA::BLACK, &screen_rect);
        snapshot.push_clip(&screen_rect);
        snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Trilinear, &image_rect);
        snapshot.pop();
        self.draw_top_bar(snapshot, &screen_rect);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "drawing coordinates fit easily in f32"
    )]
    fn draw_top_bar(&self, snapshot: &gtk::Snapshot, screen: &graphene::Rect) {
        let height = f64::from(screen.height()) * TOP_BAR_SHARE;
        let bar = graphene::Rect::new(screen.x(), screen.y(), screen.width(), height as f32);
        snapshot.append_color(&gdk::RGBA::BLACK, &bar);
        let padding = height * TOP_BAR_PADDING_SHARE;
        let middle = f64::from(bar.y()) + height / 2.0;
        let workspaces_height = height * WORKSPACES_HEIGHT_SHARE;
        let workspaces = graphene::Rect::new(
            (f64::from(bar.x()) + padding) as f32,
            (middle - workspaces_height / 2.0) as f32,
            (height * WORKSPACES_WIDTH_SHARE) as f32,
            workspaces_height as f32,
        );
        fill_rounded(
            snapshot,
            &workspaces,
            &gdk::RGBA::WHITE.with_alpha(WORKSPACES_OPACITY),
        );
        let dot = height * STATUS_DOT_SHARE;
        let right = f64::from(bar.x() + bar.width()) - padding;
        for index in 1..=STATUS_DOTS {
            let left = right - f64::from(index) * dot * 2.0 + dot;
            let disc = graphene::Rect::new(
                left as f32,
                (middle - dot / 2.0) as f32,
                dot as f32,
                dot as f32,
            );
            fill_rounded(snapshot, &disc, &gdk::RGBA::WHITE);
        }
        self.draw_clock(snapshot, &bar, height);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "font sizes and drawing coordinates fit easily in their types"
    )]
    fn draw_clock(&self, snapshot: &gtk::Snapshot, bar: &graphene::Rect, height: f64) {
        let layout = self.create_pango_layout(Some(&clock_text()));
        let mut font = pango::FontDescription::from_string("Sans Bold");
        font.set_absolute_size(height * TOP_BAR_TEXT_SHARE * f64::from(pango::SCALE));
        layout.set_font_description(Some(&font));
        let (width, text_height) = layout.pixel_size();
        snapshot.save();
        snapshot.translate(&graphene::Point::new(
            (f64::from(bar.x()) + (f64::from(bar.width()) - f64::from(width)) / 2.0) as f32,
            (f64::from(bar.y()) + (height - f64::from(text_height)) / 2.0) as f32,
        ));
        snapshot.append_layout(&layout, &gdk::RGBA::WHITE);
        snapshot.restore();
    }

    fn listen_to_drag(&self) {
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _, _| {
                view.stop_spring();
                let imp = view.imp();
                imp.drag_start.set(imp.center.get());
                imp.dragging.set(true);
                view.update_cursor();
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _, _| {
                view.imp().dragging.set(false);
                view.update_cursor();
                view.spring_back();
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, offset_x, offset_y| {
                let imp = view.imp();
                if offset_x != 0.0 || offset_y != 0.0 {
                    imp.fits.set(false);
                }
                let start = imp.drag_start.get();
                let zoom = imp.zoom.get();
                let pulled = Point {
                    x: start.x - offset_x / zoom,
                    y: start.y - offset_y / zoom,
                };
                imp.center.set(zoom_math::stretch_center(
                    pulled,
                    view.view_size(),
                    imp.image.get(),
                    zoom,
                ));
                view.queue_draw();
            }
        ));
        self.add_controller(drag);
    }

    fn listen_to_gestures(&self) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, x, y| view.imp().pointer.set(Some(Point { x, y }))
        ));
        motion.connect_leave(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| view.imp().pointer.set(None)
        ));
        self.add_controller(motion);

        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.connect_scroll(glib::clone!(
            #[weak(rename_to = view)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, vertical| {
                let anchor = view
                    .imp()
                    .pointer
                    .get()
                    .unwrap_or_else(|| view.view_center());
                view.zoom_to(view.zoom() * WHEEL_ZOOM_FACTOR.powf(-vertical), anchor);
                glib::Propagation::Stop
            }
        ));
        self.add_controller(wheel);

        self.listen_to_drag();

        let pinch = gtk::GestureZoom::new();
        pinch.connect_begin(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _| view.imp().pinch_start.set(view.zoom())
        ));
        pinch.connect_scale_changed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |gesture, scale| {
                let anchor = gesture
                    .bounding_box_center()
                    .map_or_else(|| view.view_center(), |(x, y)| Point { x, y });
                view.zoom_to(view.imp().pinch_start.get() * scale, anchor);
            }
        ));
        self.add_controller(pinch);

        let double_click = gtk::GestureClick::new();
        double_click.connect_pressed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, presses, x, y| {
                if presses != 2 {
                    return;
                }
                if view.imp().fits.get() {
                    view.zoom_to(zoom_math::ACTUAL_SIZE, Point { x, y });
                } else {
                    view.fit_to_view();
                }
            }
        ));
        self.add_controller(double_click);
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

fn fill_rounded(snapshot: &gtk::Snapshot, rect: &graphene::Rect, color: &gdk::RGBA) {
    let radius = rect.height() / 2.0;
    let corner = graphene::Size::new(radius, radius);
    snapshot.push_rounded_clip(&gsk::RoundedRect::new(
        *rect, corner, corner, corner, corner,
    ));
    snapshot.append_color(color, rect);
    snapshot.pop();
}

fn clock_text() -> String {
    let Ok(now) = glib::DateTime::now_local() else {
        return String::new();
    };
    let part = |format: &str| {
        now.format(format)
            .map(|text| text.to_string())
            .unwrap_or_default()
    };
    format!(
        "{} {} {}  {}",
        part("%a"),
        now.day_of_month(),
        part("%b"),
        part("%H:%M")
    )
}

fn draw_bounds(snapshot: &gtk::Snapshot, bounds: &graphene::Rect, color: &gdk::RGBA) {
    let half = BOUNDS_WIDTH / 2.0;
    let outline = graphene::Rect::new(
        bounds.x() - half,
        bounds.y() - half,
        bounds.width() + BOUNDS_WIDTH,
        bounds.height() + BOUNDS_WIDTH,
    );
    let builder = gsk::PathBuilder::new();
    builder.add_rect(&outline);
    let dashed = gsk::Stroke::new(BOUNDS_WIDTH);
    dashed.set_dash(&BOUNDS_DASH);
    snapshot.append_stroke(
        &builder.to_path(),
        &dashed,
        &color.with_alpha(BOUNDS_OPACITY),
    );
}

impl Default for PigouneZoomView {
    fn default() -> Self {
        glib::Object::new()
    }
}
