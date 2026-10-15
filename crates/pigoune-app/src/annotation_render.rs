use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};
use pigoune_core::annotations::{
    AnnotationDocument, Color, Geometry, Point, Rect, Shape, arrow_head, dash_pattern, handles,
    head_size_factor, shape_bounds,
};

const SELECTION_COLOR: (f32, f32, f32) = (0.21, 0.52, 0.89);
const SELECTION_PADDING: f32 = 6.0;
const SELECTION_DASH: [f32; 2] = [5.0, 4.0];
pub const HANDLE_SIZE: f32 = 10.0;

type Renderer = fn(&gtk::Snapshot, &Shape);

const RENDERERS: [(&str, Renderer); 6] = [
    ("rectangle", draw_rectangle),
    ("ellipse", draw_ellipse),
    ("line", draw_line),
    ("arrow", draw_arrow),
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

pub fn append_document(snapshot: &gtk::Snapshot, document: &AnnotationDocument) {
    for object in &document.objects {
        if let Some(shape) = object.shape() {
            append_shape(snapshot, shape);
        }
    }
}

pub fn append_shape(snapshot: &gtk::Snapshot, shape: &Shape) {
    if let Some((_, renderer)) = RENDERERS.iter().find(|(kind, _)| *kind == shape.kind) {
        renderer(snapshot, shape);
    }
}

fn draw_closed(snapshot: &gtk::Snapshot, shape: &Shape, path: &gsk::Path) {
    if let Some(fill) = shape.fill_color() {
        snapshot.append_fill(path, gsk::FillRule::Winding, &rgba(fill));
    }
    snapshot.append_stroke(path, &stroke_of(shape, false), &rgba(shape.stroke_color()));
}

fn draw_rectangle(snapshot: &gtk::Snapshot, shape: &Shape) {
    if let Some(bounds) = box_of(shape) {
        let path = rounded_rectangle_path(bounds, shape.number("corner_radius"));
        draw_closed(snapshot, shape, &path);
    }
}

fn draw_ellipse(snapshot: &gtk::Snapshot, shape: &Shape) {
    if let Some(bounds) = box_of(shape) {
        draw_closed(snapshot, shape, &ellipse_path(bounds));
    }
}

fn draw_line(snapshot: &gtk::Snapshot, shape: &Shape) {
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

fn draw_arrow(snapshot: &gtk::Snapshot, shape: &Shape) {
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

fn draw_nothing_yet(_snapshot: &gtk::Snapshot, _shape: &Shape) {}

pub fn append_selection(
    snapshot: &gtk::Snapshot,
    shape: &Shape,
    bounds: &graphene::Rect,
    zoom: f32,
) {
    let (red, green, blue) = SELECTION_COLOR;
    let accent = gdk::RGBA::new(red, green, blue, 1.0);
    let to_view = |point: Point| Point {
        x: bounds.x() + point.x * zoom,
        y: bounds.y() + point.y * zoom,
    };
    if let Some(rect) = shape_bounds(shape) {
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
