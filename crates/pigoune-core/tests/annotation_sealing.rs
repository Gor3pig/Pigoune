use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, LayerMove, Options, OverlapChoice, RECTANGLE, Rect,
    SealRecord, Shape, TEXT, number_to_value,
};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn record(generation: u32) -> SealRecord {
    SealRecord {
        generation,
        base: HASH.to_owned(),
    }
}

fn boxed(kind: &str, x: f32, y: f32, size: f32) -> Shape {
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y,
            width: size,
            height: size,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

fn text_at(x: f32, y: f32) -> Shape {
    Shape::new(
        "text",
        Geometry::Anchor(pigoune_core::annotations::Point { x, y }),
        Options::defaults(TEXT.options),
    )
}

fn ids(session: &EditSession) -> Vec<u32> {
    session.document().objects.iter().map(|o| o.id).collect()
}

fn sealed_ids(session: &EditSession) -> Vec<u32> {
    session
        .document()
        .objects
        .iter()
        .filter(|o| o.state.sealed)
        .map(|o| o.id)
        .collect()
}

fn sealed_session() -> (EditSession, u32, u32, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let below = session.add(boxed("rectangle", 0.0, 0.0, 60.0));
    let mask = session.add(boxed("blur", 20.0, 20.0, 80.0));
    let above = session.add(boxed("rectangle", 300.0, 300.0, 40.0));
    let plan = session.seal_plan();
    let document = session.sealed_document(&plan, OverlapChoice::KeepVisible, record(1));
    session.adopt_sealed(document);
    (session, below, mask, above)
}

#[test]
fn a_seal_record_survives_the_json_and_a_document_without_one_is_unchanged() {
    let plain = AnnotationDocument::default().to_json().expect("json");
    assert!(!plain.contains("seal"));
    let mut document = AnnotationDocument::default();
    document.seal = Some(record(3));
    let json = document.to_json().expect("json");
    let back = AnnotationDocument::from_json(&json).expect("readable");
    assert_eq!(back.seal, Some(record(3)));
    assert_eq!(back, document);
}

#[test]
fn a_malformed_seal_record_makes_the_document_unreadable() {
    let base = |seal: &str| format!(r#"{{"version":1,"objects":[],"seal":{seal}}}"#);
    for bad in [
        r#"{"generation":0,"base":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
        r#"{"generation":1,"base":"short"}"#,
        r#"{"generation":1,"base":"0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
        r#"{"generation":-1,"base":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
        r#"{"generation":1}"#,
        r#""sealed""#,
        "null",
        "7",
    ] {
        assert!(
            AnnotationDocument::from_json(&base(bad)).is_err(),
            "{bad} must not be accepted"
        );
    }
}

#[test]
fn the_plan_lists_the_visible_masks_and_what_they_cover() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let under = session.add(boxed("rectangle", 0.0, 0.0, 60.0));
    let words = session.add(text_at(10.0, 10.0));
    let far = session.add(boxed("rectangle", 500.0, 500.0, 10.0));
    let mask = session.add(boxed("blur", 20.0, 20.0, 80.0));
    let above = session.add(boxed("rectangle", 30.0, 30.0, 10.0));
    let hidden_mask = session.add(boxed("pixelate", 0.0, 0.0, 400.0));
    session.set_hidden(hidden_mask, true);

    let plan = session.seal_plan();

    assert_eq!(plan.masks, vec![mask]);
    assert!(plan.overlapped.contains(&under));
    assert!(plan.overlapped.contains(&words));
    assert!(!plan.overlapped.contains(&far));
    assert!(!plan.overlapped.contains(&above));
    assert_eq!(plan.texts, vec![words]);
}

#[test]
fn a_plan_without_a_visible_mask_is_empty() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 0.0, 0.0, 60.0));
    let hidden = session.add(boxed("blur", 0.0, 0.0, 60.0));
    session.set_hidden(hidden, true);
    assert!(session.seal_plan().is_empty());
}

#[test]
fn sealing_puts_the_masks_first_and_keeps_the_others_in_order() {
    let (session, below, mask, above) = sealed_session();
    assert_eq!(ids(&session), vec![mask, below, above]);
    assert_eq!(sealed_ids(&session), vec![mask]);
    assert_eq!(session.document().seal, Some(record(1)));
    assert_eq!(session.sealed_floor(), 1);
}

#[test]
fn the_choice_to_keep_objects_visible_leaves_them_untouched() {
    let (session, below, ..) = sealed_session();
    let object = session
        .document()
        .objects
        .iter()
        .find(|o| o.id == below)
        .expect("kept");
    assert!(!object.state.hidden);
    assert!(object.is_editable());
}

#[test]
fn the_choice_to_hide_covered_objects_hides_them_but_keeps_them() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let under = session.add(boxed("rectangle", 0.0, 0.0, 60.0));
    session.add(boxed("blur", 20.0, 20.0, 80.0));
    let plan = session.seal_plan();
    let document = session.sealed_document(&plan, OverlapChoice::HideOverlapped, record(1));
    session.adopt_sealed(document);
    let object = session
        .document()
        .objects
        .iter()
        .find(|o| o.id == under)
        .expect("kept");
    assert!(object.state.hidden);
    assert!(!object.state.sealed);
}

#[test]
fn a_sealed_marker_is_no_longer_a_pending_mask() {
    let (session, ..) = sealed_session();
    assert!(!session.document().hides_content());
    assert!(session.document().masks().is_empty());
    assert!(session.document().exposed_masks().is_empty());
}

#[test]
fn a_hidden_privacy_mask_is_reported_as_exposed_until_it_is_sealed() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let blur = session.add(boxed("blur", 0.0, 0.0, 50.0));
    let pixels = session.add(boxed("pixelate", 0.0, 0.0, 50.0));
    session.add(boxed("rectangle", 0.0, 0.0, 50.0));
    assert!(session.document().exposed_masks().is_empty());
    session.set_hidden(blur, true);
    assert_eq!(session.document().exposed_masks(), vec![blur]);
    session.set_hidden(pixels, true);
    assert_eq!(session.document().exposed_masks(), vec![blur, pixels]);
    session.set_hidden(blur, false);
    assert_eq!(session.document().exposed_masks(), vec![pixels]);
    session.undo();
    assert_eq!(session.document().exposed_masks(), vec![blur, pixels]);
}

#[test]
fn sealing_clears_both_histories_and_the_selection() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 0.0, 0.0, 10.0));
    let mask = session.add(boxed("blur", 0.0, 0.0, 10.0));
    session.add(boxed("rectangle", 0.0, 0.0, 10.0));
    session.undo();
    assert!(session.can_undo() && session.can_redo());
    session.select(Some(mask));
    let plan = session.seal_plan();
    let document = session.sealed_document(&plan, OverlapChoice::KeepVisible, record(1));
    session.adopt_sealed(document);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(session.selected(), None);
    assert!(!session.is_modified());
}

#[test]
fn preparing_a_sealed_document_changes_nothing_in_the_session() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 0.0, 0.0, 10.0));
    session.add(boxed("blur", 0.0, 0.0, 10.0));
    let before = session.document().clone();
    let undo_before = session.can_undo();
    let plan = session.seal_plan();
    let _ = session.sealed_document(&plan, OverlapChoice::HideOverlapped, record(1));
    assert_eq!(session.document(), &before);
    assert_eq!(session.can_undo(), undo_before);
    assert!(session.is_modified());
}

#[test]
fn a_sealed_marker_refuses_every_change_and_leaves_no_trace() {
    let (mut session, _, mask, _) = sealed_session();
    session.select(Some(mask));
    let snapshot = session.document().clone();
    assert!(!session.nudge_selected(5.0, 0.0));
    assert!(!session.set_hidden(mask, true));
    assert!(!session.set_locked(mask, true));
    assert!(!session.rename(mask, Some("x".to_owned())));
    assert!(!session.move_object(mask, LayerMove::Front));
    assert!(!session.remove_object(mask));
    assert!(!session.remove_selected());
    assert!(!session.begin_change(mask));
    assert!(!session.set_option_on_selected("radius", number_to_value(5.0)));
    assert!(session.duplicate_object(mask, 4.0).is_none());
    assert!(!session.reorder_next_to(mask, 1, true));
    assert_eq!(session.document(), &snapshot);
    assert!(!session.can_undo());
}

#[test]
fn a_sealed_marker_cannot_be_grabbed_on_the_canvas() {
    let (session, _, _, _) = sealed_session();
    let point = pigoune_core::annotations::Point { x: 60.0, y: 60.0 };
    let found = session.objects_at(point, 3.0);
    assert!(
        found
            .iter()
            .all(|id| !session.document().is_sealed_marker(*id)),
        "{found:?}"
    );
}

#[test]
fn nothing_goes_under_the_sealed_stratum() {
    let (mut session, below, mask, above) = sealed_session();
    assert!(!session.can_move_object(below, LayerMove::Back));
    assert!(!session.can_move_object(below, LayerMove::Backward));
    assert!(session.can_move_object(below, LayerMove::Front));
    assert!(session.move_object(above, LayerMove::Back));
    assert_eq!(ids(&session), vec![mask, above, below]);
    assert!(!session.reorder_next_to(below, mask, false));
    assert!(session.reorder_next_to(below, mask, true));
    assert_eq!(ids(&session), vec![mask, below, above]);
}

#[test]
fn removing_the_marker_is_undoable_and_keeps_the_seal_record() {
    let (mut session, _, mask, _) = sealed_session();
    let record_before = session.document().seal.clone();
    assert!(session.remove_marker(mask));
    assert!(!ids(&session).contains(&mask));
    assert_eq!(session.document().seal, record_before);
    assert!(session.undo());
    assert!(ids(&session).contains(&mask));
    assert!(session.document().is_sealed_marker(mask));
    assert_eq!(session.document().seal, record_before);
    assert!(session.redo());
    assert_eq!(session.document().seal, record_before);
}

#[test]
fn removing_every_marker_leaves_the_seal_record_in_the_saved_document() {
    let (mut session, _, mask, _) = sealed_session();
    session.remove_marker(mask);
    let json = session.document().to_json().expect("json");
    let back = AnnotationDocument::from_json(&json).expect("readable");
    assert_eq!(back.seal, Some(record(1)));
    assert_eq!(back.sealed_count(), 0);
}

#[test]
fn only_a_sealed_marker_can_be_removed_as_a_marker() {
    let (mut session, below, ..) = sealed_session();
    assert!(!session.remove_marker(below));
    assert!(!session.remove_marker(999));
}

#[test]
fn two_sealings_stack_their_markers_and_keep_the_older_ones_first() {
    let (mut session, below, first, above) = sealed_session();
    let second = session.add(boxed("pixelate", 300.0, 300.0, 40.0));
    let plan = session.seal_plan();
    assert_eq!(plan.masks, vec![second]);
    let document = session.sealed_document(&plan, OverlapChoice::KeepVisible, record(2));
    session.adopt_sealed(document);
    assert_eq!(ids(&session), vec![first, second, below, above]);
    assert_eq!(session.sealed_floor(), 2);
    assert_eq!(session.document().seal, Some(record(2)));
}

#[test]
fn the_sealed_stratum_stays_in_front_of_the_list_through_random_operations() {
    let (mut session, ..) = sealed_session();
    let floor = session.sealed_floor();
    let mut seed = 0x2545_F491_u64;
    let mut next = |bound: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) % bound
    };
    for _ in 0..600 {
        let all: Vec<u32> = ids(&session);
        let pick = all[usize::try_from(next(all.len() as u64)).expect("small")];
        match next(9) {
            0 => {
                session.add(boxed("rectangle", 1.0, 1.0, 9.0));
            }
            1 => {
                session.move_object(pick, LayerMove::Back);
            }
            2 => {
                session.move_object(pick, LayerMove::Backward);
            }
            3 => {
                session.move_object(pick, LayerMove::Front);
            }
            4 => {
                let target = all[usize::try_from(next(all.len() as u64)).expect("small")];
                session.reorder_next_to(pick, target, next(2) == 0);
            }
            5 => {
                session.remove_object(pick);
            }
            6 => {
                session.undo();
            }
            7 => {
                session.redo();
            }
            _ => {
                session.duplicate_object(pick, 3.0);
            }
        }
        assert_eq!(session.sealed_floor(), floor);
        assert_eq!(sealed_ids(&session).len(), floor);
        assert!(
            session.document().objects[..floor]
                .iter()
                .all(|o| o.state.sealed)
        );
    }
}
