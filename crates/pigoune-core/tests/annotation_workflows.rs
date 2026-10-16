use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, Rect, Shape,
    Tool, shape_bounds,
};

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn hollow_rectangle(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

fn drag(
    interaction: &mut Interaction,
    session: &mut EditSession,
    tool: Tool,
    from: Point,
    to: Point,
) {
    let options = Options::defaults(RECTANGLE.options);
    interaction.press(session, tool, &options, from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

fn origin_of(session: &EditSession, id: u32) -> (f32, f32) {
    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    (bounds.x, bounds.y)
}

#[test]
fn a_hollow_rectangle_can_be_moved_by_grabbing_its_inside() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow_rectangle(100.0, 100.0, 200.0, 120.0));
    session.select(None);

    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(200.0, 160.0),
        point(240.0, 190.0),
    );

    assert_eq!(origin_of(&session, id), (140.0, 130.0));
}

#[test]
fn a_new_shape_stays_manipulable_with_the_drawing_tool() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let tool = Tool::Draw(&RECTANGLE);
    drag(
        &mut interaction,
        &mut session,
        tool,
        point(100.0, 100.0),
        point(300.0, 220.0),
    );
    let id = session.selected().expect("selected after drawing");

    drag(
        &mut interaction,
        &mut session,
        tool,
        point(100.0, 100.0),
        point(150.0, 130.0),
    );

    assert_eq!(session.count(), 1);
    assert_eq!(session.selected(), Some(id));
    assert_eq!(origin_of(&session, id), (150.0, 130.0));
}

#[test]
fn a_new_shape_can_be_resized_by_its_handle_with_the_drawing_tool() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let tool = Tool::Draw(&RECTANGLE);
    drag(
        &mut interaction,
        &mut session,
        tool,
        point(100.0, 100.0),
        point(300.0, 220.0),
    );
    let id = session.selected().expect("selected after drawing");

    drag(
        &mut interaction,
        &mut session,
        tool,
        point(300.0, 220.0),
        point(400.0, 300.0),
    );

    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    assert_eq!(session.count(), 1);
    assert_eq!((bounds.width, bounds.height), (300.0, 200.0));
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn badge_size(session: &EditSession, id: u32) -> f32 {
    session.shape_of(id).expect("badge").number("size")
}

fn stroke(points: &[(f32, f32)]) -> Shape {
    Shape::new(
        "pen",
        Geometry::Polyline(points.iter().map(|(x, y)| point(*x, *y)).collect()),
        Options::new(),
    )
}

fn points_of(session: &EditSession, id: u32) -> Vec<Point> {
    match &session.shape_of(id).expect("shape").geometry {
        Geometry::Polyline(points) => points.clone(),
        other => panic!("not a stroke: {other:?}"),
    }
}

#[test]
fn a_stroke_is_scaled_by_its_corner_handle() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(stroke(&[(100.0, 100.0), (150.0, 200.0), (200.0, 100.0)]));

    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(200.0, 200.0),
        point(300.0, 400.0),
    );

    let scaled = points_of(&session, id);
    assert_eq!(scaled[0], point(100.0, 100.0));
    assert_eq!(scaled[1], point(200.0, 400.0));
    assert_eq!(scaled[2], point(300.0, 100.0));
}

#[test]
fn scaling_a_stroke_is_one_undo_step() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(stroke(&[(100.0, 100.0), (150.0, 200.0), (200.0, 100.0)]));
    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(200.0, 200.0),
        point(300.0, 400.0),
    );

    assert!(session.undo());

    assert_eq!(points_of(&session, id)[2], point(200.0, 100.0));
}

#[test]
fn a_flat_stroke_can_still_be_stretched_without_dividing_by_zero() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(stroke(&[(100.0, 100.0), (200.0, 100.0)]));

    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(200.0, 100.0),
        point(300.0, 150.0),
    );

    assert!(
        points_of(&session, id)
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
    );
}

#[test]
fn a_badge_is_resized_by_a_handle_within_its_limits() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let step = pigoune_core::annotations::definition("step").expect("step");
    let id = session.add(Shape::new(
        "step",
        Geometry::Anchor(point(100.0, 100.0)),
        Options::defaults(step.options),
    ));

    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(116.0, 116.0),
        point(130.0, 130.0),
    );
    assert!(near(badge_size(&session, id), 30.0));

    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(130.0, 130.0),
        point(900.0, 900.0),
    );
    assert!(near(badge_size(&session, id), 60.0));
}

#[test]
fn the_placing_tool_grabs_shapes_but_lets_a_click_in_the_empty_through() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow_rectangle(100.0, 100.0, 200.0, 120.0));
    session.select(None);
    let options = Options::new();

    assert!(!interaction.press(
        &mut session,
        Tool::Place,
        &options,
        point(200.0, 160.0),
        REACH
    ));
    assert!(interaction.press(
        &mut session,
        Tool::Place,
        &options,
        point(100.0, 160.0),
        REACH
    ));
    interaction.release(&mut session, point(100.0, 160.0));

    assert_eq!(session.selected(), Some(id));
}
