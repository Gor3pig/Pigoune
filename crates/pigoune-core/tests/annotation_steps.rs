use pigoune_core::annotations::{
    ARROW, AnnotationDocument, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, STEP,
    Shape, Tool, handles, hit, number_to_value, shape_bounds, translated,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn step_at(x: f32, y: f32) -> Shape {
    Shape::new(
        "step",
        Geometry::Anchor(point(x, y)),
        Options::defaults(STEP.options),
    )
}

fn rectangle() -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(pigoune_core::annotations::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

fn fresh() -> (Interaction, EditSession) {
    (
        Interaction::new(),
        EditSession::new(AnnotationDocument::default()),
    )
}

fn anchor_of(shape: &Shape) -> Point {
    match shape.geometry {
        Geometry::Anchor(at) => at,
        ref other => panic!("not an anchor: {other:?}"),
    }
}

#[test]
fn a_click_places_a_selected_step_where_the_pointer_is() {
    let (mut interaction, mut session) = fresh();

    interaction.press(
        &mut session,
        Tool::Draw(&STEP),
        &Options::defaults(STEP.options),
        point(120.0, 80.0),
        REACH,
    );
    assert!(interaction.pending().is_some());
    interaction.release(&mut session, point(120.0, 80.0));

    assert_eq!(session.count(), 1);
    let shape = session.selected_shape().expect("the step is selected");
    assert_eq!(shape.kind, "step");
    assert_eq!(anchor_of(shape), point(120.0, 80.0));
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn the_step_follows_the_pointer_until_it_is_released() {
    let (mut interaction, mut session) = fresh();
    interaction.press(
        &mut session,
        Tool::Draw(&STEP),
        &Options::defaults(STEP.options),
        point(10.0, 10.0),
        REACH,
    );

    interaction.drag(&mut session, point(60.0, 40.0));
    assert_eq!(
        anchor_of(interaction.pending().expect("a preview")),
        point(60.0, 40.0)
    );
    assert_eq!(session.count(), 0);
    interaction.release(&mut session, point(90.0, 70.0));

    assert_eq!(
        anchor_of(session.selected_shape().expect("a step")),
        point(90.0, 70.0)
    );
    assert!(interaction.pending().is_none());
}

#[test]
fn escape_while_placing_a_step_adds_nothing() {
    let (mut interaction, mut session) = fresh();
    interaction.press(
        &mut session,
        Tool::Draw(&STEP),
        &Options::defaults(STEP.options),
        point(10.0, 10.0),
        REACH,
    );

    interaction.cancel(&mut session);

    assert!(interaction.pending().is_none());
    assert_eq!(session.count(), 0);
}

#[test]
fn a_step_is_hit_inside_its_disc_and_a_bigger_badge_is_easier_to_hit() {
    let small = step_at(100.0, 100.0).with_option("size", number_to_value(10.0));
    let big = small.with_option("size", number_to_value(40.0));

    assert!(hit(&small, point(100.0, 100.0), 0.0));
    assert!(hit(&small, point(108.0, 100.0), 0.0));
    assert!(!hit(&small, point(125.0, 100.0), 0.0));
    assert!(hit(&small, point(113.0, 100.0), 4.0));
    assert!(hit(&big, point(125.0, 100.0), 0.0));
    assert!(!hit(&big, point(150.0, 100.0), 0.0));
}

#[test]
fn a_step_has_a_square_frame_around_its_disc_and_one_size_handle() {
    let shape = step_at(100.0, 50.0).with_option("size", number_to_value(20.0));

    let bounds = shape_bounds(&shape).expect("the disc has bounds");

    assert!(same(bounds.x, 80.0) && same(bounds.y, 30.0));
    assert!(same(bounds.width, 40.0) && same(bounds.height, 40.0));
    let found = handles(&shape);
    assert_eq!(found.len(), 1);
    assert!(same(found[0].1.x, 120.0) && same(found[0].1.y, 70.0));
}

#[test]
fn a_step_can_be_selected_and_moved() {
    let (mut interaction, mut session) = fresh();
    let id = session.add(step_at(100.0, 100.0));
    session.select(None);

    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        point(105.0, 100.0),
        REACH,
    );
    interaction.drag(&mut session, point(145.0, 130.0));
    interaction.release(&mut session, point(155.0, 140.0));

    assert_eq!(session.selected(), Some(id));
    assert_eq!(
        anchor_of(session.shape_of(id).expect("shape")),
        point(150.0, 140.0)
    );
    assert!(session.undo());
    assert_eq!(
        anchor_of(session.shape_of(id).expect("shape")),
        point(100.0, 100.0)
    );
    assert_eq!(
        translated(&step_at(1.0, 2.0), 3.0, 3.0).geometry,
        Geometry::Anchor(point(4.0, 5.0))
    );
}

#[test]
fn the_defaults_are_a_red_filled_badge() {
    let plain = Shape::new("step", Geometry::Anchor(point(0.0, 0.0)), Options::new());

    assert!(same(plain.number("size"), 16.0));
    assert_eq!(plain.choice("style"), "filled");
    assert_eq!(plain.color("color").r, 0xe0);
    assert!(same(plain.number("opacity"), 1.0));
    let outlined = plain.with_option("style", json!("outlined"));
    assert_eq!(outlined.choice("style"), "outlined");
    assert_eq!(plain.choice("style"), "filled");
}

#[test]
fn the_number_of_a_step_is_its_rank_among_the_steps_only() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(step_at(10.0, 10.0));
    let rectangle_id = session.add(rectangle());
    let second = session.add(step_at(50.0, 10.0));
    let third = session.add(step_at(90.0, 10.0));
    let document = session.document();

    assert_eq!(document.ordinal(first), Some(1));
    assert_eq!(document.ordinal(second), Some(2));
    assert_eq!(document.ordinal(third), Some(3));
    assert_eq!(document.ordinal(rectangle_id), Some(1));
    assert_eq!(document.next_ordinal("step"), 4);
    assert_eq!(document.next_ordinal("arrow"), 1);
    assert_eq!(document.ordinal(9999), None);
    assert_eq!(ARROW.kind, "arrow");
}

#[test]
fn deleting_a_step_renumbers_the_following_ones_and_undo_restores_them() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(step_at(10.0, 10.0));
    let second = session.add(step_at(50.0, 10.0));
    let third = session.add(step_at(90.0, 10.0));
    session.select(Some(second));

    session.remove_selected();

    assert_eq!(session.document().ordinal(first), Some(1));
    assert_eq!(session.document().ordinal(third), Some(2));
    assert_eq!(session.document().next_ordinal("step"), 3);
    session.undo();
    assert_eq!(session.document().ordinal(second), Some(2));
    assert_eq!(session.document().ordinal(third), Some(3));
}

#[test]
fn unknown_tools_do_not_take_a_number() {
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[
            {"id":1,"kind":"lasso","points":[]},
            {"id":2,"kind":"step","at":{"x":1.0,"y":2.0}},
            {"id":3,"kind":"lasso","points":[]},
            {"id":4,"kind":"step","at":{"x":9.0,"y":9.0}}
        ]}"#,
    )
    .expect("read");

    assert_eq!(document.ordinal(2), Some(1));
    assert_eq!(document.ordinal(4), Some(2));
    assert_eq!(document.ordinal(1), None);
    assert_eq!(document.next_ordinal("step"), 3);
}

#[test]
fn a_step_travels_through_the_document_with_its_options() {
    let shape = step_at(12.5, 7.25)
        .with_option("size", number_to_value(30.0))
        .with_option("style", json!("outlined"));
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(shape.clone());

    let text = session.document().to_json().expect("written");
    let read = AnnotationDocument::from_json(&text).expect("read");

    assert_eq!(read.objects[0].shape(), Some(&shape));
}
