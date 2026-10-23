use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, ELLIPSE, EditSession, Geometry, Handle, Interaction,
    Options, Point, RECTANGLE, ROTATION, Rect, Shape, TEXT, Tool, definition, handles_with_extent,
    hit, hit_with_extent, number_to_value, resized_with_extent, rotate_point, rotated_bounds,
    rotation_handle, rotation_of, shape_pivot, translated, turned_points,
};
use serde_json::json;

const REACH: f32 = 6.0;
const EXTENT: (f32, f32) = (100.0, 40.0);

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn near(left: Point, right: Point) -> bool {
    same(left.x, right.x) && same(left.y, right.y)
}

fn rectangle(rotation: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 50.0,
        }),
        Options::defaults(RECTANGLE.options)
            .with("width", number_to_value(4.0))
            .with("rotation", number_to_value(rotation)),
    )
}

fn text(rotation: f32) -> Shape {
    Shape::new(
        "text",
        Geometry::Anchor(point(50.0, 60.0)),
        Options::defaults(TEXT.options)
            .with("content", json!("Hello"))
            .with("rotation", number_to_value(rotation)),
    )
}

fn handle_point(shape: &Shape, wanted: Handle) -> Point {
    handles_with_extent(shape, Some(EXTENT))
        .into_iter()
        .find(|(handle, _)| *handle == wanted)
        .expect("handle")
        .1
}

fn rect_of(shape: &Shape) -> Rect {
    match shape.geometry {
        Geometry::Box(rect) => rect,
        _ => panic!("a box"),
    }
}

#[test]
fn only_the_shapes_that_turn_have_a_rotation_between_minus_and_plus_one_hundred_and_eighty() {
    for kind in ["rectangle", "ellipse", "text"] {
        let sheet = definition(kind).expect("tool");
        assert!(sheet.spec("rotation").is_some(), "{kind} turns");
    }
    for kind in ["line", "arrow", "pen", "highlighter", "step", "spotlight"] {
        let sheet = definition(kind).expect("tool");
        assert!(sheet.spec("rotation").is_none(), "{kind} has no angle");
    }
    assert!(ROTATION.accepts(&json!(-180.0)) && ROTATION.accepts(&json!(180.0)));
    assert!(!ROTATION.accepts(&json!(181.0)));
    assert!(same(rotation_of(&rectangle(0.0)), 0.0));
    assert!(same(
        rotation_of(&Shape::new(
            "rectangle",
            Geometry::Box(Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0
            }),
            Options::new()
        )),
        0.0
    ));
    assert!(same(rotation_of(&rectangle(500.0)), 180.0));
    assert!(ELLIPSE.spec("rotation").is_some());
}

#[test]
fn a_positive_angle_turns_clockwise_around_the_pivot() {
    let pivot = point(150.0, 125.0);

    assert!(near(
        rotate_point(point(100.0, 100.0), pivot, 90.0),
        point(175.0, 75.0)
    ));
    assert!(near(
        rotate_point(point(100.0, 100.0), pivot, 180.0),
        point(200.0, 150.0)
    ));
    assert!(near(
        rotate_point(point(175.0, 75.0), pivot, -90.0),
        point(100.0, 100.0)
    ));
    assert!(near(
        rotate_point(point(100.0, 100.0), pivot, 0.0),
        point(100.0, 100.0)
    ));
}

#[test]
fn the_pivot_is_the_centre_of_the_box_and_of_the_measured_text() {
    assert!(near(
        shape_pivot(&rectangle(30.0), None).expect("pivot"),
        point(150.0, 125.0)
    ));
    assert!(near(
        shape_pivot(&text(30.0), Some(EXTENT)).expect("pivot"),
        point(100.0, 80.0)
    ));
    let line = Shape::new(
        "line",
        Geometry::TwoPoints {
            from: point(0.0, 0.0),
            to: point(10.0, 10.0),
        },
        Options::new(),
    );
    assert!(near(
        shape_pivot(&line, None).expect("pivot"),
        point(5.0, 5.0)
    ));
    let step = Shape::new("step", Geometry::Anchor(point(0.0, 0.0)), Options::new());
    assert!(shape_pivot(&step, None).is_none());
}

#[test]
fn the_handles_and_the_bounds_turn_with_the_shape() {
    let turned = rectangle(90.0);

    assert!(near(
        handle_point(&turned, Handle::TopLeft),
        point(175.0, 75.0)
    ));
    assert!(near(
        handle_point(&turned, Handle::Right),
        point(150.0, 175.0)
    ));
    assert!(near(
        handle_point(&turned, Handle::Left),
        point(150.0, 75.0)
    ));
    let bounds = rotated_bounds(&turned, None).expect("bounds");
    assert!(same(bounds.x, 125.0) && same(bounds.y, 75.0));
    assert!(same(bounds.width, 50.0) && same(bounds.height, 100.0));
    let flat = rotated_bounds(&rectangle(0.0), None).expect("bounds");
    assert!(same(flat.x, 100.0) && same(flat.width, 100.0));
}

#[test]
fn the_round_handle_sits_above_the_top_edge_and_turns_with_the_shape() {
    let above = rotation_handle(&rectangle(0.0), None, REACH).expect("handle");
    assert!(near(above, point(150.0, 100.0 - 3.0 * REACH)));
    let turned = rotation_handle(&rectangle(90.0), None, REACH).expect("handle");
    assert!(near(turned, point(150.0 + 25.0 + 3.0 * REACH, 125.0)));
    let above_text = rotation_handle(&text(0.0), Some(EXTENT), REACH).expect("handle");
    assert!(near(above_text, point(100.0, 60.0 - 3.0 * REACH)));
}

#[test]
fn a_turned_outline_is_hit_where_it_is_seen_and_not_where_it_was() {
    let turned = rectangle(90.0);

    assert!(hit(&turned, point(125.0, 125.0), 0.0));
    assert!(hit(&turned, point(150.0, 76.0), 0.0));
    assert!(!hit(&turned, point(105.0, 100.0), 0.0));
    assert!(!hit(&turned, point(195.0, 125.0), 0.0));
    assert!(hit(&rectangle(0.0), point(105.0, 100.0), 0.0));
}

#[test]
fn a_turned_text_is_hit_inside_its_turned_box() {
    let turned = text(90.0);

    assert!(hit_with_extent(
        &turned,
        point(100.0, 125.0),
        0.0,
        Some(EXTENT)
    ));
    assert!(!hit_with_extent(
        &turned,
        point(140.0, 80.0),
        0.0,
        Some(EXTENT)
    ));
    assert!(hit_with_extent(
        &text(0.0),
        point(140.0, 80.0),
        0.0,
        Some(EXTENT)
    ));
}

#[test]
fn resizing_a_turned_shape_follows_its_own_axes_and_keeps_the_opposite_side_in_place() {
    let turned = rectangle(90.0);
    let left_before = handle_point(&turned, Handle::Left);

    let longer = resized_with_extent(&turned, Handle::Right, point(150.0, 215.0), EXTENT);

    let rect = rect_of(&longer);
    assert!(same(rect.width, 140.0) && same(rect.height, 50.0));
    assert!(same(rotation_of(&longer), 90.0));
    assert!(near(handle_point(&longer, Handle::Left), left_before));
    assert!(near(
        handle_point(&longer, Handle::Right),
        point(150.0, 215.0)
    ));
}

#[test]
fn resizing_an_unturned_shape_is_what_it_always_was() {
    let flat = rectangle(0.0);

    let wider = resized_with_extent(&flat, Handle::Right, point(260.0, 999.0), EXTENT);

    let rect = rect_of(&wider);
    assert!(same(rect.x, 100.0) && same(rect.width, 160.0) && same(rect.height, 50.0));
}

#[test]
fn a_turned_text_scales_around_the_opposite_corner_of_its_turned_box() {
    let turned = text(90.0);
    let corner_before = handle_point(&turned, Handle::BottomLeft);
    let pivot = point(100.0, 80.0);

    let bigger = resized_with_extent(
        &turned,
        Handle::TopRight,
        rotate_point(point(250.0, 20.0), pivot, 90.0),
        EXTENT,
    );

    assert!(same(bigger.number("font_size"), 64.0));
    assert!(same(rotation_of(&bigger), 90.0));
    let corner_after = handles_with_extent(&bigger, Some((200.0, 80.0)))
        .into_iter()
        .find(|(handle, _)| *handle == Handle::BottomLeft)
        .expect("handle")
        .1;
    assert!(near(corner_after, corner_before));
}

#[test]
fn moving_a_turned_shape_keeps_its_angle() {
    let moved = translated(&rectangle(35.0), 10.0, 20.0);

    assert!(same(rotation_of(&moved), 35.0));
    assert!(near(
        shape_pivot(&moved, None).expect("pivot"),
        point(160.0, 145.0)
    ));
}

fn selected_rectangle(rotation: f32) -> (Interaction, EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.set_text_measurer(|_| EXTENT);
    let id = session.add(rectangle(rotation));
    session.select(Some(id));
    (Interaction::new(), session, id)
}

fn turn(interaction: &mut Interaction, session: &mut EditSession, from: Point, to: Point) {
    let options = Options::defaults(RECTANGLE.options);
    interaction.press(session, Tool::Select, &options, from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

#[test]
fn dragging_the_round_handle_turns_the_shape_in_one_undoable_step() {
    let (mut interaction, mut session, id) = selected_rectangle(0.0);
    let handle = rotation_handle(&rectangle(0.0), None, REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(232.0, 125.0));

    assert!(same(
        rotation_of(session.shape_of(id).expect("shape")),
        90.0
    ));
    assert!(session.undo());
    assert!(same(rotation_of(session.shape_of(id).expect("shape")), 0.0));
    assert!(session.redo());
    assert!(same(
        rotation_of(session.shape_of(id).expect("shape")),
        90.0
    ));
}

#[test]
fn turning_again_continues_from_the_current_angle_without_a_jump() {
    let (mut interaction, mut session, id) = selected_rectangle(90.0);
    let handle = rotation_handle(&rectangle(90.0), None, REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(150.0, 207.0));

    assert!(same(
        rotation_of(session.shape_of(id).expect("shape")),
        180.0
    ));
}

#[test]
fn the_angle_goes_the_short_way_round_and_stays_within_a_half_turn() {
    let (mut interaction, mut session, id) = selected_rectangle(170.0);
    let handle = rotation_handle(&rectangle(170.0), None, REACH).expect("handle");
    let pivot = point(150.0, 125.0);
    let target = rotate_point(handle, pivot, 30.0);

    turn(&mut interaction, &mut session, handle, target);

    let angle = rotation_of(session.shape_of(id).expect("shape"));
    assert!(same(angle, -160.0), "{angle}");
}

#[test]
fn holding_shift_turns_by_steps_of_fifteen_degrees() {
    let (mut interaction, mut session, id) = selected_rectangle(0.0);
    let handle = rotation_handle(&rectangle(0.0), None, REACH).expect("handle");
    let pivot = point(150.0, 125.0);
    interaction.set_constrained(true);

    turn(
        &mut interaction,
        &mut session,
        handle,
        rotate_point(handle, pivot, 37.0),
    );

    assert!(same(
        rotation_of(session.shape_of(id).expect("shape")),
        30.0
    ));
}

#[test]
fn a_locked_shape_cannot_be_turned() {
    let (mut interaction, mut session, id) = selected_rectangle(0.0);
    assert!(session.set_locked(id, true));
    let handle = rotation_handle(&rectangle(0.0), None, REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(232.0, 125.0));

    assert!(same(rotation_of(session.shape_of(id).expect("shape")), 0.0));
}

#[test]
fn a_selected_text_turns_around_the_centre_of_its_measured_box() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.set_text_measurer(|_| EXTENT);
    let id = session.add(text(0.0));
    session.select(Some(id));
    let mut interaction = Interaction::new();
    let handle = rotation_handle(&text(0.0), Some(EXTENT), REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(160.0, 80.0));

    assert!(same(rotation_of(session.shape_of(id).expect("text")), 90.0));
}

#[test]
fn pressing_the_round_handle_of_an_unselected_shape_does_not_turn_it() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    session.select(None);
    let mut interaction = Interaction::new();
    let handle = rotation_handle(&rectangle(0.0), None, REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(232.0, 125.0));

    assert!(same(rotation_of(session.shape_of(id).expect("shape")), 0.0));
}

#[test]
fn the_angle_survives_a_trip_through_the_document_and_an_old_document_reads_as_flat() {
    let document = AnnotationDocument::new(
        None,
        vec![
            AnnotationObject::new(1, rectangle(-47.0)),
            AnnotationObject::new(2, text(120.0)),
        ],
    );

    let back = AnnotationDocument::from_json(&document.to_json().expect("json")).expect("read");

    let angles: Vec<f32> = back
        .objects
        .iter()
        .map(|object| rotation_of(object.shape().expect("shape")))
        .collect();
    assert!(same(angles[0], -47.0) && same(angles[1], 120.0));
    let old = r#"{"version":1,"objects":[{"id":1,"kind":"rectangle","bounds":{"x":1,"y":2,"width":3,"height":4},"options":{}}]}"#;
    let read = AnnotationDocument::from_json(old).expect("old document");
    assert!(same(
        rotation_of(read.objects[0].shape().expect("shape")),
        0.0
    ));
}

#[test]
fn a_blur_and_a_pixelation_turn_and_are_hit_where_they_are_seen() {
    for kind in ["blur", "pixelate"] {
        let sheet = definition(kind).expect("tool");
        assert!(sheet.spec("rotation").is_some(), "{kind} turns");
        let shape = Shape::new(
            kind,
            Geometry::Box(Rect {
                x: 70.0,
                y: 40.0,
                width: 60.0,
                height: 20.0,
            }),
            Options::defaults(sheet.options).with("rotation", number_to_value(90.0)),
        );

        assert!(
            hit(&shape, point(100.0, 25.0), 0.0),
            "{kind} is hit in its turned area"
        );
        assert!(
            !hit(&shape, point(72.0, 50.0), 0.0),
            "{kind} is not hit where it was"
        );
        let bounds = rotated_bounds(&shape, None).expect("bounds");
        assert!(same(bounds.x, 90.0) && same(bounds.y, 20.0));
        assert!(same(bounds.width, 20.0) && same(bounds.height, 60.0));
        assert!(rotation_handle(&shape, None, REACH).is_some());
    }
}

fn line() -> Shape {
    Shape::new(
        "line",
        Geometry::TwoPoints {
            from: point(100.0, 100.0),
            to: point(200.0, 100.0),
        },
        Options::defaults(definition("line").expect("tool").options),
    )
}

fn arrow(curve: f32) -> Shape {
    Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: point(100.0, 100.0),
            to: point(200.0, 100.0),
        },
        Options::defaults(definition("arrow").expect("tool").options)
            .with("curve", number_to_value(curve)),
    )
}

fn stroke(kind: &str) -> Shape {
    Shape::new(
        kind,
        Geometry::Polyline(vec![point(0.0, 0.0), point(100.0, 0.0), point(100.0, 50.0)]),
        Options::defaults(definition(kind).expect("tool").options),
    )
}

fn ends_of(shape: &Shape) -> (Point, Point) {
    match shape.geometry {
        Geometry::TwoPoints { from, to } => (from, to),
        _ => panic!("two points"),
    }
}

fn points_of(shape: &Shape) -> Vec<Point> {
    match &shape.geometry {
        Geometry::Polyline(points) => points.clone(),
        _ => panic!("a polyline"),
    }
}

fn selected(shape: Shape) -> (Interaction, EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(shape);
    session.select(Some(id));
    (Interaction::new(), session, id)
}

#[test]
fn a_line_an_arrow_a_pen_and_a_highlighter_have_a_round_handle_but_a_step_does_not() {
    for shape in [line(), arrow(0.0), stroke("pen"), stroke("highlighter")] {
        assert!(
            rotation_handle(&shape, None, REACH).is_some(),
            "{} turns",
            shape.kind
        );
    }
    let step = Shape::new("step", Geometry::Anchor(point(0.0, 0.0)), Options::new());
    assert!(rotation_handle(&step, None, REACH).is_none());
    let spotlight = Shape::new(
        "spotlight",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        Options::new(),
    );
    assert!(rotation_handle(&spotlight, None, REACH).is_none());
    let above = rotation_handle(&line(), None, REACH).expect("handle");
    assert!(near(above, point(150.0, 100.0 - 3.0 * REACH)));
}

#[test]
fn turning_the_points_moves_them_around_the_centre_without_storing_an_angle() {
    let turned = turned_points(&line(), point(150.0, 100.0), 90.0);

    let (from, to) = ends_of(&turned);
    assert!(near(from, point(150.0, 50.0)) && near(to, point(150.0, 150.0)));
    assert!(same(rotation_of(&turned), 0.0));
    let flipped = turned_points(&stroke("pen"), point(50.0, 25.0), 180.0);
    let points = points_of(&flipped);
    assert!(near(points[0], point(100.0, 50.0)));
    assert!(near(points[1], point(0.0, 50.0)));
    assert!(near(points[2], point(0.0, 0.0)));
}

#[test]
fn four_quarter_turns_bring_the_points_back_and_the_curve_is_kept() {
    let mut shape = arrow(40.0);
    let pivot = point(150.0, 100.0);
    for _ in 0..4 {
        shape = turned_points(&shape, pivot, 90.0);
    }

    let (from, to) = ends_of(&shape);
    assert!(near(from, point(100.0, 100.0)) && near(to, point(200.0, 100.0)));
    assert!(same(shape.number("curve"), 40.0));
    let once = turned_points(&arrow(40.0), pivot, 30.0);
    assert!(same(once.number("curve"), 40.0));
}

#[test]
fn dragging_the_round_handle_turns_a_line_in_one_undoable_step() {
    let (mut interaction, mut session, id) = selected(line());
    let handle = rotation_handle(&line(), None, REACH).expect("handle");

    turn(&mut interaction, &mut session, handle, point(232.0, 100.0));

    let (from, to) = ends_of(session.shape_of(id).expect("line"));
    assert!(near(from, point(150.0, 50.0)) && near(to, point(150.0, 150.0)));
    assert!(session.undo());
    let (from, to) = ends_of(session.shape_of(id).expect("line"));
    assert!(near(from, point(100.0, 100.0)) && near(to, point(200.0, 100.0)));
    assert!(session.redo());
    let (from, _) = ends_of(session.shape_of(id).expect("line"));
    assert!(near(from, point(150.0, 50.0)));
}

#[test]
fn turning_a_line_again_starts_from_where_it_is_and_never_accumulates_error() {
    let (mut interaction, mut session, id) = selected(line());
    for _ in 0..4 {
        let shape = session.shape_of(id).cloned().expect("line");
        let handle = rotation_handle(&shape, None, REACH).expect("handle");
        let pivot = shape_pivot(&shape, None).expect("pivot");
        turn(
            &mut interaction,
            &mut session,
            handle,
            rotate_point(handle, pivot, 90.0),
        );
    }

    let (from, to) = ends_of(session.shape_of(id).expect("line"));
    assert!((from.x - 100.0).abs() < 0.5 && (from.y - 100.0).abs() < 0.5);
    assert!((to.x - 200.0).abs() < 0.5 && (to.y - 100.0).abs() < 0.5);
}

#[test]
fn holding_shift_turns_a_pen_stroke_by_steps_of_fifteen_degrees() {
    let (mut interaction, mut session, id) = selected(stroke("pen"));
    let shape = session.shape_of(id).cloned().expect("pen");
    let handle = rotation_handle(&shape, None, REACH).expect("handle");
    let pivot = shape_pivot(&shape, None).expect("pivot");
    interaction.set_constrained(true);

    turn(
        &mut interaction,
        &mut session,
        handle,
        rotate_point(handle, pivot, 37.0),
    );

    let expected = turned_points(&shape, pivot, 30.0);
    let (got, want) = (
        points_of(session.shape_of(id).expect("pen")),
        points_of(&expected),
    );
    assert!(got.iter().zip(&want).all(|(a, b)| near(*a, *b)));
}

#[test]
fn a_turned_line_is_picked_where_it_is_seen() {
    let (mut interaction, mut session, id) = selected(line());
    let handle = rotation_handle(&line(), None, REACH).expect("handle");
    turn(&mut interaction, &mut session, handle, point(232.0, 100.0));
    session.select(None);

    assert_eq!(session.pick(point(150.0, 60.0), REACH), Some(id));
    assert_eq!(session.pick(point(190.0, 100.0), REACH), None);
}

#[test]
fn a_locked_line_and_an_unselected_stroke_are_not_turned() {
    let (mut interaction, mut session, id) = selected(line());
    assert!(session.set_locked(id, true));
    let handle = rotation_handle(&line(), None, REACH).expect("handle");
    turn(&mut interaction, &mut session, handle, point(232.0, 100.0));
    let (from, _) = ends_of(session.shape_of(id).expect("line"));
    assert!(near(from, point(100.0, 100.0)));

    let (mut interaction, mut session, id) = selected(stroke("pen"));
    session.select(None);
    let shape = session.shape_of(id).cloned().expect("pen");
    let handle = rotation_handle(&shape, None, REACH).expect("handle");
    turn(&mut interaction, &mut session, handle, point(300.0, 25.0));
    let points = points_of(session.shape_of(id).expect("pen"));
    assert!(near(points[0], point(0.0, 0.0)));
}
