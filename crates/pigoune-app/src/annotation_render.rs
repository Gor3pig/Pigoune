use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use pigoune_core::annotations::{
    AnnotationDocument, Color, CropEditor, Geometry, Guide, Handle, PathStep, Point, Rect, Shape,
    Stroke, TEXT_BACKDROP_SHARE, TEXT_OUTLINE_SHARE, arrow_curve_control, arrow_head, curve_point,
    curve_polyline, dash_pattern, guide_extent, handles_with_extent, head_size_factor,
    rounded_spot_radius, shape_bounds, sketch_curve, sketch_ellipse, sketch_line, sketch_outline,
    sketch_rectangle, sketched_curve, step_label, text_visual_bounds,
};

const SHADOW_OFFSET: (f32, f32) = (2.0, 3.0);
const SHADOW_BLUR: f32 = 6.0;
const SHADOW_OPACITY: f32 = 0.45;
const SHADOW_REFERENCE_WIDTH: f32 = 1200.0;
const SHADOW_LARGEST_SCALE: f32 = 4.0;
const SELECTION_COLOR: (f32, f32, f32) = (0.21, 0.52, 0.89);
const GUIDE_COLOR: (f32, f32, f32) = (0.93, 0.22, 0.55);
const BLUR_REACH_PER_RADIUS: f32 = 2.0;
const SELECTION_PADDING: f32 = 6.0;
const SELECTION_DASH: [f32; 2] = [5.0, 4.0];
pub const HANDLE_SIZE: f32 = 10.0;

pub type PixelCache = RefCell<HashMap<([u32; 5], u64), gdk::Texture>>;

const PIXEL_CACHE_LIMIT: usize = 24;

pub struct Backdrop<'a> {
    pub texture: &'a gdk::Texture,
    pub renderer: &'a gsk::Renderer,
    pub cache: &'a PixelCache,
}

pub struct Scene<'a> {
    pub backdrop: &'a Backdrop<'a>,
    pub document: &'a AnnotationDocument,
    pub upto: usize,
}

pub struct Draw<'a> {
    pub pango: &'a pango::Context,
    pub ordinal: u32,
    pub scene: Option<&'a Scene<'a>>,
}

type Renderer = fn(&gtk::Snapshot, &Shape, &Draw);

const RENDERERS: [(&str, Renderer); 11] = [
    ("rectangle", draw_rectangle),
    ("ellipse", draw_ellipse),
    ("line", draw_line),
    ("arrow", draw_arrow),
    ("pen", draw_pen),
    ("highlighter", draw_highlighter),
    ("step", draw_step),
    ("text", draw_text),
    ("blur", draw_blur),
    ("pixelate", draw_pixelate),
    ("spotlight", draw_spotlight),
];

#[cfg(test)]
fn drawn_kinds() -> impl Iterator<Item = &'static str> {
    RENDERERS.iter().map(|(kind, _)| *kind)
}

fn rgba(color: Color) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    )
}

fn graphene_rect(rect: Rect) -> graphene::Rect {
    graphene::Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn stroke_of(shape: &Shape, rounded: bool) -> gsk::Stroke {
    let width = shape.number("width").max(0.5);
    let stroke = gsk::Stroke::new(width);
    stroke.set_line_join(gsk::LineJoin::Round);
    let dash = shape.choice("dash");
    if let Some(pattern) = dash_pattern(dash, width) {
        stroke.set_dash(&pattern);
    }
    stroke.set_line_cap(if rounded || dash == "dotted" {
        gsk::LineCap::Round
    } else {
        gsk::LineCap::Butt
    });
    stroke
}

fn box_of(shape: &Shape) -> Option<Rect> {
    match shape.geometry {
        Geometry::Box(rect) => Some(rect),
        _ => None,
    }
}

fn ends_of(shape: &Shape) -> Option<(Point, Point)> {
    match shape.geometry {
        Geometry::TwoPoints { from, to } => Some((from, to)),
        _ => None,
    }
}

fn segment_path(from: Point, to: Point) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(from.x, from.y);
    builder.line_to(to.x, to.y);
    builder.to_path()
}

fn rounded_rectangle_path(bounds: Rect, radius: f32) -> gsk::Path {
    let limit = bounds.width.min(bounds.height) / 2.0;
    let corner = graphene::Size::new(radius.min(limit), radius.min(limit));
    let rounded = gsk::RoundedRect::new(graphene_rect(bounds), corner, corner, corner, corner);
    let builder = gsk::PathBuilder::new();
    builder.add_rounded_rect(&rounded);
    builder.to_path()
}

fn ellipse_path(bounds: Rect) -> gsk::Path {
    let half = graphene::Size::new(bounds.width / 2.0, bounds.height / 2.0);
    let rounded = gsk::RoundedRect::new(graphene_rect(bounds), half, half, half, half);
    let builder = gsk::PathBuilder::new();
    builder.add_rounded_rect(&rounded);
    builder.to_path()
}

pub fn append_document(
    snapshot: &gtk::Snapshot,
    document: &AnnotationDocument,
    context: &pango::Context,
    backdrop: Option<&Backdrop>,
    skipped: Option<u32>,
) {
    for (index, object) in document.objects.iter().enumerate() {
        if Some(object.id) == skipped || !object.is_rendered() {
            continue;
        }
        if let Some(shape) = object.shape() {
            let ordinal = document.ordinal(object.id).unwrap_or(1);
            let scene = backdrop.map(|backdrop| Scene {
                backdrop,
                document,
                upto: index,
            });
            append_shape(snapshot, shape, ordinal, context, scene.as_ref());
        }
    }
}

pub fn append_shape(
    snapshot: &gtk::Snapshot,
    shape: &Shape,
    ordinal: u32,
    context: &pango::Context,
    scene: Option<&Scene>,
) {
    if let Some((_, renderer)) = RENDERERS.iter().find(|(kind, _)| *kind == shape.kind) {
        let draw = Draw {
            pango: context,
            ordinal,
            scene,
        };
        if shape.toggle("shadow") {
            snapshot.push_shadow(&[drop_shadow(scene)]);
            renderer(snapshot, shape, &draw);
            snapshot.pop();
        } else {
            renderer(snapshot, shape, &draw);
        }
    }
}

fn drop_shadow(scene: Option<&Scene>) -> gsk::Shadow {
    let scale = scene.map_or(1.0, |scene| {
        (pixels(scene.backdrop.texture.width()) / SHADOW_REFERENCE_WIDTH)
            .clamp(1.0, SHADOW_LARGEST_SCALE)
    });
    gsk::Shadow::new(
        gdk::RGBA::new(0.0, 0.0, 0.0, SHADOW_OPACITY),
        SHADOW_OFFSET.0 * scale,
        SHADOW_OFFSET.1 * scale,
        SHADOW_BLUR * scale,
    )
}

fn sketched_path(strokes: &[Stroke]) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    for step in strokes.iter().flatten() {
        match *step {
            PathStep::MoveTo(to) => builder.move_to(to.x, to.y),
            PathStep::LineTo(to) => builder.line_to(to.x, to.y),
            PathStep::QuadTo(control, to) => builder.quad_to(control.x, control.y, to.x, to.y),
            PathStep::CubicTo(first, second, to) => {
                builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
            }
        }
    }
    builder.to_path()
}

fn draw_closed(snapshot: &gtk::Snapshot, shape: &Shape, path: &gsk::Path, outline: &gsk::Path) {
    if let Some(fill) = shape.fill_color() {
        snapshot.append_fill(path, gsk::FillRule::Winding, &rgba(fill));
    }
    snapshot.append_stroke(
        outline,
        &stroke_of(shape, false),
        &rgba(shape.stroke_color()),
    );
}

fn draw_rectangle(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some(bounds) = box_of(shape) {
        let path = rounded_rectangle_path(bounds, shape.number("corner_radius"));
        if shape.roughness().is_precise() {
            draw_closed(snapshot, shape, &path, &path);
        } else {
            let strokes = sketch_rectangle(bounds, shape.number("corner_radius"), shape.hand(), 0);
            draw_closed(snapshot, shape, &path, &sketched_path(&strokes));
        }
    }
}

fn draw_ellipse(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some(bounds) = box_of(shape) {
        let path = ellipse_path(bounds);
        if shape.roughness().is_precise() {
            draw_closed(snapshot, shape, &path, &path);
        } else {
            let strokes = sketch_ellipse(bounds, shape.hand(), 0);
            draw_closed(snapshot, shape, &path, &sketched_path(&strokes));
        }
    }
}

fn draw_line(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some((from, to)) = ends_of(shape) {
        let path = if shape.roughness().is_precise() {
            segment_path(from, to)
        } else {
            sketched_path(&sketch_line(from, to, shape.hand(), 0))
        };
        snapshot.append_stroke(&path, &stroke_of(shape, true), &rgba(shape.stroke_color()));
    }
}

fn head_path(tip: Point, left: Point, right: Point, closed: bool) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(left.x, left.y);
    builder.line_to(tip.x, tip.y);
    builder.line_to(right.x, right.y);
    if closed {
        builder.close();
    }
    builder.to_path()
}

const ARROW_SHAFT_SALT: u32 = 100;
const ARROW_HEAD_SALT: u32 = 200;
const TAPER_TAIL: f32 = 0.12;
const TAPER_HEAD_SHARE: f32 = 1.6;

fn shaft_path(from: Point, control: Option<Point>, to: Point) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(from.x, from.y);
    match control {
        Some(control) => builder.quad_to(control.x, control.y, to.x, to.y),
        None => builder.line_to(to.x, to.y),
    }
    builder.to_path()
}

fn tapered_shaft(from: Point, control: Option<Point>, to: Point, width: f32) -> gsk::Path {
    let middle = control.unwrap_or(Point {
        x: f32::midpoint(from.x, to.x),
        y: f32::midpoint(from.y, to.y),
    });
    let centers = curve_polyline(from, middle, to);
    let last = centers.len().saturating_sub(1);
    let mut left = Vec::with_capacity(centers.len());
    let mut right = Vec::with_capacity(centers.len());
    for (index, center) in centers.iter().enumerate() {
        let ahead = centers[(index + 1).min(last)];
        let behind = centers[index.saturating_sub(1)];
        let (dx, dy) = (ahead.x - behind.x, ahead.y - behind.y);
        let length = dx.hypot(dy).max(f32::EPSILON);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the curve has a couple of dozen points"
        )]
        let progress = index as f32 / last.max(1) as f32;
        let half = width * TAPER_HEAD_SHARE / 2.0 * (TAPER_TAIL + (1.0 - TAPER_TAIL) * progress);
        left.push(Point {
            x: center.x - dy / length * half,
            y: center.y + dx / length * half,
        });
        right.push(Point {
            x: center.x + dy / length * half,
            y: center.y - dx / length * half,
        });
    }
    let builder = gsk::PathBuilder::new();
    if let Some(first) = left.first() {
        builder.move_to(first.x, first.y);
    }
    for point in left.iter().skip(1).chain(right.iter().rev()) {
        builder.line_to(point.x, point.y);
    }
    builder.close();
    builder.to_path()
}

const CURVE_SEARCH_STEPS: u8 = 64;

fn closest_curve_parameter(
    from: Point,
    control: Point,
    to: Point,
    target: Point,
    low: f32,
    high: f32,
) -> f32 {
    let mut best = (f32::MAX, low);
    for step in 0..=CURVE_SEARCH_STEPS {
        let t = low + (high - low) * f32::from(step) / f32::from(CURVE_SEARCH_STEPS);
        let at = curve_point(from, control, to, t);
        let distance = (at.x - target.x).hypot(at.y - target.y);
        if distance < best.0 {
            best = (distance, t);
        }
    }
    best.1
}

fn trimmed_curve(
    from: Point,
    control: Point,
    to: Point,
    start: (bool, Point),
    end: (bool, Point),
) -> (Point, Option<Point>, Point) {
    let first = if start.0 {
        closest_curve_parameter(from, control, to, start.1, 0.0, 0.5)
    } else {
        0.0
    };
    let last = if end.0 {
        closest_curve_parameter(from, control, to, end.1, 0.5, 1.0)
    } else {
        1.0
    };
    let weight_from = (1.0 - first) * (1.0 - last);
    let weight_control = (1.0 - first) * last + first * (1.0 - last);
    let weight_to = first * last;
    let trimmed_control = Point {
        x: weight_from * from.x + weight_control * control.x + weight_to * to.x,
        y: weight_from * from.y + weight_control * control.y + weight_to * to.y,
    };
    (
        curve_point(from, control, to, first),
        Some(trimmed_control),
        curve_point(from, control, to, last),
    )
}

fn draw_arrow(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    let ink = shape.stroke_color();
    let translucent = ink.a < u8::MAX;
    if translucent {
        snapshot.push_opacity(f64::from(ink.a) / 255.0);
    }
    draw_arrow_parts(snapshot, shape, rgba(Color { a: u8::MAX, ..ink }));
    if translucent {
        snapshot.pop();
    }
}

fn draw_arrow_parts(snapshot: &gtk::Snapshot, shape: &Shape, color: gdk::RGBA) {
    let Some((from, to)) = ends_of(shape) else {
        return;
    };
    let width = shape.number("width");
    let factor = head_size_factor(shape.choice("head_size"));
    let head_style = shape.choice("head");
    let control = arrow_curve_control(shape);
    let solid = gsk::Stroke::new(width.max(0.5));
    solid.set_line_join(gsk::LineJoin::Round);
    solid.set_line_cap(gsk::LineCap::Round);
    let end = arrow_head(control.unwrap_or(from), to, width, factor);
    let start = arrow_head(control.unwrap_or(to), from, width, factor);
    let double = head_style == "double";
    let shaft_from = if double { start.base } else { from };
    let shaft_to = if head_style == "none" { to } else { end.base };
    let (shaft_from, control, shaft_to) = match control {
        Some(control) => trimmed_curve(
            from,
            control,
            to,
            (double, shaft_from),
            (head_style != "none", shaft_to),
        ),
        None => (shaft_from, None, shaft_to),
    };
    let level = shape.roughness();
    let hand = shape.hand();
    if shape.choice("shaft") == "tapered" && !double {
        let tapered = if level.is_precise() {
            tapered_shaft(shaft_from, control, shaft_to, width)
        } else {
            let middle = control.unwrap_or(Point {
                x: f32::midpoint(shaft_from.x, shaft_to.x),
                y: f32::midpoint(shaft_from.y, shaft_to.y),
            });
            let (start, bend, end) =
                sketched_curve(shaft_from, middle, shaft_to, hand, ARROW_SHAFT_SALT, 0);
            tapered_shaft(start, Some(bend), end, width)
        };
        snapshot.append_fill(&tapered, gsk::FillRule::Winding, &color);
    } else {
        let path = if level.is_precise() {
            shaft_path(shaft_from, control, shaft_to)
        } else {
            let strokes = match control {
                Some(bend) => sketch_curve(shaft_from, bend, shaft_to, hand, ARROW_SHAFT_SALT),
                None => sketch_line(shaft_from, shaft_to, hand, ARROW_SHAFT_SALT),
            };
            sketched_path(&strokes)
        };
        snapshot.append_stroke(&path, &stroke_of(shape, true), &color);
    }
    let mut heads = Vec::new();
    match head_style {
        "none" => {}
        "double" => heads.extend([end, start]),
        _ => heads.push(end),
    }
    for (index, head) in heads.into_iter().enumerate() {
        let open = head_style == "open";
        let path = head_path(head.tip, head.left, head.right, !open);
        let outline = if level.is_precise() {
            path.clone()
        } else {
            let salt = ARROW_HEAD_SALT + u32::try_from(index).unwrap_or(0) * 16;
            let strokes = if open {
                sketch_outline(&[head.left, head.tip, head.right], false, hand, salt)
            } else {
                sketch_outline(&[head.tip, head.left, head.right], true, hand, salt)
            };
            sketched_path(&strokes)
        };
        if !open {
            snapshot.append_fill(&path, gsk::FillRule::Winding, &color);
        }
        snapshot.append_stroke(&outline, &solid, &color);
    }
}

fn stroke_path(points: &[Point], smooth: bool) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    match points {
        [] => {}
        [only] => {
            builder.move_to(only.x, only.y);
            builder.line_to(only.x, only.y);
        }
        [first, rest @ ..] => {
            builder.move_to(first.x, first.y);
            if smooth && points.len() >= 3 {
                for pair in points[1..].windows(2) {
                    let middle = Point {
                        x: f32::midpoint(pair[0].x, pair[1].x),
                        y: f32::midpoint(pair[0].y, pair[1].y),
                    };
                    builder.quad_to(pair[0].x, pair[0].y, middle.x, middle.y);
                }
                if let Some(last) = rest.last() {
                    builder.line_to(last.x, last.y);
                }
            } else {
                for point in rest {
                    builder.line_to(point.x, point.y);
                }
            }
        }
    }
    builder.to_path()
}

fn draw_free_stroke(snapshot: &gtk::Snapshot, shape: &Shape, cap: gsk::LineCap) {
    let Geometry::Polyline(points) = &shape.geometry else {
        return;
    };
    let width = shape.number("width").max(0.5);
    let stroke = gsk::Stroke::new(width);
    stroke.set_line_join(gsk::LineJoin::Round);
    let dash = shape.choice("dash");
    if let Some(pattern) = dash_pattern(dash, width) {
        stroke.set_dash(&pattern);
    }
    stroke.set_line_cap(if dash == "dotted" {
        gsk::LineCap::Round
    } else {
        cap
    });
    snapshot.append_stroke(
        &stroke_path(points, shape.toggle("smooth")),
        &stroke,
        &rgba(shape.stroke_color()),
    );
}

fn draw_pen(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    draw_free_stroke(snapshot, shape, gsk::LineCap::Round);
}

fn draw_highlighter(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    draw_free_stroke(snapshot, shape, gsk::LineCap::Square);
}

fn text_color_over(color: Color) -> Color {
    let luminance =
        0.2126 * f32::from(color.r) + 0.7152 * f32::from(color.g) + 0.0722 * f32::from(color.b);
    if luminance > 153.0 {
        Color {
            r: 0x24,
            g: 0x1f,
            b: 0x31,
            a: 255,
        }
    } else {
        Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

fn draw_step(snapshot: &gtk::Snapshot, shape: &Shape, draw: &Draw) {
    let Geometry::Anchor(center) = shape.geometry else {
        return;
    };
    let radius = shape.number("size");
    let color = shape.stroke_color();
    let outlined = shape.choice("style") == "outlined";
    let disc = gsk::PathBuilder::new();
    disc.add_circle(&graphene::Point::new(center.x, center.y), radius);
    let path = disc.to_path();
    let text = if outlined {
        snapshot.append_fill(
            &path,
            gsk::FillRule::Winding,
            &gdk::RGBA::new(1.0, 1.0, 1.0, 0.92 * f32::from(color.a) / 255.0),
        );
        let ring = gsk::Stroke::new((radius * 0.16).max(1.5));
        snapshot.append_stroke(&path, &ring, &rgba(color));
        color
    } else {
        snapshot.append_fill(&path, gsk::FillRule::Winding, &rgba(color));
        text_color_over(color)
    };
    let digits = step_label(draw.ordinal, shape.choice("sequence"));
    let scale = match digits.len() {
        1 => 1.15,
        2 => 0.95,
        _ => 0.75,
    };
    let layout = pango::Layout::new(draw.pango);
    layout.set_text(&digits);
    let mut font = pango::FontDescription::from_string("Sans Bold");
    font.set_absolute_size(f64::from(radius * scale) * f64::from(pango::SCALE));
    layout.set_font_description(Some(&font));
    let (width, height) = layout.pixel_size();
    snapshot.save();
    snapshot.translate(&graphene::Point::new(
        center.x - f32::from(i16::try_from(width).unwrap_or(0)) / 2.0,
        center.y - f32::from(i16::try_from(height).unwrap_or(0)) / 2.0,
    ));
    snapshot.append_layout(
        &layout,
        &rgba(Color {
            a: if outlined { color.a } else { 255 },
            ..text
        }),
    );
    snapshot.restore();
}

fn text_family(shape: &Shape) -> &'static str {
    match shape.choice("family") {
        "serif" => "Serif",
        "mono" => "Monospace",
        _ => "Sans",
    }
}

fn text_layout(context: &pango::Context, shape: &Shape) -> pango::Layout {
    let layout = pango::Layout::new(context);
    layout.set_text(shape.text("content"));
    let mut font = pango::FontDescription::new();
    font.set_family(text_family(shape));
    font.set_weight(if shape.toggle("bold") {
        pango::Weight::Bold
    } else {
        pango::Weight::Normal
    });
    font.set_style(if shape.toggle("italic") {
        pango::Style::Italic
    } else {
        pango::Style::Normal
    });
    font.set_absolute_size(f64::from(shape.number("font_size")) * f64::from(pango::SCALE));
    layout.set_font_description(Some(&font));
    layout
}

fn pixels(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(i16::MAX))
}

pub fn measure_text(context: &pango::Context, shape: &Shape) -> (f32, f32) {
    let (width, height) = text_layout(context, shape).pixel_size();
    (pixels(width), pixels(height))
}

fn draw_text(snapshot: &gtk::Snapshot, shape: &Shape, draw: &Draw) {
    let Geometry::Anchor(at) = shape.geometry else {
        return;
    };
    if shape.text("content").is_empty() {
        return;
    }
    let layout = text_layout(draw.pango, shape);
    let (width, height) = layout.pixel_size();
    snapshot.save();
    snapshot.translate(&graphene::Point::new(at.x, at.y));
    let ink = shape.stroke_color();
    let bubble = shape.choice("background") == "bubble";
    let backdrop = match shape.choice("background") {
        "light" => Some(gdk::RGBA::new(1.0, 1.0, 1.0, 0.88)),
        "dark" => Some(gdk::RGBA::new(0.0, 0.0, 0.0, 0.72)),
        "bubble" => Some(rgba(ink)),
        _ => None,
    };
    if let Some(backdrop) = backdrop {
        let padding = shape.number("font_size") * TEXT_BACKDROP_SHARE;
        let path = rounded_rectangle_path(
            Rect {
                x: -padding,
                y: -padding,
                width: pixels(width) + 2.0 * padding,
                height: pixels(height) + 2.0 * padding,
            },
            padding,
        );
        snapshot.append_fill(&path, gsk::FillRule::Winding, &backdrop);
    }
    let letters = if bubble {
        Color {
            a: ink.a,
            ..text_color_over(ink)
        }
    } else {
        ink
    };
    if shape.toggle("outline") {
        draw_text_outline(snapshot, &layout, shape, text_color_over(letters));
    }
    snapshot.append_layout(&layout, &rgba(letters));
    snapshot.restore();
}

fn draw_text_outline(
    snapshot: &gtk::Snapshot,
    layout: &pango::Layout,
    shape: &Shape,
    color: Color,
) {
    let reach = (shape.number("font_size") * TEXT_OUTLINE_SHARE).max(1.0);
    let steps = [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ];
    for (dx, dy) in steps {
        snapshot.save();
        snapshot.translate(&graphene::Point::new(dx * reach, dy * reach));
        snapshot.append_layout(layout, &rgba(color));
        snapshot.restore();
    }
}

pub fn text_view_css(shape: &Shape, zoom: f32) -> String {
    let ink = shape.stroke_color();
    let bubble = shape.choice("background") == "bubble";
    let color = if bubble {
        Color {
            a: ink.a,
            ..text_color_over(ink)
        }
    } else {
        ink
    };
    let bubble_background = format!(
        "rgba({}, {}, {}, {})",
        ink.r,
        ink.g,
        ink.b,
        f32::from(ink.a) / 255.0
    );
    let background = match shape.choice("background") {
        "light" => "rgba(255, 255, 255, 0.88)",
        "dark" => "rgba(0, 0, 0, 0.72)",
        "bubble" => bubble_background.as_str(),
        _ => "transparent",
    };
    format!(
        ".annotation-editor, .annotation-editor text {{ font-family: {family}; font-size: {size}px; font-weight: {weight}; font-style: {style}; color: rgba({r}, {g}, {b}, {a}); }} textview.annotation-editor text {{ background: {background}; }}",
        family = text_family(shape),
        size = shape.number("font_size") * zoom,
        weight = if shape.toggle("bold") {
            "bold"
        } else {
            "normal"
        },
        style = if shape.toggle("italic") {
            "italic"
        } else {
            "normal"
        },
        r = color.r,
        g = color.g,
        b = color.b,
        a = f32::from(color.a) / 255.0,
    )
}

fn text_bounds(shape: &Shape, context: &pango::Context) -> Option<Rect> {
    if shape.kind != "text" {
        return None;
    }
    text_visual_bounds(shape, measure_text(context, shape))
}

fn whole_texture(texture: &gdk::Texture) -> graphene::Rect {
    graphene::Rect::new(0.0, 0.0, pixels(texture.width()), pixels(texture.height()))
}

fn draw_below(
    snapshot: &gtk::Snapshot,
    scene: &Scene,
    filter: gsk::ScalingFilter,
    context: &pango::Context,
) {
    let texture = scene.backdrop.texture;
    snapshot.append_scaled_texture(texture, filter, &whole_texture(texture));
    for (index, object) in scene.document.objects[..scene.upto].iter().enumerate() {
        if let Some(shape) = object.shape() {
            let ordinal = scene.document.ordinal(object.id).unwrap_or(1);
            let earlier = Scene {
                backdrop: scene.backdrop,
                document: scene.document,
                upto: index,
            };
            append_shape(snapshot, shape, ordinal, context, Some(&earlier));
        }
    }
}

fn draw_spotlight(snapshot: &gtk::Snapshot, shape: &Shape, draw: &Draw) {
    let (Some(scene), Some(region)) = (draw.scene, shape_bounds(shape)) else {
        return;
    };
    let veil = gsk::PathBuilder::new();
    veil.add_rect(&whole_texture(scene.backdrop.texture));
    match shape.choice("spot") {
        "ellipse" => veil.add_path(&ellipse_path(region)),
        "rounded" => veil.add_path(&rounded_rectangle_path(region, rounded_spot_radius(region))),
        _ => veil.add_rect(&graphene_rect(region)),
    }
    snapshot.append_fill(
        &veil.to_path(),
        gsk::FillRule::EvenOdd,
        &gdk::RGBA::new(0.0, 0.0, 0.0, shape.number("dimming")),
    );
}

fn edge_axis(index: i32, length: f32, reach: f32) -> (f32, f32, f32, f32) {
    match index {
        -1 => (-reach, reach * length, -reach, reach),
        1 => (
            length - (length - 1.0) * reach,
            reach * length,
            length,
            reach,
        ),
        _ => (0.0, length, 0.0, length),
    }
}

fn append_edge_extension(snapshot: &gtk::Snapshot, texture: &gdk::Texture, reach: f32) {
    let width = pixels(texture.width());
    let height = pixels(texture.height());
    for row in -1..=1 {
        for column in -1..=1 {
            if row == 0 && column == 0 {
                continue;
            }
            let (x, scaled_width, clip_x, clip_width) = edge_axis(column, width, reach);
            let (y, scaled_height, clip_y, clip_height) = edge_axis(row, height, reach);
            snapshot.push_clip(&graphene::Rect::new(
                clip_x,
                clip_y,
                clip_width,
                clip_height,
            ));
            snapshot.append_scaled_texture(
                texture,
                gsk::ScalingFilter::Nearest,
                &graphene::Rect::new(x, y, scaled_width, scaled_height),
            );
            snapshot.pop();
        }
    }
}

fn blur_reaches_outside(region: Rect, reach: f32, texture: &gdk::Texture) -> bool {
    region.x - reach < 0.0
        || region.y - reach < 0.0
        || region.x + region.width + reach > pixels(texture.width())
        || region.y + region.height + reach > pixels(texture.height())
}

fn draw_blur(snapshot: &gtk::Snapshot, shape: &Shape, draw: &Draw) {
    let (Some(scene), Some(region)) = (draw.scene, shape_bounds(shape)) else {
        return;
    };
    let radius = shape.number("radius");
    let reach = radius * BLUR_REACH_PER_RADIUS;
    snapshot.push_clip(&graphene_rect(region));
    snapshot.push_blur(f64::from(radius));
    if blur_reaches_outside(region, reach, scene.backdrop.texture) {
        append_edge_extension(snapshot, scene.backdrop.texture, reach.ceil());
    }
    draw_below(snapshot, scene, gsk::ScalingFilter::Linear, draw.pango);
    snapshot.pop();
    snapshot.pop();
}

fn pixel_tile(
    scene: &Scene,
    region: Rect,
    cell: f32,
    context: &pango::Context,
) -> Option<gdk::Texture> {
    let columns = (region.width / cell).ceil().max(1.0);
    let rows = (region.height / cell).ceil().max(1.0);
    let tile = gtk::Snapshot::new();
    tile.scale(1.0 / cell, 1.0 / cell);
    tile.translate(&graphene::Point::new(-region.x, -region.y));
    draw_below(&tile, scene, gsk::ScalingFilter::Trilinear, context);
    let node = tile.to_node()?;
    Some(
        scene
            .backdrop
            .renderer
            .render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, columns, rows))),
    )
}

fn scene_signature(scene: &Scene) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{:?}", &scene.document.objects[..scene.upto]).hash(&mut hasher);
    hasher.finish()
}

fn cached_pixel_tile(
    scene: &Scene,
    region: Rect,
    cell: f32,
    context: &pango::Context,
) -> Option<gdk::Texture> {
    let key = (
        [region.x, region.y, region.width, region.height, cell].map(f32::to_bits),
        scene_signature(scene),
    );
    let known = scene.backdrop.cache.borrow().get(&key).cloned();
    if known.is_some() {
        return known;
    }
    let tile = pixel_tile(scene, region, cell, context)?;
    let mut cache = scene.backdrop.cache.borrow_mut();
    if cache.len() >= PIXEL_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(key, tile.clone());
    Some(tile)
}

fn draw_pixelate(snapshot: &gtk::Snapshot, shape: &Shape, draw: &Draw) {
    let (Some(scene), Some(region)) = (draw.scene, shape_bounds(shape)) else {
        return;
    };
    let cell = shape.number("cell").max(1.0);
    let Some(tile) = cached_pixel_tile(scene, region, cell, draw.pango) else {
        return;
    };
    let covered = graphene::Rect::new(
        region.x,
        region.y,
        pixels(tile.width()) * cell,
        pixels(tile.height()) * cell,
    );
    snapshot.push_clip(&graphene_rect(region));
    snapshot.append_scaled_texture(&tile, gsk::ScalingFilter::Nearest, &covered);
    snapshot.pop();
}

pub fn append_guides(
    snapshot: &gtk::Snapshot,
    guides: &[Guide],
    whole: &graphene::Rect,
    zoom: f32,
    area: Rect,
) {
    if guides.is_empty() {
        return;
    }
    let (red, green, blue) = GUIDE_COLOR;
    let color = gdk::RGBA::new(red, green, blue, 0.9);
    let builder = gsk::PathBuilder::new();
    for guide in guides {
        let (from, to) = guide_extent(*guide, area);
        builder.move_to(whole.x() + from.x * zoom, whole.y() + from.y * zoom);
        builder.line_to(whole.x() + to.x * zoom, whole.y() + to.y * zoom);
    }
    snapshot.append_stroke(&builder.to_path(), &gsk::Stroke::new(1.0), &color);
}

pub fn append_selection(
    snapshot: &gtk::Snapshot,
    shape: &Shape,
    bounds: &graphene::Rect,
    zoom: f32,
    context: &pango::Context,
    with_handles: bool,
) {
    let (red, green, blue) = SELECTION_COLOR;
    let accent = gdk::RGBA::new(red, green, blue, 1.0);
    let to_view = |point: Point| Point {
        x: bounds.x() + point.x * zoom,
        y: bounds.y() + point.y * zoom,
    };
    if let Some(rect) = shape_bounds(shape).or_else(|| text_bounds(shape, context)) {
        let corner = to_view(Point {
            x: rect.x,
            y: rect.y,
        });
        let frame = graphene::Rect::new(
            corner.x - SELECTION_PADDING,
            corner.y - SELECTION_PADDING,
            rect.width * zoom + 2.0 * SELECTION_PADDING,
            rect.height * zoom + 2.0 * SELECTION_PADDING,
        );
        let builder = gsk::PathBuilder::new();
        builder.add_rect(&frame);
        let dashed = gsk::Stroke::new(1.0);
        dashed.set_dash(&SELECTION_DASH);
        snapshot.append_stroke(&builder.to_path(), &dashed, &accent);
    }
    if !with_handles {
        return;
    }
    let extent = (shape.kind == "text").then(|| measure_text(context, shape));
    for (handle, point) in handles_with_extent(shape, extent) {
        let center = to_view(point);
        if handle == Handle::Curve {
            let builder = gsk::PathBuilder::new();
            builder.add_circle(&graphene::Point::new(center.x, center.y), HANDLE_SIZE / 2.0);
            let path = builder.to_path();
            snapshot.append_fill(
                &path,
                gsk::FillRule::Winding,
                &gdk::RGBA::new(1.0, 1.0, 1.0, 1.0),
            );
            snapshot.append_stroke(&path, &gsk::Stroke::new(1.5), &accent);
            continue;
        }
        let square = graphene::Rect::new(
            center.x - HANDLE_SIZE / 2.0,
            center.y - HANDLE_SIZE / 2.0,
            HANDLE_SIZE,
            HANDLE_SIZE,
        );
        snapshot.append_color(&gdk::RGBA::new(1.0, 1.0, 1.0, 1.0), &square);
        let builder = gsk::PathBuilder::new();
        builder.add_rect(&square);
        snapshot.append_stroke(&builder.to_path(), &gsk::Stroke::new(1.5), &accent);
    }
}

pub fn append_crop(
    snapshot: &gtk::Snapshot,
    editor: &CropEditor,
    whole: &graphene::Rect,
    zoom: f32,
) {
    let frame = editor.frame();
    let left = whole.x() + frame.x * zoom;
    let top = whole.y() + frame.y * zoom;
    let width = frame.width * zoom;
    let height = frame.height * zoom;
    let shade = gdk::RGBA::new(0.0, 0.0, 0.0, 0.55);
    let (right, bottom) = (whole.x() + whole.width(), whole.y() + whole.height());
    for rect in [
        graphene::Rect::new(whole.x(), whole.y(), whole.width(), top - whole.y()),
        graphene::Rect::new(
            whole.x(),
            top + height,
            whole.width(),
            bottom - top - height,
        ),
        graphene::Rect::new(whole.x(), top, left - whole.x(), height),
        graphene::Rect::new(left + width, top, right - left - width, height),
    ] {
        if rect.width() > 0.0 && rect.height() > 0.0 {
            snapshot.append_color(&shade, &rect);
        }
    }
    let guide = gdk::RGBA::new(1.0, 1.0, 1.0, 0.45);
    let thirds = gsk::PathBuilder::new();
    for step in [1.0_f32, 2.0] {
        thirds.move_to(left + width * step / 3.0, top);
        thirds.line_to(left + width * step / 3.0, top + height);
        thirds.move_to(left, top + height * step / 3.0);
        thirds.line_to(left + width, top + height * step / 3.0);
    }
    snapshot.append_stroke(&thirds.to_path(), &gsk::Stroke::new(1.0), &guide);
    let outline = gsk::PathBuilder::new();
    outline.add_rect(&graphene::Rect::new(left, top, width, height));
    snapshot.append_stroke(
        &outline.to_path(),
        &gsk::Stroke::new(2.0),
        &gdk::RGBA::new(1.0, 1.0, 1.0, 1.0),
    );
    let (red, green, blue) = SELECTION_COLOR;
    let accent = gdk::RGBA::new(red, green, blue, 1.0);
    for (_, point) in editor.handle_points() {
        let square = graphene::Rect::new(
            whole.x() + point.x * zoom - HANDLE_SIZE / 2.0,
            whole.y() + point.y * zoom - HANDLE_SIZE / 2.0,
            HANDLE_SIZE,
            HANDLE_SIZE,
        );
        snapshot.append_color(&gdk::RGBA::new(1.0, 1.0, 1.0, 1.0), &square);
        let builder = gsk::PathBuilder::new();
        builder.add_rect(&square);
        snapshot.append_stroke(&builder.to_path(), &gsk::Stroke::new(1.5), &accent);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pigoune_core::annotations::definitions;

    use super::drawn_kinds;

    #[test]
    fn every_registered_annotation_tool_has_a_renderer() {
        let drawn: HashSet<&str> = drawn_kinds().collect();
        for definition in definitions() {
            assert!(
                drawn.contains(definition.kind),
                "{} has no renderer",
                definition.kind
            );
        }
        assert_eq!(drawn.len(), definitions().len());
    }
}
