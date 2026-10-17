use pigoune_core::annotations::{
    AnnotationDocument, DASH, EditSession, Geometry, Options, PEN, Point, RECTANGLE, Rect,
    Roughness, SEED_OPTION, Shape, definition, definitions, shape_bounds,
};
use serde_json::{Value, json};

fn rectangle() -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 60.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

fn sketchy_rectangle() -> Shape {
    rectangle().with_option("roughness", json!("sketchy"))
}

fn seed_of(session: &EditSession, id: u32) -> Option<Value> {
    session
        .shape_of(id)
        .and_then(|shape| shape.options.get(SEED_OPTION).cloned())
}

#[test]
fn only_the_stroke_tools_offer_the_hand_drawn_look() {
    let offering: Vec<&str> = definitions()
        .iter()
        .filter(|tool| tool.spec("roughness").is_some())
        .map(|tool| tool.kind)
        .collect();
    assert_eq!(offering, ["rectangle", "ellipse", "line", "arrow"]);
}

#[test]
fn a_tool_that_hides_content_never_offers_a_hand_drawn_edge() {
    for tool in definitions().iter().filter(|tool| tool.hides_content) {
        assert!(tool.spec("roughness").is_none(), "{}", tool.kind);
        assert!(tool.spec("dash").is_none(), "{}", tool.kind);
    }
}

#[test]
fn the_pen_gets_the_line_type_but_not_the_hand_drawn_look() {
    assert!(PEN.spec("dash").is_some());
    assert!(PEN.spec("roughness").is_none());
    assert!(definition("highlighter").is_some_and(|tool| tool.spec("dash").is_none()));
    assert_eq!(DASH.default_value(), json!("solid"));
}

#[test]
fn a_shape_without_the_option_reads_as_precise() {
    let old = Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        Options::new(),
    );
    assert_eq!(old.roughness(), Roughness::Precise);
    assert_eq!(old.seed(), 0);
}

#[test]
fn an_unknown_or_damaged_choice_reads_as_precise() {
    let odd = rectangle().with_option("roughness", json!("wild"));
    assert_eq!(odd.roughness(), Roughness::Precise);
    let number = rectangle().with_option("roughness", json!(3));
    assert_eq!(number.roughness(), Roughness::Precise);
}

#[test]
fn a_new_annotation_receives_its_own_seed() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(sketchy_rectangle());
    let second = session.add(sketchy_rectangle());
    assert!(seed_of(&session, first).is_some());
    assert_ne!(seed_of(&session, first), seed_of(&session, second));
}

#[test]
fn a_duplicate_keeps_exactly_the_seed_of_the_original() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let original = session.add(sketchy_rectangle());
    let copy = session.duplicate_object(original, 12.0).expect("copy");
    assert_eq!(seed_of(&session, copy), seed_of(&session, original));
    let (a, b) = (
        session.shape_of(original).expect("original"),
        session.shape_of(copy).expect("copy"),
    );
    assert_eq!(a.options, b.options);
    assert_eq!(a.roughness(), b.roughness());
}

#[test]
fn the_look_stays_the_same_when_the_shape_moves_grows_or_changes_color() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(sketchy_rectangle());
    let seed = seed_of(&session, id);
    assert!(session.nudge_selected(5.0, 3.0));
    assert!(session.set_option_on_selected("color", json!({"r": 0, "g": 0, "b": 0, "a": 255})));
    assert!(session.set_option_on_selected("width", json!(9.0)));
    assert_eq!(seed_of(&session, id), seed);
    assert_eq!(
        session.shape_of(id).map(Shape::roughness),
        Some(Roughness::Sketchy)
    );
}

#[test]
fn undo_and_redo_bring_back_the_same_look() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle());
    assert!(session.set_option_on_selected("roughness", json!("very_sketchy")));
    let sketched = session.shape_of(id).cloned().expect("shape");
    assert!(sketched.options.get(SEED_OPTION).is_some());
    assert!(session.undo());
    let precise = session.shape_of(id).expect("shape");
    assert_eq!(precise.roughness(), Roughness::Precise);
    assert!(session.redo());
    assert_eq!(session.shape_of(id), Some(&sketched));
}

#[test]
fn choosing_the_look_and_its_seed_is_one_single_undo_step() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle());
    let before = session.shape_of(id).cloned().expect("shape");
    assert!(session.set_option_on_selected("roughness", json!("sketchy")));
    assert!(session.undo());
    assert_eq!(session.shape_of(id), Some(&before));
}

#[test]
fn asking_again_for_the_same_look_changes_nothing() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle());
    assert!(session.set_option_on_selected("roughness", json!("sketchy")));
    assert!(!session.set_option_on_selected("roughness", json!("sketchy")));
}

#[test]
fn changing_from_one_hand_drawn_level_to_another_keeps_the_seed() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle());
    assert!(session.set_option_on_selected("roughness", json!("sketchy")));
    let seed = seed_of(&session, id);
    assert!(session.set_option_on_selected("roughness", json!("very_sketchy")));
    assert_eq!(seed_of(&session, id), seed);
}

#[test]
fn the_seed_survives_saving_and_reading_the_document() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(sketchy_rectangle());
    let saved = session.document().to_json().expect("saved");
    let read = AnnotationDocument::from_json(&saved).expect("read");
    let reopened = EditSession::restored(read);
    assert_eq!(seed_of(&reopened, id), seed_of(&session, id));
    assert_eq!(
        reopened.shape_of(id).map(Shape::roughness),
        Some(Roughness::Sketchy)
    );
}

#[test]
fn the_hand_drawn_look_does_not_move_the_bounds_used_to_select_a_shape() {
    let precise = rectangle();
    let sketchy = sketchy_rectangle();
    assert_eq!(shape_bounds(&precise), shape_bounds(&sketchy));
    let near = Point { x: 10.0, y: 40.0 };
    assert!(pigoune_core::annotations::hit(&sketchy, near, 6.0));
}
