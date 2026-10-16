use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Options, RECTANGLE, Rect, Shape,
};
use serde_json::json;

fn rectangle(x: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

fn saved(session: &EditSession) -> String {
    session.document().to_json().expect("serializable")
}

fn reopened(text: &str) -> EditSession {
    EditSession::restored(AnnotationDocument::from_json(text).expect("readable"))
}

fn ids(session: &EditSession) -> Vec<u32> {
    session.document().objects.iter().map(|o| o.id).collect()
}

fn with_next_id(text: &str, value: serde_json::Value) -> String {
    let mut document: serde_json::Value = serde_json::from_str(text).expect("json");
    document["next_id"] = value;
    document.to_string()
}

#[test]
fn a_deleted_last_object_never_gives_its_identifier_back_after_a_reopening() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    session.add(rectangle(60.0));
    let doomed = session.add(rectangle(120.0));
    session.select(Some(doomed));
    assert!(session.remove_selected());

    let mut again = reopened(&saved(&session));
    let fresh = again.add(rectangle(180.0));

    assert!(fresh > doomed, "{fresh} must not reuse {doomed}");
}

#[test]
fn undoing_every_creation_then_saving_still_keeps_the_identifiers_used() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    let second = session.add(rectangle(60.0));
    assert!(session.undo() && session.undo());
    assert_eq!(session.count(), 0);

    let mut again = reopened(&saved(&session));
    let fresh = again.add(rectangle(0.0));

    assert!(fresh > second);
}

#[test]
fn identifiers_keep_growing_over_many_sessions() {
    let mut text = saved(&EditSession::new(AnnotationDocument::default()));
    let mut seen = Vec::new();
    for round in 0..6 {
        let mut session = reopened(&text);
        let first = session.add(rectangle(0.0));
        let second = session.add(rectangle(60.0));
        seen.extend([first, second]);
        session.select(Some(second));
        assert!(session.remove_selected(), "round {round}");
        text = saved(&session);
    }
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), seen.len());
}

#[test]
fn a_document_that_never_lost_an_object_keeps_its_old_json() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    session.add(rectangle(60.0));

    assert!(!saved(&session).contains("next_id"));
}

#[test]
fn the_stored_counter_survives_a_second_save_and_stays_out_of_the_unknown_fields() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    let doomed = session.add(rectangle(60.0));
    session.select(Some(doomed));
    session.remove_selected();
    let first = saved(&session);

    let second = saved(&reopened(&first));

    assert_eq!(first, second);
    assert_eq!(second.matches("next_id").count(), 1);
}

#[test]
fn an_invalid_or_too_small_counter_is_ignored_without_refusing_the_document() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    session.add(rectangle(60.0));
    let text = saved(&session);

    for bad in [
        json!(1),
        json!(2),
        json!(0),
        json!(-5),
        json!(3.5),
        json!("12"),
        json!(null),
        json!(u64::MAX),
        json!([1]),
    ] {
        let mut again = reopened(&with_next_id(&text, bad.clone()));
        assert_eq!(ids(&again), vec![1, 2], "{bad}");
        assert_eq!(again.add(rectangle(0.0)), 3, "{bad}");
    }
}

#[test]
fn a_valid_counter_above_the_highest_identifier_is_honoured() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    let text = with_next_id(&saved(&session), json!(40));

    let mut again = reopened(&text);

    assert_eq!(again.add(rectangle(0.0)), 40);
}

#[test]
fn a_document_without_the_counter_falls_back_to_the_highest_identifier() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    session.add(rectangle(60.0));
    let mut document: serde_json::Value = serde_json::from_str(&saved(&session)).expect("json");
    document.as_object_mut().expect("object").remove("next_id");

    let mut again = reopened(&document.to_string());

    assert_eq!(again.add(rectangle(0.0)), 3);
}

#[test]
fn undo_and_redo_restore_the_editable_content_while_the_counter_only_grows() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    let before = session.document().clone();
    let second = session.add(rectangle(60.0));
    let after = session.document().clone();

    assert!(session.undo());
    assert!(session.document().same_content_as(&before));
    assert!(session.redo());
    assert!(session.document().same_content_as(&after));
    assert!(session.undo());
    let third = session.add(rectangle(120.0));
    assert!(third > second);
    assert_ne!(session.document(), &before, "the full documents differ");
}

#[test]
fn unknown_document_fields_are_still_kept_next_to_the_counter() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    let doomed = session.add(rectangle(60.0));
    session.select(Some(doomed));
    session.remove_selected();
    let mut document: serde_json::Value = serde_json::from_str(&saved(&session)).expect("json");
    document["from_the_future"] = json!({"kept": true});

    let again = reopened(&document.to_string());

    let back: serde_json::Value = serde_json::from_str(&saved(&again)).expect("json");
    assert_eq!(back["from_the_future"], json!({"kept": true}));
    assert!(back["next_id"].is_u64());
}

#[test]
fn duplicate_identifiers_are_still_refused() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(rectangle(0.0));
    session.add(rectangle(60.0));
    let text = saved(&session).replacen("\"id\":2", "\"id\":1", 1);

    assert!(AnnotationDocument::from_json(&text).is_err());
}
