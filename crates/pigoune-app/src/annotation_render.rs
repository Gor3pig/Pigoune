use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use pigoune_core::annotations::{
    AnnotationDocument, Color, CropEditor, Geometry, Point, Rect, Shape, arrow_head, dash_pattern,
    handles, head_size_factor, shape_bounds,
};

const SELECTION_COLOR: (f32, f32, f32) = (0.21, 0.52, 0.89);
const SELECTION_PADDING: f32 = 6.0;
const SELECTION_DASH: [f32; 2] = [5.0, 4.0];
pub const HANDLE_SIZE: f32 = 10.0;

pub struct Draw<'a> {
    pub pango: &'a pango::Context,
    pub ordinal: u32,
}

type Renderer = fn(&gtk::Snapshot, &Shape, &Draw);

const RENDERERS: [(&str, Renderer); 10] = [
    ("rectangle", draw_rectangle),
    ("ellipse", draw_ellipse),
    ("line", draw_line),
    ("arrow", draw_arrow),
    ("pen", draw_pen),
    ("highlighter", draw_highlighter),
    ("step", draw_step),
    ("text", draw_text),
    ("blur", draw_nothing_yet),
    ("pixelate", draw_nothing_yet),
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
    skipped: Option<u32>,
) {
    for object in &document.objects {
        if Some(object.id) == skipped {
            continue;
        }
        if let Some(shape) = object.shape() {
            let ordinal = document.ordinal(object.id).unwrap_or(1);
            append_shape(snapshot, shape, ordinal, context);
        }
    }
}

pub fn append_shape(
    snapshot: &gtk::Snapshot,
    shape: &Shape,
    ordinal: u32,
    context: &pango::Context,
) {
    if let Some((_, renderer)) = RENDERERS.iter().find(|(kind, _)| *kind == shape.kind) {
        renderer(
            snapshot,
            shape,
            &Draw {
                pango: context,
                ordinal,
            },
        );
    }
}

fn draw_closed(snapshot: &gtk::Snapshot, shape: &Shape, path: &gsk::Path) {
    if let Some(fill) = shape.fill_color() {
        snapshot.append_fill(path, gsk::FillRule::Winding, &rgba(fill));
    }
    snapshot.append_stroke(path, &stroke_of(shape, false), &rgba(shape.stroke_color()));
}

fn draw_rectangle(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some(bounds) = box_of(shape) {
        let path = rounded_rectangle_path(bounds, shape.number("corner_radius"));
        draw_closed(snapshot, shape, &path);
    }
}

fn draw_ellipse(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some(bounds) = box_of(shape) {
        draw_closed(snapshot, shape, &ellipse_path(bounds));
    }
}

fn draw_line(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    if let Some((from, to)) = ends_of(shape) {
        snapshot.append_stroke(
            &segment_path(from, to),
            &stroke_of(shape, true),
            &rgba(shape.stroke_color()),
        );
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

fn draw_arrow(snapshot: &gtk::Snapshot, shape: &Shape, _draw: &Draw) {
    let Some((from, to)) = ends_of(shape) else {
        return;
    };
    let width = shape.number("width");
    let factor = head_size_factor(shape.choice("head_size"));
    let color = rgba(shape.stroke_color());
    let head_style = shape.choice("head");
    let solid = gsk::Stroke::new(width.max(0.5));
    solid.set_line_join(gsk::LineJoin::Round);
    solid.set_line_cap(gsk::LineCap::Round);
    let end = arrow_head(from, to, width, factor);
    let start = arrow_head(to, from, width, factor);
    let double = head_style == "double";
    let shaft_from = if double { start.base } else { from };
    let shaft_to = if head_style == "none" { to } else { end.base };
    snapshot.append_stroke(
        &segment_path(shaft_from, shaft_to),
        &stroke_of(shape, true),
        &color,
    );
    let mut heads = Vec::new();
    match head_style {
        "none" => {}
        "double" => heads.extend([end, start]),
        _ => heads.push(end),
    }
    for head in heads {
        let path = head_path(head.tip, head.left, head.right, head_style != "open");
        if head_style == "open" {
            snapshot.append_stroke(&path, &solid, &color);
        } else {
            snapshot.append_fill(&path, gsk::FillRule::Winding, &color);
            snapshot.append_stroke(&path, &solid, &color);
        }
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
    let stroke = gsk::Stroke::new(shape.number("width").max(0.5));
    stroke.set_line_join(gsk::LineJoin::Round);
    stroke.set_line_cap(cap);
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
    let digits = draw.ordinal.to_string();
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
    let backdrop = match shape.choice("background") {
        "light" => Some(gdk::RGBA::new(1.0, 1.0, 1.0, 0.88)),
        "dark" => Some(gdk::RGBA::new(0.0, 0.0, 0.0, 0.72)),
        _ => None,
    };
    if let Some(backdrop) = backdrop {
        let padding = shape.number("font_size") * 0.25;
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
    snapshot.append_layout(&layout, &rgba(shape.stroke_color()));
    snapshot.restore();
}

pub fn text_view_css(shape: &Shape, zoom: f32) -> String {
    let color = shape.stroke_color();
    let background = match shape.choice("background") {
        "light" => "rgba(255, 255, 255, 0.88)",
        "dark" => "rgba(0, 0, 0, 0.72)",
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
    let Geometry::Anchor(at) = shape.geometry else {
        return None;
    };
    (shape.kind == "text").then(|| {
        let (width, height) = measure_text(context, shape);
        Rect {
            x: at.x,
            y: at.y,
            width,
            height,
        }
    })
}

fn draw_nothing_yet(_snapshot: &gtk::Snapshot, _shape: &Shape, _draw: &Draw) {}

pub fn append_selection(
    snapshot: &gtk::Snapshot,
    shape: &Shape,
    bounds: &graphene::Rect,
    zoom: f32,
    context: &pango::Context,
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
    for (_, point) in handles(shape) {
        let center = to_view(point);
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
