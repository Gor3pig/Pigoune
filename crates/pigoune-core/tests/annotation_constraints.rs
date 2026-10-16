use pigoune_core::annotations::{
    ARROW, AnnotationDocument, ELLIPSE, EditSession, Geometry, GeometryKind, Interaction, Options,
    Point, RECTANGLE, Tool, constrained_end,
};

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn same(left: Point, right: Point) -> bool {
    near(left.x, right.x) && near(left.y, right.y)
}

#[test]
fn a_box_becomes_a_square_on_the_longest_side_in_the_direction_of_the_drag() {
    let origin = point(10.0, 10.0);

    assert!(same(
        constrained_end(origin, point(60.0, 30.0), GeometryKind::Box),
        point(60.0, 60.0)
    ));
    assert!(same(
        constrained_end(origin, point(-20.0, 40.0), GeometryKind::Box),
        point(-20.0, 40.0)
    ));
    assert!(same(
        constrained_end(origin, point(-40.0, -10.0), GeometryKind::Box),
        point(-40.0, -40.0)
    ));
}

#[test]
fn a_segment_snaps_to_the_nearest_multiple_of_forty_five_degrees_and_keeps_its_length() {
    let origin = point(0.0, 0.0);

    let nearly_flat = constrained_end(origin, point(100.0, 8.0), GeometryKind::TwoPoints);
    assert!(near(nearly_flat.y, 0.0));
    assert!(near(nearly_flat.x, 100.0_f32.hypot(8.0)));

    let diagonal = constrained_end(origin, point(50.0, 60.0), GeometryKind::TwoPoints);
    assert!(near(diagonal.x, diagonal.y));
    assert!(near(diagonal.x.hypot(diagonal.y), 50.0_f32.hypot(60.0)));

    let upward = constrained_end(origin, point(3.0, -90.0), GeometryKind::TwoPoints);
    assert!(near(upward.x, 0.0));
    assert!(near(upward.y, -(3.0_f32.hypot(90.0))));
}

#[test]
fn points_and_strokes_are_never_constrained() {
    let end = point(7.0, 3.0);

    assert!(same(
        constrained_end(point(0.0, 0.0), end, GeometryKind::Polyline),
        end
    ));
    assert!(same(
        constrained_end(point(0.0, 0.0), end, GeometryKind::Anchor),
        end
    ));
}

fn drawn_with(
    tool: &'static pigoune_core::annotations::ToolDefinition,
    constrained: bool,
    end: Point,
) -> Geometry {
    let mut session = EditSession::new(AnnotationDocument::default());
    let mut interaction = Interaction::new();
    interaction.set_constrained(constrained);
    let options = Options::defaults(tool.options);
    interaction.press(
        &mut session,
        Tool::Draw(tool),
        &options,
        point(0.0, 0.0),
        5.0,
    );
    interaction.drag(&mut session, end);
    interaction.release(&mut session, end);
    session
        .document()
        .objects
        .last()
        .and_then(|object| object.shape())
        .expect("a shape was drawn")
        .geometry
        .clone()
}

#[test]
fn a_constrained_rectangle_and_ellipse_come_out_square() {
    for tool in [&RECTANGLE, &ELLIPSE] {
        let Geometry::Box(rect) = drawn_with(tool, true, point(120.0, 40.0)) else {
            panic!("a box");
        };
        assert!(near(rect.width, rect.height));
        assert!(near(rect.width, 120.0));
    }
}

#[test]
fn a_free_rectangle_keeps_the_proportions_of_the_drag() {
    let Geometry::Box(rect) = drawn_with(&RECTANGLE, false, point(120.0, 40.0)) else {
        panic!("a box");
    };
    assert!(near(rect.width, 120.0));
    assert!(near(rect.height, 40.0));
}

#[test]
fn a_constrained_arrow_is_horizontal_when_dragged_nearly_flat() {
    let Geometry::TwoPoints { from, to } = drawn_with(&ARROW, true, point(100.0, 9.0)) else {
        panic!("two points");
    };
    assert!(near(from.y, to.y));
    assert!(to.x > from.x);
}
