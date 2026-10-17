use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, LayerMove, Options, Point, RECTANGLE, Rect, Refusal,
    Shape, number_to_value,
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

fn ids(session: &EditSession) -> Vec<u32> {
    session.document().objects.iter().map(|o| o.id).collect()
}

fn reopened(session: &EditSession) -> EditSession {
    let json = session.document().to_json().expect("serializable");
    EditSession::restored(AnnotationDocument::from_json(&json).expect("readable"))
}

#[test]
fn a_new_object_is_visible_unlocked_and_unnamed() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    let object = &session.document().objects[0];

    assert_eq!(object.id, id);
    assert!(object.is_rendered() && object.is_pickable() && object.is_editable());
    assert_eq!(object.state.name, None);
    let json = session.document().to_json().expect("serializable");
    for field in ["hidden", "locked", "name"] {
        assert!(!json.contains(field), "{field} is only written when set");
    }
}

#[test]
fn the_editing_states_survive_a_save_and_a_reopening() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(rectangle(0.0));
    let second = session.add(rectangle(100.0));
    assert!(session.set_hidden(first, true));
    assert!(session.set_locked(second, true));
    assert!(session.rename(second, Some("  Titre  ".to_owned())));

    let again = reopened(&session);

    let states: Vec<_> = again
        .document()
        .objects
        .iter()
        .map(|object| object.state.clone())
        .collect();
    assert!(states[0].hidden && !states[0].locked && states[0].name.is_none());
    assert!(states[1].locked && !states[1].hidden);
    assert_eq!(states[1].name.as_deref(), Some("Titre"));
    assert_eq!(
        again.document().to_json().expect("serializable"),
        session.document().to_json().expect("serializable")
    );
}

#[test]
fn a_document_without_any_state_field_opens_as_before() {
    let mut written = EditSession::new(AnnotationDocument::default());
    written.add(rectangle(1.0));
    let text = written.document().to_json().expect("serializable");
    assert!(!text.contains("hidden"));
    let document = AnnotationDocument::from_json(&text);
    let session = EditSession::restored(document.expect("readable"));

    let object = &session.document().objects[0];
    assert_eq!(object.id, 1);
    assert!(object.is_pickable());
}

#[test]
fn the_states_and_unknown_fields_of_an_object_are_all_kept() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    session.set_locked(id, true);
    let mut value: serde_json::Value =
        serde_json::from_str(&session.document().to_json().expect("serializable")).expect("json");
    value["objects"][0]["from_the_future"] = json!({"kept": true});

    let text = value.to_string();
    let document = AnnotationDocument::from_json(&text).expect("readable");

    let back: serde_json::Value =
        serde_json::from_str(&document.to_json().expect("serializable")).expect("json");
    assert_eq!(back["objects"][0]["from_the_future"], json!({"kept": true}));
    assert_eq!(back["objects"][0]["locked"], json!(true));
}

#[test]
fn identifiers_are_never_reused_during_a_session() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(rectangle(0.0));
    let second = session.add(rectangle(100.0));
    assert!(session.undo());
    let third = session.add(rectangle(200.0));

    assert!(third > second && second > first);
    session.select(Some(third));
    assert!(session.remove_selected());
    let fourth = session.add(rectangle(300.0));
    assert!(fourth > third);
    let mut seen = ids(&session);
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), session.count());
}

#[test]
fn hiding_locking_and_renaming_are_undoable_exactly() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    let before = session.document().clone();

    assert!(session.set_hidden(id, true));
    assert!(session.set_locked(id, true));
    assert!(session.rename(id, Some("Cadre".to_owned())));
    assert!(!session.set_hidden(id, true), "no change leaves no step");
    let after = session.document().clone();

    assert!(session.undo() && session.undo() && session.undo());
    assert_eq!(session.document(), &before);
    assert!(session.redo() && session.redo() && session.redo());
    assert_eq!(session.document(), &after);
}

#[test]
fn a_locked_object_refuses_every_ordinary_change_and_leaves_no_trace() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let back = session.add(rectangle(0.0));
    let id = session.add(rectangle(100.0));
    session.add(rectangle(200.0));
    session.select(Some(id));
    assert!(session.set_locked(id, true));
    let before = session.document().clone();
    let undo_depth = session.can_undo();

    assert_eq!(session.refusal(id), Some(Refusal::Locked));
    assert!(!session.begin_change(id));
    assert!(!session.nudge_selected(5.0, 0.0));
    assert!(!session.remove_selected());
    assert!(!session.set_option_on_selected("width", number_to_value(9.0)));
    assert!(!session.can_move_selected(LayerMove::Front));
    assert!(!session.move_selected(LayerMove::Back));

    assert_eq!(session.document(), &before);
    assert_eq!(session.can_undo(), undo_depth);
    assert_eq!(session.refusal(back), None);
}

#[test]
fn a_locked_object_can_still_be_removed_on_purpose_and_comes_back_exactly() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    session.set_locked(id, true);
    session.set_hidden(id, true);
    let before = session.document().clone();

    assert!(session.remove_object(id));
    assert_eq!(session.count(), 0);
    assert!(session.undo());

    assert_eq!(session.document(), &before);
}

#[test]
fn a_copy_of_a_locked_hidden_object_is_a_plain_object() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    session.set_locked(id, true);
    session.rename(id, Some("Original".to_owned()));
    session.select(Some(id));

    let copy = session.duplicate_selected(10.0).expect("a copy");

    let state = &session
        .document()
        .objects
        .iter()
        .find(|object| object.id == copy)
        .expect("copy")
        .state;
    assert!(!state.locked && !state.hidden && state.name.is_none());
}

#[test]
fn picking_ignores_hidden_and_locked_objects_but_selection_by_id_does_not() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let hidden = session.add(rectangle(0.0));
    let locked = session.add(rectangle(0.0));
    session.set_hidden(hidden, true);
    session.set_locked(locked, true);
    session.select(None);
    let on_edge = Point { x: 0.0, y: 25.0 };

    assert_eq!(session.pick(on_edge, 6.0), None);
    assert!(session.objects_at(on_edge, 6.0).is_empty());
    session.select(Some(hidden));
    assert_eq!(session.selected(), Some(hidden));
}

#[test]
fn objects_at_lists_everything_under_the_pointer_from_the_top() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let bottom = session.add(rectangle(0.0));
    let middle = session.add(rectangle(0.0));
    let top = session.add(rectangle(0.0));
    session.select(None);

    assert_eq!(
        session.objects_at(Point { x: 0.0, y: 25.0 }, 6.0),
        vec![top, middle, bottom]
    );
}
