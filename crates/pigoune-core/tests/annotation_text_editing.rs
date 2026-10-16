use pigoune_core::annotations::{
    AnnotationDocument, Crop, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, Rect,
    Shape, TEXT, Tool,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn text(content: &str, x: f32, y: f32) -> Shape {
    Shape::new(
        "text",
        Geometry::Anchor(point(x, y)),
        Options::defaults(TEXT.options),
    )
    .with_option("content", json!(content))
}

fn filled_rectangle(x: f32, y: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y,
            width: 300.0,
            height: 200.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
    .with_option("fill", json!(true))
}

fn content(session: &EditSession, id: u32) -> String {
    session
        .shape_of(id)
        .expect("text")
        .text("content")
        .to_owned()
}

fn anchor(session: &EditSession, id: u32) -> Point {
    match session.shape_of(id).expect("text").geometry {
        Geometry::Anchor(at) => at,
        _ => panic!("anchored"),
    }
}

fn click(session: &mut EditSession, at: Point) {
    let mut interaction = Interaction::new();
    interaction.press(session, Tool::Select, &Options::new(), at, REACH);
    interaction.release(session, at);
}

fn drag(session: &mut EditSession, from: Point, to: Point) {
    let mut interaction = Interaction::new();
    interaction.press(session, Tool::Select, &Options::new(), from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

#[test]
fn a_text_is_found_under_the_pointer_and_nothing_else_is() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.add(filled_rectangle(400.0, 400.0));
    session.select(None);

    assert_eq!(session.text_at(point(110.0, 110.0), REACH), Some(id));
    assert_eq!(session.text_at(point(500.0, 500.0), REACH), None);
    assert_eq!(session.text_at(point(900.0, 900.0), REACH), None);
}

#[test]
fn a_text_covered_by_a_shape_above_it_is_not_the_one_under_the_pointer() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.add(filled_rectangle(90.0, 90.0));
    session.select(None);

    assert_eq!(session.text_at(point(110.0, 110.0), REACH), None);
    assert!(session.shape_of(id).is_some());
}

#[test]
fn a_locked_hidden_or_cropped_out_text_is_not_found() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let locked = session.add(text("Locked", 100.0, 100.0));
    let hidden = session.add(text("Hidden", 100.0, 300.0));
    let cropped = session.add(text("Out", 1000.0, 1000.0));
    session.set_locked(locked, true);
    session.set_hidden(hidden, true);
    session.select(None);

    assert_eq!(session.text_at(point(110.0, 110.0), REACH), None);
    assert_eq!(session.text_at(point(110.0, 310.0), REACH), None);
    session.set_crop(Some(Crop {
        x: 0,
        y: 0,
        width: 500,
        height: 500,
    }));
    assert_eq!(session.text_at(point(1010.0, 1010.0), REACH), None);
    assert!(session.shape_of(cropped).is_some());
}

#[test]
fn a_simple_click_on_a_selected_text_changes_nothing_and_leaves_no_step() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.mark_saved();
    let before = session.document().clone();

    click(&mut session, point(110.0, 110.0));
    click(&mut session, point(110.0, 110.0));

    assert_eq!(session.selected(), Some(id));
    assert_eq!(session.document(), &before);
    assert!(!session.is_modified());
}

#[test]
fn a_drag_on_a_text_moves_it_in_a_single_undoable_step() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.select(None);

    drag(&mut session, point(110.0, 110.0), point(150.0, 130.0));

    assert_eq!(anchor(&session, id), point(140.0, 120.0));
    assert_eq!(session.selected(), Some(id));
    assert!(session.undo());
    assert_eq!(anchor(&session, id), point(100.0, 100.0));
    assert!(session.redo());
    assert_eq!(anchor(&session, id), point(140.0, 120.0));
}

#[test]
fn changing_the_content_is_one_step_that_undo_and_redo_restore_exactly() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    let before = session.document().clone();
    session.select(Some(id));

    assert!(session.set_option_on_selected("content", json!("Bonjour")));
    let after = session.document().clone();
    assert_eq!(content(&session, id), "Bonjour");
    assert_eq!(session.selected(), Some(id));

    assert!(session.undo());
    assert_eq!(session.document(), &before);
    assert_eq!(content(&session, id), "Hello");
    assert!(session.redo());
    assert_eq!(session.document(), &after);
}

#[test]
fn an_unchanged_content_leaves_no_step() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.mark_saved();
    session.select(Some(id));

    assert!(!session.set_option_on_selected("content", json!("Hello")));
    assert!(!session.is_modified());
}

#[test]
fn creating_a_text_is_one_step_selected_and_exactly_restorable() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let before = session.document().clone();

    let id = session.add(text("New", 10.0, 10.0));
    let after = session.document().clone();

    assert_eq!(session.selected(), Some(id));
    assert!(session.undo());
    assert!(session.document().same_content_as(&before));
    assert!(session.redo());
    assert_eq!(session.document(), &after);
    assert_eq!(session.selected(), Some(id));
}

#[test]
fn emptying_a_text_removes_it_and_undo_brings_back_its_words() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.select(Some(id));

    assert!(session.remove_selected());
    assert_eq!(session.count(), 0);
    assert!(session.undo());

    assert_eq!(content(&session, id), "Hello");
    assert_eq!(session.selected(), Some(id));
}

#[test]
fn a_locked_text_refuses_a_change_of_content() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.set_locked(id, true);
    session.select(Some(id));
    let before = session.document().clone();

    assert!(!session.set_option_on_selected("content", json!("Changed")));

    assert_eq!(session.document(), &before);
}

#[test]
fn a_text_edited_before_saving_reopens_with_its_content_and_stays_editable() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text("Hello", 100.0, 100.0));
    session.select(Some(id));
    session.set_option_on_selected("content", json!("Bonjour le monde"));
    let saved = session.document().to_json().expect("serializable");

    let mut again = EditSession::restored(AnnotationDocument::from_json(&saved).expect("readable"));

    assert_eq!(content(&again, id), "Bonjour le monde");
    assert_eq!(again.text_at(point(110.0, 110.0), REACH), Some(id));
    again.select(Some(id));
    assert!(again.set_option_on_selected("content", json!("Salut")));
    assert!(again.undo());
    assert_eq!(content(&again, id), "Bonjour le monde");
    assert!(again.redo());
    assert_eq!(content(&again, id), "Salut");
}
