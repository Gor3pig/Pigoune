use pigoune_core::annotations::{
    ARROW, AnnotationDocument, EditSession, Geometry, Handle, Interaction, Options, Point, Shape,
    Tool, handles,
};

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn session_with_arrow() -> (EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: point(0.0, 0.0),
            to: point(200.0, 0.0),
        },
        Options::defaults(ARROW.options),
    ));
    (session, id)
}

fn curve_handle(session: &EditSession, id: u32) -> Point {
    handles(session.shape_of(id).expect("arrow"))
        .into_iter()
        .find(|(handle, _)| *handle == Handle::Curve)
        .map(|(_, at)| at)
        .expect("curve handle")
}

fn pull_curve(session: &mut EditSession, id: u32, to: Point, tool: Tool) {
    let mut interaction = Interaction::new();
    let from = curve_handle(session, id);
    assert!(interaction.press(session, tool, &Options::new(), from, REACH));
    interaction.drag(session, to);
    interaction.release(session, to);
}

fn curve_of(session: &EditSession, id: u32) -> f32 {
    session.shape_of(id).expect("arrow").number("curve")
}

#[test]
fn a_straight_arrow_offers_its_curve_handle_in_the_middle() {
    let (session, id) = session_with_arrow();

    let middle = curve_handle(&session, id);

    assert!(near(middle.x, 100.0) && near(middle.y, 0.0));
}

#[test]
fn pulling_the_handle_bends_the_arrow_and_the_handle_follows_the_pointer() {
    let (mut session, id) = session_with_arrow();

    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Select);

    assert!(near(curve_of(&session, id), 60.0));
    let middle = curve_handle(&session, id);
    assert!(near(middle.x, 100.0) && near(middle.y, 30.0));
}

#[test]
fn pulling_to_the_other_side_reverses_the_curve() {
    let (mut session, id) = session_with_arrow();

    pull_curve(&mut session, id, point(100.0, -30.0), Tool::Select);

    assert!(near(curve_of(&session, id), -60.0));
}

#[test]
fn the_handle_does_not_exist_while_a_drawing_tool_is_active() {
    let (mut session, id) = session_with_arrow();

    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Draw(&ARROW));

    assert_eq!(session.count(), 2);
    assert!(near(curve_of(&session, id), 0.0));
}

#[test]
fn a_tiny_pull_leaves_the_arrow_straight() {
    let (mut session, id) = session_with_arrow();

    pull_curve(&mut session, id, point(100.0, 1.0), Tool::Select);

    assert!(near(curve_of(&session, id), 0.0));
}

#[test]
fn the_bend_is_limited() {
    let (mut session, id) = session_with_arrow();

    pull_curve(&mut session, id, point(100.0, 900.0), Tool::Select);

    assert!(near(curve_of(&session, id), 100.0));
}

#[test]
fn a_whole_pull_is_a_single_undo_step() {
    let (mut session, id) = session_with_arrow();
    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Select);

    assert!(session.undo());

    assert!(near(curve_of(&session, id), 0.0));
    assert_eq!(session.count(), 1);
}

#[test]
fn a_double_press_on_the_handle_straightens_the_arrow() {
    let (mut session, id) = session_with_arrow();
    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Select);
    let mut interaction = Interaction::new();

    let straightened =
        interaction.straighten(&mut session, Tool::Select, point(100.0, 30.0), REACH);

    assert!(straightened);
    assert!(near(curve_of(&session, id), 0.0));
    assert!(session.undo());
    assert!(near(curve_of(&session, id), 60.0));
}

#[test]
fn a_double_press_elsewhere_changes_nothing() {
    let (mut session, id) = session_with_arrow();
    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Select);
    let mut interaction = Interaction::new();

    assert!(!interaction.straighten(&mut session, Tool::Select, point(10.0, 200.0), REACH));
    assert!(near(curve_of(&session, id), 60.0));
}

#[test]
fn a_moved_arrow_keeps_its_curve_and_its_tip_follows_the_tangent() {
    let (mut session, id) = session_with_arrow();
    pull_curve(&mut session, id, point(100.0, 30.0), Tool::Select);
    let mut interaction = Interaction::new();

    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        point(50.0, 22.5),
        REACH,
    );
    interaction.drag(&mut session, point(100.0, 62.5));
    interaction.release(&mut session, point(100.0, 62.5));

    assert!(near(curve_of(&session, id), 60.0));
    let middle = curve_handle(&session, id);
    assert!(near(middle.x, 150.0) && near(middle.y, 70.0));
}
