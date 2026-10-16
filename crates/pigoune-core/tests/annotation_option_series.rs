use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, Rect, Shape,
    Tool, number_to_value, shape_bounds,
};

const REACH: f32 = 6.0;

fn rectangle(x: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
    .with_option("width", number_to_value(2.0))
}

fn width_of(session: &EditSession, id: u32) -> f32 {
    session.shape_of(id).expect("shape").number("width")
}

fn set_width(session: &mut EditSession, value: f32) -> bool {
    session.set_option_on_selected("width", number_to_value(value))
}

fn undo_all(session: &mut EditSession) -> usize {
    let mut steps = 0;
    while session.undo() {
        steps += 1;
    }
    steps
}

fn session_with_one() -> (EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(rectangle(0.0));
    (session, id)
}

#[test]
fn one_continuous_interaction_on_a_slider_is_a_single_undo_step() {
    let (mut session, id) = session_with_one();
    let original = width_of(&session, id);

    session.begin_option_series();
    for value in [6.0, 7.0, 9.0, 12.0, 15.0] {
        assert!(set_width(&mut session, value));
    }
    session.end_option_series();

    assert!((width_of(&session, id) - 15.0).abs() < 1e-3);
    assert!(session.undo());
    assert!((width_of(&session, id) - original).abs() < 1e-3);
    assert!(session.redo());
    assert!((width_of(&session, id) - 15.0).abs() < 1e-3);
    assert_eq!(undo_all(&mut session), 2);
}

#[test]
fn two_separate_interactions_are_two_steps_even_when_done_quickly() {
    let (mut session, id) = session_with_one();

    session.begin_option_series();
    set_width(&mut session, 6.0);
    set_width(&mut session, 8.0);
    session.end_option_series();
    session.begin_option_series();
    set_width(&mut session, 10.0);
    set_width(&mut session, 12.0);
    session.end_option_series();

    assert!(session.undo());
    assert!((width_of(&session, id) - 8.0).abs() < 1e-3);
    assert!(session.undo());
    assert!((width_of(&session, id) - 2.0).abs() < 1e-3);
}

#[test]
fn changes_outside_any_interaction_are_never_merged() {
    let (mut session, id) = session_with_one();

    set_width(&mut session, 6.0);
    set_width(&mut session, 8.0);
    set_width(&mut session, 10.0);

    assert_eq!(undo_all(&mut session), 4);
    assert_eq!(session.count(), 0);
    assert!(id > 0);
}

#[test]
fn a_different_property_ends_the_run_of_the_first_one() {
    let (mut session, _) = session_with_one();

    session.begin_option_series();
    set_width(&mut session, 6.0);
    set_width(&mut session, 7.0);
    session.set_option_on_selected("opacity", number_to_value(0.5));
    set_width(&mut session, 9.0);
    session.end_option_series();

    assert_eq!(undo_all(&mut session), 4);
}

#[test]
fn a_change_of_selection_ends_the_series() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let first = session.add(rectangle(0.0));
    let second = session.add(rectangle(200.0));

    session.begin_option_series();
    session.select(Some(first));
    set_width(&mut session, 6.0);
    session.select(Some(second));
    set_width(&mut session, 6.0);
    session.end_option_series();

    assert!((width_of(&session, first) - 6.0).abs() < 1e-3);
    assert!((width_of(&session, second) - 6.0).abs() < 1e-3);
    assert!(session.undo());
    assert!(width_of(&session, second) < 6.0);
    assert!((width_of(&session, first) - 6.0).abs() < 1e-3);
}

#[test]
fn another_edit_in_between_ends_the_series() {
    let (mut session, id) = session_with_one();

    session.begin_option_series();
    set_width(&mut session, 6.0);
    assert!(session.nudge_selected(5.0, 0.0));
    set_width(&mut session, 8.0);
    session.end_option_series();

    assert_eq!(undo_all(&mut session), 4);
    assert!(id > 0);
}

#[test]
fn a_series_whose_end_never_arrives_blocks_nothing_and_never_swallows_a_new_interaction() {
    let (mut session, id) = session_with_one();

    session.begin_option_series();
    set_width(&mut session, 6.0);
    set_width(&mut session, 7.0);
    session.begin_option_series();
    set_width(&mut session, 9.0);

    assert!(session.can_undo());
    assert!(session.nudge_selected(3.0, 0.0));
    assert!(session.undo());
    assert!(session.undo());
    assert!((width_of(&session, id) - 7.0).abs() < 1e-3);
    assert!(session.undo());
    assert!(session.can_redo());
}

#[test]
fn going_back_to_the_starting_value_leaves_no_trace() {
    let (mut session, id) = session_with_one();
    session.mark_saved();
    let original = width_of(&session, id);

    session.begin_option_series();
    set_width(&mut session, 12.0);
    set_width(&mut session, original);
    session.end_option_series();

    assert!(!session.is_modified());
    assert_eq!(undo_all(&mut session), 1);
}

#[test]
fn changing_a_property_after_a_save_marks_the_document_modified_again() {
    let (mut session, _) = session_with_one();
    session.begin_option_series();
    set_width(&mut session, 6.0);
    session.end_option_series();
    session.mark_saved();

    session.begin_option_series();
    set_width(&mut session, 8.0);
    set_width(&mut session, 9.0);
    session.end_option_series();

    assert!(session.is_modified());
    assert!(session.undo());
    assert!(!session.is_modified());
}

#[test]
fn a_property_change_never_leaves_the_object_unmovable() {
    let (mut session, id) = session_with_one();
    session.begin_option_series();
    for value in [6.0, 9.0, 14.0] {
        set_width(&mut session, value);
    }
    let mut interaction = Interaction::new();
    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        Point { x: 0.0, y: 40.0 },
        REACH,
    );
    interaction.drag(&mut session, Point { x: 10.0, y: 40.0 });
    interaction.release(&mut session, Point { x: 20.0, y: 40.0 });

    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    assert!((bounds.x - 20.0).abs() < 1e-3, "{bounds:?}");
}

#[test]
fn an_orphaned_move_is_settled_into_one_step_instead_of_blocking_or_vanishing() {
    let (mut session, id) = session_with_one();
    let before = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");

    assert!(session.begin_change(id));
    let moved =
        pigoune_core::annotations::translated(session.shape_of(id).expect("shape"), 30.0, 0.0);
    session.preview(moved);
    assert!(!session.can_undo(), "the move is still open");

    let mut interaction = Interaction::new();
    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        Point { x: 900.0, y: 900.0 },
        REACH,
    );

    let after = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    assert!(
        (after.x - (before.x + 30.0)).abs() < 1e-3,
        "the visible move is kept"
    );
    assert!(session.can_undo());
    assert!(session.undo());
    let restored = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    assert!((restored.x - before.x).abs() < 1e-3);
}

#[test]
fn an_orphaned_move_that_changed_nothing_leaves_no_step() {
    let (mut session, id) = session_with_one();
    session.mark_saved();

    assert!(session.begin_change(id));
    session.settle_open_gesture();

    assert!(!session.is_modified());
    assert!(session.begin_change(id));
}
