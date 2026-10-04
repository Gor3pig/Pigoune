use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::zoom_math::{self, Point, Size};

const WHEEL_ZOOM_FACTOR: f64 = 1.25;
const KEY_ZOOM_FACTOR: f64 = 1.25;
const MOVABLE_CURSOR: &str = "grab";
const MOVING_CURSOR: &str = "grabbing";

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
        pub pinch_start: Cell<f64>,
        pub on_zoom_changed: RefCell<Option<ZoomChangedCallback>>,
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
                pinch_start: Cell::new(1.0),
                on_zoom_changed: RefCell::default(),
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

    fn apply_zoom(&self, zoom: f64, center: Point) {
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

    #[expect(
        clippy::cast_possible_truncation,
        reason = "drawing coordinates are far below the limits of f32"
    )]
    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let Some(texture) = imp.texture.borrow().clone() else {
            return;
        };
        let zoom = imp.zoom.get();
        let image = imp.image.get();
        let center = imp.center.get();
        let view_center = self.view_center();
        let bounds = graphene::Rect::new(
            (view_center.x - center.x * zoom) as f32,
            (view_center.y - center.y * zoom) as f32,
            (image.width * zoom) as f32,
            (image.height * zoom) as f32,
        );
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
    }

    fn listen_to_drag(&self) {
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _, _| {
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
                view.apply_zoom(
                    zoom,
                    Point {
                        x: start.x - offset_x / zoom,
                        y: start.y - offset_y / zoom,
                    },
                );
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

impl Default for PigouneZoomView {
    fn default() -> Self {
        glib::Object::new()
    }
}
