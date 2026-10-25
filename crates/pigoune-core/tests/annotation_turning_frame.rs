use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, Rect, Shape,
    Tool, definition, number_to_value, rotate_point, rotation_handle, shape_bounds, shape_pivot,
};

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn near(left: Point, right: Point) -> bool {
    same(left.x, right.x) && same(left.y, right.y)
}

fn slanted_arrow() -> Shape {
    Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: point(100.0, 100.0),
            to: point(200.0, 140.0),
        },
        Options::defaults(definition("arrow").expect("tool").options),
    )
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
        Options::defaults(RECTANGLE.options).with("rotation", number_to_value(rotation)),
    )
}

fn selected(shape: Shape) -> (Interaction, EditSession) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(shape);
    session.select(Some(id));
    (Interaction::new(), session)
}

fn grab(interaction: &mut Interaction, session: &mut EditSession, shape: &Shape) -> Point {
    let handle = rotation_handle(shape, None, REACH).expect("handle");
    let options = Options::defaults(RECTANGLE.options);
    assert!(interaction.press(session, Tool::Select, &options, handle, REACH));
    handle
}

#[test]
fn nothing_is_turning_before_the_round_handle_is_grabbed_and_after_it_is_released() {
    let shape = slanted_arrow();
    let (mut interaction, mut session) = selected(shape.clone());
    assert!(interaction.turning(REACH).is_none());

    let handle = grab(&mut interaction, &mut session, &shape);
    assert!(interaction.turning(REACH).is_some());
    interaction.release(&mut session, handle);

    assert!(interaction.turning(REACH).is_none());
}

#[test]
fn the_frame_of_a_turning_arrow_keeps_the_shape_it_had_when_it_was_grabbed() {
    let shape = slanted_arrow();
    let bounds = shape_bounds(&shape).expect("bounds");
    let pivot = shape_pivot(&shape, None).expect("pivot");
    let (mut interaction, mut session) = selected(shape.clone());
    let handle = grab(&mut interaction, &mut session, &shape);

    for degrees in [20.0, 70.0, 130.0, -45.0] {
        interaction.drag(&mut session, rotate_point(handle, pivot, degrees));
        let turning = interaction.turning(REACH).expect("turning");
        let frame = turning.rigid.expect("a rigid frame");

        assert_eq!(frame.bounds, bounds);
        assert!(near(turning.pivot, pivot));
        assert!(same(turning.degrees, degrees));
        assert!(near(frame.handle, rotate_point(handle, pivot, degrees)));
    }
}

#[test]
fn the_box_around_the_points_changes_while_the_rigid_frame_does_not() {
    let shape = slanted_arrow();
    let pivot = shape_pivot(&shape, None).expect("pivot");
    let (mut interaction, mut session) = selected(shape.clone());
    let handle = grab(&mut interaction, &mut session, &shape);

    interaction.drag(&mut session, rotate_point(handle, pivot, 90.0));

    let previewed = session.selected_shape().expect("shape");
    assert_ne!(shape_bounds(previewed), shape_bounds(&shape));
    let frame = interaction
        .turning(REACH)
        .and_then(|turning| turning.rigid)
        .expect("a rigid frame");
    assert_eq!(Some(frame.bounds), shape_bounds(&shape));
}

#[test]
fn holding_shift_shows_the_angle_that_is_really_applied() {
    let shape = slanted_arrow();
    let pivot = shape_pivot(&shape, None).expect("pivot");
    let (mut interaction, mut session) = selected(shape.clone());
    let handle = grab(&mut interaction, &mut session, &shape);
    interaction.set_constrained(true);

    interaction.drag(&mut session, rotate_point(handle, pivot, 37.0));

    let turning = interaction.turning(REACH).expect("turning");
    assert!(same(turning.degrees, 30.0));
}

#[test]
fn a_shape_turned_by_option_shows_its_angle_and_has_no_rigid_frame() {
    let shape = rectangle(20.0);
    let pivot = shape_pivot(&shape, None).expect("pivot");
    let (mut interaction, mut session) = selected(shape.clone());
    let handle = grab(&mut interaction, &mut session, &shape);
    assert!(same(
        interaction.turning(REACH).expect("turning").degrees,
        20.0
    ));

    interaction.drag(&mut session, rotate_point(handle, pivot, 40.0));

    let turning = interaction.turning(REACH).expect("turning");
    assert!(turning.rigid.is_none());
    assert!(same(turning.degrees, 60.0));
    assert!(near(turning.pivot, pivot));
}

#[test]
fn a_pen_stroke_gets_the_same_rigid_frame_as_an_arrow() {
    let shape = Shape::new(
        "pen",
        Geometry::Polyline(vec![point(0.0, 0.0), point(100.0, 0.0), point(100.0, 50.0)]),
        Options::defaults(definition("pen").expect("tool").options),
    );
    let pivot = shape_pivot(&shape, None).expect("pivot");
    let (mut interaction, mut session) = selected(shape.clone());
    let handle = grab(&mut interaction, &mut session, &shape);

    interaction.drag(&mut session, rotate_point(handle, pivot, 50.0));

    let frame = interaction
        .turning(REACH)
        .and_then(|turning| turning.rigid)
        .expect("a rigid frame");
    assert_eq!(Some(frame.bounds), shape_bounds(&shape));
}
