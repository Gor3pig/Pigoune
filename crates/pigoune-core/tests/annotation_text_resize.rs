use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Handle, Interaction, Options, Point, TEXT, Tool,
    handles, handles_with_extent, resized, resized_with_extent,
};
use serde_json::json;

const REACH: f32 = 6.0;
const EXTENT: (f32, f32) = (100.0, 40.0);

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn text_at(x: f32, y: f32) -> pigoune_core::annotations::Shape {
    pigoune_core::annotations::Shape::new(
        "text",
        Geometry::Anchor(point(x, y)),
        Options::defaults(TEXT.options).with("content", json!("Hello")),
    )
}

fn anchor_of(shape: &pigoune_core::annotations::Shape) -> Point {
    match shape.geometry {
        Geometry::Anchor(at) => at,
        _ => panic!("a text has an anchor"),
    }
}

#[test]
fn a_text_has_eight_handles_around_its_box_once_its_extent_is_known() {
    let shape = text_at(50.0, 60.0);

    assert!(handles(&shape).is_empty());
    let found = handles_with_extent(&shape, Some(EXTENT));
    assert_eq!(found.len(), 8);
    let at = |wanted: Handle| {
        found
            .iter()
            .find(|(handle, _)| *handle == wanted)
            .expect("handle")
            .1
    };
    assert_eq!(at(Handle::TopLeft), point(50.0, 60.0));
    assert_eq!(at(Handle::BottomRight), point(150.0, 100.0));
    assert_eq!(at(Handle::Right), point(150.0, 80.0));
    assert_eq!(at(Handle::Top), point(100.0, 60.0));
}

#[test]
fn the_handles_of_a_text_with_a_background_surround_the_padding_too() {
    let shape = text_at(50.0, 60.0).with_option("background", json!("light"));
    let margin = 32.0 * 0.25;

    let found = handles_with_extent(&shape, Some(EXTENT));
    let top_left = found
        .iter()
        .find(|(handle, _)| *handle == Handle::TopLeft)
        .expect("handle")
        .1;
    assert!(same(top_left.x, 50.0 - margin) && same(top_left.y, 60.0 - margin));
}

#[test]
fn a_corner_scales_the_font_around_the_opposite_corner() {
    let shape = text_at(50.0, 60.0);

    let bigger = resized_with_extent(&shape, Handle::BottomRight, point(250.0, 140.0), EXTENT);
    assert!(same(bigger.number("font_size"), 64.0));
    assert_eq!(anchor_of(&bigger), point(50.0, 60.0));

    let smaller = resized_with_extent(&shape, Handle::TopLeft, point(100.0, 80.0), EXTENT);
    assert!(same(smaller.number("font_size"), 16.0));
    let at = anchor_of(&smaller);
    assert!(same(at.x, 100.0) && same(at.y, 80.0));
}

#[test]
fn a_side_follows_only_its_own_direction() {
    let shape = text_at(50.0, 60.0);

    let wider = resized_with_extent(&shape, Handle::Right, point(250.0, 9999.0), EXTENT);
    assert!(same(wider.number("font_size"), 64.0));
    assert_eq!(anchor_of(&wider), point(50.0, 60.0));

    let taller = resized_with_extent(&shape, Handle::Bottom, point(-500.0, 140.0), EXTENT);
    assert!(same(taller.number("font_size"), 64.0));
}

#[test]
fn the_font_stays_between_its_limits_and_the_box_follows_the_limit() {
    let shape = text_at(50.0, 60.0);

    let huge = resized_with_extent(&shape, Handle::BottomRight, point(90000.0, 90000.0), EXTENT);
    assert!(same(huge.number("font_size"), 200.0));
    assert_eq!(anchor_of(&huge), point(50.0, 60.0));

    let tiny = resized_with_extent(&shape, Handle::TopLeft, point(149.0, 99.0), EXTENT);
    assert!(same(tiny.number("font_size"), 8.0));
    let at = anchor_of(&tiny);
    assert!(same(at.x, 150.0 - 100.0 * 0.25) && same(at.y, 100.0 - 40.0 * 0.25));
}

#[test]
fn without_an_extent_a_text_cannot_be_resized() {
    let shape = text_at(50.0, 60.0);

    assert_eq!(
        resized(&shape, Handle::BottomRight, point(250.0, 140.0)),
        shape
    );
}

#[test]
fn dragging_a_handle_resizes_the_selected_text_in_one_undoable_step() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    session.set_text_measurer(|_| EXTENT);
    let id = session.add(text_at(50.0, 60.0));
    session.select(Some(id));
    let options = Options::defaults(TEXT.options);

    interaction.press(
        &mut session,
        Tool::Select,
        &options,
        point(150.0, 100.0),
        REACH,
    );
    interaction.drag(&mut session, point(200.0, 120.0));
    interaction.release(&mut session, point(250.0, 140.0));

    let resized = session.shape_of(id).expect("text");
    assert!(same(resized.number("font_size"), 64.0));
    assert!(session.undo());
    assert!(same(
        session.shape_of(id).expect("text").number("font_size"),
        32.0
    ));
    assert!(session.redo());
    assert!(same(
        session.shape_of(id).expect("text").number("font_size"),
        64.0
    ));
}

#[test]
fn a_locked_text_shows_no_handle_to_grab() {
    let mut interaction = Interaction::new();
    let mut session = EditSession::new(AnnotationDocument::default());
    session.set_text_measurer(|_| EXTENT);
    let id = session.add(text_at(50.0, 60.0));
    session.select(Some(id));
    assert!(session.set_locked(id, true));
    let options = Options::defaults(TEXT.options);

    interaction.press(
        &mut session,
        Tool::Select,
        &options,
        point(150.0, 100.0),
        REACH,
    );
    interaction.release(&mut session, point(250.0, 140.0));

    assert!(same(
        session.shape_of(id).expect("text").number("font_size"),
        32.0
    ));
}
