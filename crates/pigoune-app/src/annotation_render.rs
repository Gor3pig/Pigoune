use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};
use pigoune_core::annotations::{
    AnnotationDocument, Color, Point, Rect, Shape, Stroke, arrow_head, handles, shape_bounds,
};

const SELECTION_COLOR: (f32, f32, f32) = (0.21, 0.52, 0.89);
const SELECTION_PADDING: f32 = 6.0;
const SELECTION_DASH: [f32; 2] = [5.0, 4.0];
pub const HANDLE_SIZE: f32 = 10.0;

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

fn line_stroke(stroke: Stroke) -> gsk::Stroke {
    let line = gsk::Stroke::new(stroke.width.max(0.5));
    line.set_line_cap(gsk::LineCap::Round);
    line.set_line_join(gsk::LineJoin::Round);
    line
}

fn rectangle_path(bounds: Rect) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.add_rect(&graphene_rect(bounds));
    builder.to_path()
}

fn ellipse_path(bounds: Rect) -> gsk::Path {
    let half = graphene::Size::new(bounds.width / 2.0, bounds.height / 2.0);
    let rounded = gsk::RoundedRect::new(graphene_rect(bounds), half, half, half, half);
    let builder = gsk::PathBuilder::new();
    builder.add_rounded_rect(&rounded);
    builder.to_path()
}

fn segment_path(from: Point, to: Point) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(from.x, from.y);
    builder.line_to(to.x, to.y);
    builder.to_path()
}

fn polyline_path(points: &[Point]) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    if let Some((first, rest)) = points.split_first() {
        builder.move_to(first.x, first.y);
        if rest.is_empty() {
            builder.line_to(first.x, first.y);
        }
        for point in rest {
            builder.line_to(point.x, point.y);
        }
    }
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
    match shape {
        Shape::Rectangle {
            bounds,
            stroke,
            fill,
        } => append_closed(snapshot, &rectangle_path(*bounds), *stroke, *fill),
        Shape::Ellipse {
            bounds,
            stroke,
            fill,
        } => append_closed(snapshot, &ellipse_path(*bounds), *stroke, *fill),
        Shape::Line { from, to, stroke } => {
            snapshot.append_stroke(
                &segment_path(*from, *to),
                &line_stroke(*stroke),
                &rgba(stroke.color),
            );
        }
        Shape::Arrow { from, to, stroke } => append_arrow(snapshot, *from, *to, *stroke),
        Shape::Pen { points, stroke } | Shape::Highlighter { points, stroke } => {
            snapshot.append_stroke(
                &polyline_path(points),
                &line_stroke(*stroke),
                &rgba(stroke.color),
            );
        }
        Shape::Text { .. } | Shape::Step { .. } | Shape::Blur { .. } | Shape::Pixelate { .. } => {}
    }
}

fn append_closed(snapshot: &gtk::Snapshot, path: &gsk::Path, stroke: Stroke, fill: Option<Color>) {
    if let Some(fill) = fill {
        snapshot.append_fill(path, gsk::FillRule::Winding, &rgba(fill));
    }
    let outline = gsk::Stroke::new(stroke.width.max(0.5));
    outline.set_line_join(gsk::LineJoin::Round);
    snapshot.append_stroke(path, &outline, &rgba(stroke.color));
}

fn append_arrow(snapshot: &gtk::Snapshot, from: Point, to: Point, stroke: Stroke) {
    let head = arrow_head(from, to, stroke.width);
    let color = rgba(stroke.color);
    snapshot.append_stroke(&segment_path(from, head.base), &line_stroke(stroke), &color);
    let triangle = gsk::PathBuilder::new();
    triangle.move_to(head.tip.x, head.tip.y);
    triangle.line_to(head.left.x, head.left.y);
    triangle.line_to(head.right.x, head.right.y);
    triangle.close();
    snapshot.append_fill(&triangle.to_path(), gsk::FillRule::Winding, &color);
    snapshot.append_stroke(&triangle.to_path(), &line_stroke(stroke), &color);
}

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
