use pigoune_core::annotations::{
    ARROW, AnnotationDocument, Crop, EditSession, Geometry, Handle, Interaction, LayerMove,
    Options, Point, RECTANGLE, Rect, Shape, Tool, handles, number_to_value, shape_bounds,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn drag(
    interaction: &mut Interaction,
    session: &mut EditSession,
    tool: Tool,
    options: &Options,
    from: Point,
    to: Point,
) {
    interaction.press(session, tool, options, from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

fn curve_handle(session: &EditSession, id: u32) -> Point {
    handles(session.shape_of(id).expect("arrow"))
        .into_iter()
        .find(|(handle, _)| *handle == Handle::Curve)
        .map(|(_, at)| at)
        .expect("curve handle")
}

fn snapshot(session: &EditSession) -> String {
    session.document().to_json().expect("serializable")
}

struct Journey {
    session: EditSession,
    interaction: Interaction,
    rectangle: u32,
    arrow: u32,
}

fn journey() -> Journey {
    let mut session = EditSession::new(AnnotationDocument::default());
    let mut interaction = Interaction::new();
    let rectangle_options = Options::defaults(RECTANGLE.options);
    let arrow_options = Options::defaults(ARROW.options);

    drag(
        &mut interaction,
        &mut session,
        Tool::Draw(&RECTANGLE),
        &rectangle_options,
        point(100.0, 100.0),
        point(300.0, 220.0),
    );
    let rectangle = session.selected().expect("rectangle selected");
    drag(
        &mut interaction,
        &mut session,
        Tool::Draw(&ARROW),
        &arrow_options,
        point(400.0, 400.0),
        point(700.0, 300.0),
    );
    let arrow = session.selected().expect("arrow selected");
    assert!(session.set_option_on_selected("head", json!("open")));
    assert!(session.set_option_on_selected("width", number_to_value(10.0)));
    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        &Options::new(),
        point(100.0, 160.0),
        point(150.0, 200.0),
    );
    assert_eq!(session.selected(), Some(rectangle));
    session.select(Some(arrow));
    let middle = curve_handle(&session, arrow);
    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        &arrow_options,
        middle,
        point(middle.x + 20.0, middle.y + 60.0),
    );
    session.select(Some(rectangle));
    assert!(session.move_selected(LayerMove::Front));
    Journey {
        session,
        interaction,
        rectangle,
        arrow,
    }
}

#[test]
fn every_step_of_the_journey_lands_where_expected() {
    let journey = journey();
    let session = &journey.session;

    let rectangle =
        shape_bounds(session.shape_of(journey.rectangle).expect("rectangle")).expect("bounds");
    assert!(near(rectangle.x, 150.0) && near(rectangle.y, 140.0));
    let arrow = session.shape_of(journey.arrow).expect("arrow");
    assert_eq!(arrow.choice("head"), "open");
    assert!(near(arrow.number("width"), 10.0));
    assert!(arrow.number("curve").abs() > 1.0);
    assert_eq!(arrow.choice("shaft"), "tapered");
    assert_eq!(
        session.document().objects.last().map(|object| object.id),
        Some(journey.rectangle)
    );
}

#[test]
fn undoing_everything_then_redoing_everything_restores_the_same_document() {
    let mut journey = journey();
    let finished = snapshot(&journey.session);

    while journey.session.undo() {}
    assert_eq!(journey.session.count(), 0);
    while journey.session.redo() {}

    assert_eq!(snapshot(&journey.session), finished);
}

#[test]
fn undo_then_a_new_action_discards_the_old_redo_branch() {
    let mut journey = journey();
    assert!(journey.session.undo());
    assert!(journey.session.can_redo());

    journey.session.select(Some(journey.arrow));
    assert!(
        journey
            .session
            .set_option_on_selected("head", json!("double"))
    );

    assert!(!journey.session.can_redo());
}

#[test]
fn a_saved_and_reopened_document_keeps_everything_editable() {
    let journey = journey();
    let saved = snapshot(&journey.session);

    let reopened = AnnotationDocument::from_json(&saved).expect("readable");
    let mut session = EditSession::restored(reopened);
    let mut interaction = Interaction::new();

    assert_eq!(snapshot(&session), saved);
    drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        &Options::new(),
        point(150.0, 200.0),
        point(200.0, 250.0),
    );
    assert_eq!(session.selected(), Some(journey.rectangle));
    let moved =
        shape_bounds(session.shape_of(journey.rectangle).expect("rectangle")).expect("bounds");
    assert!(near(moved.x, 200.0) && near(moved.y, 190.0));
    assert!(session.is_modified());
}

#[test]
fn the_front_shape_wins_the_picking_and_the_order_survives_a_reload() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let options = Options::defaults(RECTANGLE.options).with("fill", json!(true));
    for _ in 0..2 {
        session.add(Shape::new(
            "rectangle",
            Geometry::Box(Rect {
                x: 100.0,
                y: 100.0,
                width: 200.0,
                height: 200.0,
            }),
            options.clone(),
        ));
        session.select(None);
    }
    let ids: Vec<u32> = session.document().objects.iter().map(|o| o.id).collect();
    assert_eq!(session.pick(point(200.0, 200.0), REACH), Some(ids[1]));

    session.select(Some(ids[0]));
    assert!(session.move_selected(LayerMove::Front));
    assert_eq!(session.pick(point(200.0, 200.0), REACH), Some(ids[0]));

    let reloaded = EditSession::restored(
        AnnotationDocument::from_json(&snapshot(&session)).expect("readable"),
    );
    assert_eq!(reloaded.pick(point(200.0, 200.0), REACH), Some(ids[0]));
}

#[test]
fn cropping_keeps_coordinates_and_hides_what_is_outside_the_frame() {
    let mut journey = journey();
    let before = snapshot(&journey.session);

    assert!(journey.session.set_crop(Some(Crop {
        x: 80,
        y: 80,
        width: 300,
        height: 200,
    })));

    let after = AnnotationDocument::from_json(&snapshot(&journey.session)).expect("readable");
    let arrow_origin =
        shape_bounds(journey.session.shape_of(journey.arrow).expect("arrow")).expect("bounds");
    assert!(arrow_origin.x > 380.0);
    assert_eq!(
        journey
            .session
            .pick(point(arrow_origin.x + 5.0, arrow_origin.y + 5.0), 30.0),
        None
    );
    assert!(journey.session.pick(point(150.0, 140.0), REACH).is_some());
    assert_eq!(after.objects.len(), 2);

    assert!(journey.session.undo());
    assert_eq!(snapshot(&journey.session), before);
    let _ = &mut journey.interaction;
}

#[test]
fn an_interrupted_gesture_leaves_no_trace() {
    let mut journey = journey();
    let before = snapshot(&journey.session);
    journey.session.select(Some(journey.rectangle));

    journey.interaction.press(
        &mut journey.session,
        Tool::Select,
        &Options::new(),
        point(200.0, 200.0),
        REACH,
    );
    journey
        .interaction
        .drag(&mut journey.session, point(500.0, 500.0));
    journey.interaction.cancel(&mut journey.session);

    assert_eq!(snapshot(&journey.session), before);
    assert!(!journey.interaction.is_active());
}
