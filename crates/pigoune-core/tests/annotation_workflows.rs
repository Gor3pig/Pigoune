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
