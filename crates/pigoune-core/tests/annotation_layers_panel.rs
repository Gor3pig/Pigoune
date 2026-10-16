use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, LayerMove, Options, Point, RECTANGLE, Rect, Shape,
    TEXT, layer_entries,
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

fn text(content: &str) -> Shape {
    Shape::new(
        "text",
        Geometry::Anchor(Point { x: 5.0, y: 5.0 }),
        Options::defaults(TEXT.options),
    )
    .with_option("content", json!(content))
}

fn model_order(session: &EditSession) -> Vec<u32> {
    session.document().objects.iter().map(|o| o.id).collect()
}

fn panel_order(session: &EditSession) -> Vec<u32> {
    layer_entries(session.document())
        .iter()
        .map(|entry| entry.id)
        .collect()
}

fn session_with(count: u8) -> (EditSession, Vec<u32>) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let ids = (0..count)
        .map(|index| session.add(rectangle(f32::from(index) * 60.0)))
        .collect();
    (session, ids)
}

struct Dice(u64);

impl Dice {
    fn below(&mut self, limit: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).expect("fits") % limit.max(1)
    }
}

#[test]
fn the_panel_lists_the_front_first_and_is_the_exact_reverse_of_the_document() {
    let (session, ids) = session_with(5);

    let mut expected = ids.clone();
    expected.reverse();
    assert_eq!(panel_order(&session), expected);
    let mut reversed = panel_order(&session);
    reversed.reverse();
    assert_eq!(reversed, model_order(&session));
}

#[test]
fn every_annotation_appears_exactly_once_with_its_rank_among_its_own_kind() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(rectangle(0.0));
    let note = session.add(text("Hello"));
    let second = session.add(rectangle(60.0));

    let entries = layer_entries(session.document());

    assert_eq!(entries.len(), 3);
    let by_id = |id: u32| entries.iter().find(|entry| entry.id == id).expect("entry");
    assert_eq!(
        (by_id(first).kind.as_str(), by_id(first).ordinal),
        ("rectangle", 1)
    );
    assert_eq!(
        (by_id(second).kind.as_str(), by_id(second).ordinal),
        ("rectangle", 2)
    );
    assert_eq!(
        (by_id(note).kind.as_str(), by_id(note).ordinal),
        ("text", 1)
    );
    assert_eq!(by_id(note).summary, "Hello");
    assert_eq!(by_id(first).summary, "");
}

#[test]
fn a_long_text_is_summarized_on_its_first_line_only() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(text(
        "Une phrase vraiment très longue qui dépasse\nseconde ligne",
    ));

    let entries = layer_entries(session.document());

    assert!(entries[0].summary.ends_with('…'));
    assert!(!entries[0].summary.contains('\n'));
    assert!(entries[0].summary.chars().count() <= 25);
}

#[test]
fn a_custom_name_replaces_nothing_but_is_carried_next_to_the_automatic_data() {
    let (mut session, ids) = session_with(2);
    assert!(session.rename(ids[0], Some("  Cadre  ".to_owned())));

    let entries = layer_entries(session.document());
    let named = entries
        .iter()
        .find(|entry| entry.id == ids[0])
        .expect("entry");

    assert_eq!(named.name.as_deref(), Some("Cadre"));
    assert_eq!(named.ordinal, 1);
    assert!(session.rename(ids[0], Some("   ".to_owned())));
    assert_eq!(layer_entries(session.document())[1].name, None);
}

#[test]
fn an_object_moves_by_identifier_without_changing_the_selection() {
    let (mut session, ids) = session_with(4);
    session.select(Some(ids[3]));

    assert!(session.move_object(ids[0], LayerMove::Front));

    assert_eq!(session.selected(), Some(ids[3]));
    assert_eq!(model_order(&session), vec![ids[1], ids[2], ids[3], ids[0]]);
    assert!(session.undo());
    assert_eq!(model_order(&session), ids);
}

#[test]
fn reordering_next_to_another_layer_lands_exactly_above_or_below_it() {
    let (mut session, ids) = session_with(5);
    let (one, two, three, four, five) = (ids[0], ids[1], ids[2], ids[3], ids[4]);

    assert!(session.reorder_next_to(one, four, true));
    assert_eq!(model_order(&session), vec![two, three, four, one, five]);
    assert!(session.reorder_next_to(five, two, false));
    assert_eq!(model_order(&session), vec![five, two, three, four, one]);
    assert!(session.reorder_next_to(five, one, true));
    assert_eq!(model_order(&session), vec![two, three, four, one, five]);
    assert!(session.reorder_next_to(two, one, false));
    assert_eq!(model_order(&session), vec![three, four, two, one, five]);
}

#[test]
fn a_drop_that_changes_nothing_leaves_no_history_step() {
    let (mut session, ids) = session_with(4);
    session.mark_saved();

    assert!(!session.reorder_next_to(ids[1], ids[1], true));
    assert!(!session.reorder_next_to(ids[1], ids[0], true));
    assert!(!session.reorder_next_to(ids[1], ids[2], false));
    assert!(!session.reorder_next_to(ids[3], ids[2], true));
    assert!(!session.reorder_next_to(ids[0], ids[1], false));
    assert!(!session.reorder_next_to(ids[0], 999, true));
    assert!(!session.reorder_next_to(999, ids[0], true));

    assert_eq!(model_order(&session), ids);
    assert!(!session.is_modified());
}

#[test]
fn the_first_and_the_last_rows_can_be_dropped_in_both_directions() {
    let (mut session, ids) = session_with(3);
    let top = ids[2];
    let bottom = ids[0];

    assert!(session.reorder_next_to(top, bottom, false));
    assert_eq!(model_order(&session)[0], top);
    assert!(session.undo());
    assert!(session.reorder_next_to(bottom, top, true));
    assert_eq!(*model_order(&session).last().expect("last"), bottom);
}

#[test]
fn reordering_is_one_exact_undoable_step_and_the_panel_follows() {
    let (mut session, ids) = session_with(4);
    let before = session.document().clone();

    assert!(session.reorder_next_to(ids[0], ids[3], true));
    let after = session.document().clone();
    let after_panel = panel_order(&session);

    assert!(session.undo());
    assert_eq!(session.document(), &before);
    assert_eq!(panel_order(&session).first(), Some(&ids[3]));
    assert!(session.redo());
    assert_eq!(session.document(), &after);
    assert_eq!(panel_order(&session), after_panel);
}

#[test]
fn random_operations_keep_the_panel_the_exact_reverse_of_the_document() {
    for seed in 0..60 {
        let mut dice = Dice(seed + 1);
        let mut session = EditSession::new(AnnotationDocument::default());
        let mut reference: Vec<u32> = Vec::new();
        for step in 0..60 {
            let known = model_order(&session);
            match dice.below(6) {
                0 | 1 => {
                    let id = session.add(rectangle(f32::from(u8::try_from(step).unwrap_or(0))));
                    reference.push(id);
                }
                2 if !known.is_empty() => {
                    let id = known[dice.below(known.len())];
                    let target = known[dice.below(known.len())];
                    let above = dice.below(2) == 0;
                    let mut simulated = reference.clone();
                    if id != target {
                        let from = simulated.iter().position(|x| *x == id).expect("id");
                        simulated.remove(from);
                        let at = simulated.iter().position(|x| *x == target).expect("target");
                        simulated.insert(if above { at + 1 } else { at }, id);
                    }
                    let moved = session.reorder_next_to(id, target, above);
                    if moved {
                        reference = simulated;
                    }
                    assert_eq!(moved, simulated_differs(&reference, &known));
                }
                3 if !known.is_empty() => {
                    let id = known[dice.below(known.len())];
                    assert!(session.remove_object(id));
                    reference.retain(|other| *other != id);
                }
                4 if !known.is_empty() => {
                    let id = known[dice.below(known.len())];
                    let copy = session.duplicate_object(id, 6.0).expect("copy");
                    let at = reference.iter().position(|x| *x == id).expect("id");
                    reference.insert(at + 1, copy);
                }
                _ => {
                    if session.undo() {
                        assert!(session.redo());
                    }
                }
            }
            assert_eq!(model_order(&session), reference, "seed {seed} step {step}");
            let mut shown = panel_order(&session);
            shown.reverse();
            assert_eq!(shown, reference, "seed {seed} step {step}");
        }
        while session.undo() {}
        assert_eq!(session.count(), 0);
    }
}

fn simulated_differs(reference: &[u32], before: &[u32]) -> bool {
    reference != before
}

#[test]
fn duplicating_by_identifier_places_the_copy_right_above_and_selects_it() {
    let (mut session, ids) = session_with(3);
    session.select(Some(ids[2]));

    let copy = session.duplicate_object(ids[0], 12.0).expect("copy");

    assert_eq!(model_order(&session), vec![ids[0], copy, ids[1], ids[2]]);
    assert_eq!(session.selected(), Some(copy));
    assert!(session.undo());
    assert_eq!(model_order(&session), ids);
    assert!(session.redo());
    assert_eq!(model_order(&session), vec![ids[0], copy, ids[1], ids[2]]);
    assert_eq!(session.duplicate_object(999, 12.0), None);
}

#[test]
fn a_covered_annotation_is_selectable_by_identifier_while_picking_keeps_the_topmost_rule() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let covered = session.add(rectangle(0.0).with_option("fill", json!(true)));
    let cover = session.add(rectangle(0.0).with_option("fill", json!(true)));
    session.select(None);
    let inside = Point { x: 25.0, y: 25.0 };

    assert_eq!(session.pick(inside, 6.0), Some(cover));
    assert_eq!(session.objects_at(inside, 6.0), vec![cover, covered]);
    session.select(Some(covered));

    assert_eq!(session.selected(), Some(covered));
    assert_eq!(
        session.pick(inside, 6.0),
        Some(cover),
        "the visible topmost object keeps priority at the click"
    );
    assert_eq!(model_order(&session), vec![covered, cover]);
}

#[test]
fn names_and_order_survive_a_save_and_a_reopening() {
    let (mut session, ids) = session_with(3);
    session.rename(ids[0], Some("Fond".to_owned()));
    session.rename(ids[2], Some("Titre".to_owned()));
    session.reorder_next_to(ids[0], ids[2], true);
    let saved = session.document().to_json().expect("serializable");

    let again = EditSession::restored(AnnotationDocument::from_json(&saved).expect("readable"));

    assert_eq!(model_order(&again), model_order(&session));
    let names: Vec<Option<String>> = layer_entries(again.document())
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        names,
        vec![Some("Fond".to_owned()), Some("Titre".to_owned()), None]
    );
}

#[test]
fn a_rename_is_one_exact_undoable_step() {
    let (mut session, ids) = session_with(2);
    let before = session.document().clone();

    assert!(session.rename(ids[1], Some("Flèche".to_owned())));
    assert!(session.undo());
    assert_eq!(session.document(), &before);
    assert!(session.redo());
    assert_eq!(
        layer_entries(session.document())[0].name.as_deref(),
        Some("Flèche")
    );
}
