use pigoune_core::annotations::{
    AnnotationDocument, Crop, CursorKind, EditSession, Geometry, HitMode, Interaction, Options,
    Point, Rect, Shape, Tool, definition, number_to_value, shape_bounds,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn boxed(kind: &str, x: f32, y: f32, width: f32, height: f32) -> Shape {
    let sheet = definition(kind).expect("tool");
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        Options::defaults(sheet.options),
    )
}

fn drag(session: &mut EditSession, tool: Tool, from: Point, to: Point) {
    let mut interaction = Interaction::new();
    let options = match tool {
        Tool::Draw(sheet) => Options::defaults(sheet.options),
        _ => Options::new(),
    };
    interaction.press(session, tool, &options, from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

fn drawing(kind: &str) -> Tool {
    Tool::Draw(definition(kind).expect("tool"))
}

fn origin(session: &EditSession, id: u32) -> (f32, f32) {
    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    (bounds.x, bounds.y)
}

#[test]
fn a_rectangle_can_be_drawn_inside_the_one_that_was_just_drawn() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let big = session.add(boxed("rectangle", 100.0, 100.0, 400.0, 300.0));

    drag(
        &mut session,
        drawing("rectangle"),
        point(200.0, 200.0),
        point(300.0, 300.0),
    );

    assert_eq!(session.count(), 2);
    assert_eq!(origin(&session, big), (100.0, 100.0));
}

#[test]
fn a_blur_can_be_drawn_over_a_selected_blur() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(boxed("blur", 100.0, 100.0, 300.0, 200.0));

    drag(
        &mut session,
        drawing("blur"),
        point(200.0, 150.0),
        point(450.0, 350.0),
    );

    assert_eq!(session.count(), 2);
    assert_eq!(origin(&session, first), (100.0, 100.0));
}

#[test]
fn a_filled_shape_and_a_spotlight_are_drawn_over_too() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let filled = session
        .add(boxed("rectangle", 100.0, 100.0, 400.0, 300.0).with_option("fill", json!(true)));
    session.add(boxed("spotlight", 600.0, 100.0, 300.0, 300.0));
    session.select(None);

    drag(
        &mut session,
        drawing("ellipse"),
        point(200.0, 200.0),
        point(300.0, 300.0),
    );
    drag(
        &mut session,
        drawing("ellipse"),
        point(700.0, 200.0),
        point(800.0, 300.0),
    );

    assert_eq!(session.count(), 4);
    assert_eq!(origin(&session, filled), (100.0, 100.0));
}

#[test]
fn a_region_is_grabbed_by_its_border_under_a_drawing_tool() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let region = session.add(boxed("pixelate", 100.0, 100.0, 300.0, 200.0));
    session.select(None);

    drag(
        &mut session,
        drawing("rectangle"),
        point(100.0, 200.0),
        point(130.0, 230.0),
    );

    assert_eq!(session.count(), 1);
    assert_eq!(origin(&session, region), (130.0, 130.0));
}

#[test]
fn the_selection_tool_still_grabs_the_inside_of_everything() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let region = session.add(boxed("blur", 100.0, 100.0, 300.0, 200.0));
    let hollow = session.add(boxed("rectangle", 500.0, 100.0, 300.0, 200.0));
    session.select(None);

    drag(
        &mut session,
        Tool::Select,
        point(250.0, 200.0),
        point(260.0, 210.0),
    );
    drag(
        &mut session,
        Tool::Select,
        point(650.0, 200.0),
        point(660.0, 210.0),
    );

    assert_eq!(origin(&session, region), (110.0, 110.0));
    assert_eq!(origin(&session, hollow), (510.0, 110.0));
}

#[test]
fn a_text_and_a_badge_are_grabbed_by_their_box_under_a_drawing_tool() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let text_sheet = definition("text").expect("text");
    let text = session.add(
        Shape::new(
            "text",
            Geometry::Anchor(point(100.0, 100.0)),
            Options::defaults(text_sheet.options),
        )
        .with_option("content", json!("Hello")),
    );
    let step_sheet = definition("step").expect("step");
    let step = session.add(Shape::new(
        "step",
        Geometry::Anchor(point(400.0, 100.0)),
        Options::defaults(step_sheet.options),
    ));
    session.select(None);

    drag(
        &mut session,
        drawing("rectangle"),
        point(110.0, 110.0),
        point(130.0, 120.0),
    );
    session.select(None);
    drag(
        &mut session,
        drawing("rectangle"),
        point(402.0, 102.0),
        point(412.0, 112.0),
    );

    assert_eq!(session.count(), 2);
    let Geometry::Anchor(step_at) = session.shape_of(step).expect("step").geometry else {
        panic!("anchored")
    };
    assert!(near(step_at.x, 410.0) && near(step_at.y, 110.0));
    let Geometry::Anchor(moved_text) = session.shape_of(text).expect("text").geometry else {
        panic!("anchored")
    };
    assert!(near(moved_text.x, 120.0) && near(moved_text.y, 110.0));
}

#[test]
fn the_cursor_follows_the_same_rule() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 100.0, 100.0, 400.0, 300.0));
    let tool = drawing("rectangle");

    assert_eq!(
        Interaction::cursor(&session, tool, point(300.0, 250.0), REACH),
        CursorKind::Crosshair
    );
    assert_eq!(
        Interaction::cursor(&session, tool, point(100.0, 170.0), REACH),
        CursorKind::Move
    );
    assert_eq!(
        Interaction::cursor(&session, Tool::Select, point(300.0, 250.0), REACH),
        CursorKind::Move
    );
}

#[test]
fn a_text_outside_the_crop_cannot_be_picked() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let text_sheet = definition("text").expect("text");
    session.add(
        Shape::new(
            "text",
            Geometry::Anchor(point(20.0, 20.0)),
            Options::defaults(text_sheet.options),
        )
        .with_option("content", json!("Hello")),
    );
    assert!(
        session
            .pick(point(25.0, 25.0), REACH, HitMode::Edge)
            .is_some()
    );

    session.set_crop(Some(Crop {
        x: 400,
        y: 300,
        width: 300,
        height: 200,
    }));

    assert_eq!(session.pick(point(25.0, 25.0), REACH, HitMode::Edge), None);
}

#[test]
fn a_series_of_nudges_is_a_single_undo_step_and_other_edits_end_the_series() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));

    for _ in 0..30 {
        assert!(session.nudge_selected(1.0, 0.0));
    }
    assert!(near(origin(&session, id).0, 130.0));
    assert!(session.undo());
    assert!(near(origin(&session, id).0, 100.0));
    assert!(session.undo());
    assert_eq!(session.count(), 0);

    assert!(session.redo());
    assert!(session.redo());
    assert!(near(origin(&session, id).0, 130.0));
}

#[test]
fn a_new_selection_or_another_edit_starts_a_new_nudge_series() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));
    session.nudge_selected(5.0, 0.0);
    session.set_option_on_selected("width", number_to_value(9.0));
    session.nudge_selected(5.0, 0.0);
    session.nudge_selected(5.0, 0.0);

    assert!(session.undo());
    assert!(near(origin(&session, id).0, 105.0));
    assert!(session.undo());
    assert!(session.undo());
    assert!(near(origin(&session, id).0, 100.0));
}

#[test]
fn nudging_after_a_save_makes_the_document_modified_again() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));
    session.nudge_selected(1.0, 0.0);
    session.mark_saved();
    assert!(!session.is_modified());

    session.nudge_selected(1.0, 0.0);

    assert!(session.is_modified());
}
