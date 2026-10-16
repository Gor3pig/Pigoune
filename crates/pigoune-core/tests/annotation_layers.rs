use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, LayerMove, Options, Point, Rect, STEP, Shape,
};

fn rectangle(x: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        Options::new(),
    )
}

fn step(x: f32) -> Shape {
    Shape::new(
        "step",
        Geometry::Anchor(Point { x, y: 0.0 }),
        Options::defaults(STEP.options),
    )
}

fn order(session: &EditSession) -> Vec<u32> {
    session
        .document()
        .objects
        .iter()
        .map(|object| object.id)
        .collect()
}

fn three() -> (EditSession, [u32; 3]) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(rectangle(1.0));
    let second = session.add(rectangle(2.0));
    let third = session.add(rectangle(3.0));
    (session, [first, second, third])
}

#[test]
fn an_object_goes_up_and_down_one_layer_at_a_time() {
    let (mut session, [first, second, third]) = three();
    session.select(Some(first));

    assert!(session.move_selected(LayerMove::Forward));
    assert_eq!(order(&session), [second, first, third]);
    assert!(session.move_selected(LayerMove::Backward));
    assert_eq!(order(&session), [first, second, third]);
}

#[test]
fn an_object_goes_to_the_front_or_to_the_back() {
    let (mut session, [first, second, third]) = three();
    session.select(Some(second));

    assert!(session.move_selected(LayerMove::Front));
    assert_eq!(order(&session), [first, third, second]);
    assert!(session.move_selected(LayerMove::Back));
    assert_eq!(order(&session), [second, first, third]);
}

#[test]
fn a_move_that_changes_nothing_is_refused_and_leaves_no_trace() {
    let (mut session, [first, _, third]) = three();
    session.select(Some(third));
    assert!(!session.can_move_selected(LayerMove::Front));
    assert!(!session.can_move_selected(LayerMove::Forward));
    assert!(session.can_move_selected(LayerMove::Back));
    let undoable_before = session.can_undo();
    let modified_before = session.is_modified();

    assert!(!session.move_selected(LayerMove::Front));

    assert_eq!(session.can_undo(), undoable_before);
    assert_eq!(session.is_modified(), modified_before);
    session.select(Some(first));
    assert!(!session.can_move_selected(LayerMove::Back));
    assert!(!session.can_move_selected(LayerMove::Backward));
}

#[test]
fn nothing_moves_without_a_selection() {
    let (mut session, _) = three();
    session.select(None);

    assert!(!session.can_move_selected(LayerMove::Front));
    assert!(!session.move_selected(LayerMove::Front));
}

#[test]
fn a_move_is_one_undoable_step() {
    let (mut session, [first, second, third]) = three();
    session.select(Some(first));
    session.move_selected(LayerMove::Front);
    assert_eq!(order(&session), [second, third, first]);

    assert!(session.undo());
    assert_eq!(order(&session), [first, second, third]);
    assert_eq!(session.selected(), Some(first));
    assert!(session.redo());
    assert_eq!(order(&session), [second, third, first]);
}

#[test]
fn moving_a_step_renumbers_the_steps() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let one = session.add(step(1.0));
    let two = session.add(step(2.0));
    assert_eq!(session.document().ordinal(one), Some(1));
    assert_eq!(session.document().ordinal(two), Some(2));

    session.select(Some(two));
    session.move_selected(LayerMove::Back);

    assert_eq!(session.document().ordinal(two), Some(1));
    assert_eq!(session.document().ordinal(one), Some(2));
}

#[test]
fn a_moved_layer_marks_the_session_as_modified() {
    let (mut session, [first, ..]) = three();
    session.mark_saved();
    session.select(Some(first));

    session.move_selected(LayerMove::Front);

    assert!(session.is_modified());
}
