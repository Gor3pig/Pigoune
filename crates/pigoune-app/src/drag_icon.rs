use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};

const CARD: f32 = 112.0;
const MARGIN: f32 = 24.0;
const PADDING: f32 = 8.0;
const RADIUS: f32 = 12.0;
const BACK_ANGLES: [f32; 2] = [-10.0, 5.0];
const FRONT_ANGLE: f32 = -4.0;
const BADGE_HEIGHT: f32 = 24.0;
const BADGE_SIDE_PADDING: f32 = 8.0;
const RING: f32 = 2.0;
const SHADOW_BLUR: f32 = 10.0;
const SHADOW_DROP: f32 = 3.0;

const ICON_WIDTH: i32 = 360;
const STACK_OPACITY: f64 = 0.7;
const CAPTION_CLASS: &str = "drag-caption";

pub struct StackIcon {
    pub paintable: gdk::Paintable,
    pub hot_x: i32,
    pub hot_y: i32,
}

pub fn stack_icon(
    widget: &impl IsA<gtk::Widget>,
    layers: &[gdk::Paintable],
    count: usize,
) -> Option<StackIcon> {
    let side = CARD + 2.0 * MARGIN;
    let center = graphene::Point::new(side / 2.0, side / 2.0);
    let snapshot = gtk::Snapshot::new();
    let behind = layers.iter().skip(1).zip(BACK_ANGLES).rev();
    for (layer, angle) in behind {
        draw_card(&snapshot, layer, center, angle);
    }
    if let Some(front) = layers.first() {
        draw_card(&snapshot, front, center, FRONT_ANGLE);
    }
    if count > 1 {
        let corner = graphene::Point::new(MARGIN + CARD - 6.0, MARGIN + 6.0);
        draw_badge(&snapshot, widget, count, corner);
    }
    let paintable = snapshot.to_paintable(Some(&graphene::Size::new(side, side)))?;
    Some(StackIcon {
        paintable,
        hot_x: whole(center.x()),
        hot_y: whole(center.y()),
    })
}

pub fn caption_label() -> gtk::Label {
    let caption = gtk::Label::builder()
        .visible(false)
        .use_markup(true)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .max_width_chars(40)
        .halign(gtk::Align::Center)
        .build();
    caption.add_css_class(CAPTION_CLASS);
    caption
}

pub fn icon_widget(icon: &StackIcon, caption: &gtk::Label) -> gtk::Widget {
    let picture = gtk::Picture::for_paintable(&icon.paintable);
    picture.set_can_shrink(false);
    picture.set_halign(gtk::Align::Center);
    picture.set_opacity(STACK_OPACITY);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.set_width_request(ICON_WIDTH);
    layout.append(&picture);
    layout.append(caption);
    layout.upcast()
}

pub fn hotspot_x(icon: &StackIcon) -> i32 {
    let picture_width = icon.paintable.intrinsic_width();
    (ICON_WIDTH - picture_width) / 2 + icon.hot_x
}

fn draw_card(
    snapshot: &gtk::Snapshot,
    layer: &gdk::Paintable,
    center: graphene::Point,
    angle: f32,
) {
    snapshot.save();
    snapshot.translate(&center);
    snapshot.rotate(angle);
    snapshot.translate(&graphene::Point::new(-CARD / 2.0, -CARD / 2.0));
    let bounds = graphene::Rect::new(0.0, 0.0, CARD, CARD);
    let rounded = gsk::RoundedRect::from_rect(bounds, RADIUS);
    snapshot.append_outset_shadow(
        &rounded,
        &gdk::RGBA::new(0.0, 0.0, 0.0, 0.28),
        0.0,
        SHADOW_DROP,
        0.0,
        SHADOW_BLUR,
    );
    snapshot.push_rounded_clip(&rounded);
    snapshot.append_color(&card_color(), &bounds);
    let (width, height) = fitted(layer, CARD - 2.0 * PADDING);
    snapshot.save();
    snapshot.translate(&graphene::Point::new(
        (CARD - width) / 2.0,
        (CARD - height) / 2.0,
    ));
    layer.snapshot(snapshot, f64::from(width), f64::from(height));
    snapshot.restore();
    snapshot.pop();
    let edge = edge_color();
    snapshot.append_border(&rounded, &[1.0; 4], &[edge, edge, edge, edge]);
    snapshot.restore();
}

fn draw_badge(
    snapshot: &gtk::Snapshot,
    widget: &impl IsA<gtk::Widget>,
    count: usize,
    center: graphene::Point,
) {
    let layout = widget.create_pango_layout(Some(&count.to_string()));
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
    layout.set_attributes(Some(&attributes));
    let (text_width, text_height) = layout.pixel_size();
    let width = (pixels(text_width) + 2.0 * BADGE_SIDE_PADDING).max(BADGE_HEIGHT);
    let pill = graphene::Rect::new(
        center.x() - width / 2.0,
        center.y() - BADGE_HEIGHT / 2.0,
        width,
        BADGE_HEIGHT,
    );
    let ring = pill.inset_r(-RING, -RING);
    fill_rounded(snapshot, &ring, ring.height() / 2.0, &card_color());
    fill_rounded(
        snapshot,
        &pill,
        BADGE_HEIGHT / 2.0,
        &adw::StyleManager::default().accent_color_rgba(),
    );
    snapshot.save();
    snapshot.translate(&graphene::Point::new(
        pill.x() + (width - pixels(text_width)) / 2.0,
        pill.y() + (BADGE_HEIGHT - pixels(text_height)) / 2.0,
    ));
    snapshot.append_layout(&layout, &gdk::RGBA::WHITE);
    snapshot.restore();
}

fn fill_rounded(snapshot: &gtk::Snapshot, bounds: &graphene::Rect, radius: f32, color: &gdk::RGBA) {
    snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(*bounds, radius));
    snapshot.append_color(color, bounds);
    snapshot.pop();
}

fn card_color() -> gdk::RGBA {
    if adw::StyleManager::default().is_dark() {
        gdk::RGBA::new(0.24, 0.24, 0.26, 1.0)
    } else {
        gdk::RGBA::new(1.0, 1.0, 1.0, 1.0)
    }
}

fn edge_color() -> gdk::RGBA {
    if adw::StyleManager::default().is_dark() {
        gdk::RGBA::new(1.0, 1.0, 1.0, 0.10)
    } else {
        gdk::RGBA::new(0.0, 0.0, 0.0, 0.12)
    }
}

fn fitted(layer: &gdk::Paintable, side: f32) -> (f32, f32) {
    let ratio = layer.intrinsic_aspect_ratio();
    if ratio <= 0.0 {
        return (side, side);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an aspect ratio fits in f32"
    )]
    let ratio = ratio as f32;
    if ratio >= 1.0 {
        (side, side / ratio)
    } else {
        (side * ratio, side)
    }
}

fn pixels(value: i32) -> f32 {
    i16::try_from(value).map_or(0.0, f32::from)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the icon is a few dozen pixels wide"
)]
fn whole(value: f32) -> i32 {
    value.round() as i32
}
