use pigoune_core::annotations::{
    AnnotationDocument, Crop, CursorKind, EditSession, Geometry, Interaction, Options, Point, Rect,
    Shape, Tool, definition, number_to_value, shape_bounds,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn boxed(kind: &str, x: f32, y: f32, width: f32, height: f32) -> Shape {
    let sheet = definition(kind).expect("tool");
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        Options::defaults(sheet.options),
    )
}

fn drag(session: &mut EditSession, tool: Tool, from: Point, to: Point) {
    let mut interaction = Interaction::new();
    let options = match tool {
        Tool::Draw(sheet) => Options::defaults(sheet.options),
        _ => Options::new(),
    };
    interaction.press(session, tool, &options, from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

fn drawing(kind: &str) -> Tool {
    Tool::Draw(definition(kind).expect("tool"))
}

fn origin(session: &EditSession, id: u32) -> (f32, f32) {
    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    (bounds.x, bounds.y)
}

fn hollow(x: f32, y: f32, width: f32, height: f32) -> Shape {
    boxed("rectangle", x, y, width, height)
}

fn filled(x: f32, y: f32, width: f32, height: f32) -> Shape {
    hollow(x, y, width, height).with_option("fill", json!(true))
}

fn move_by(session: &mut EditSession, from: Point, dx: f32, dy: f32) {
    drag(session, Tool::Select, from, point(from.x + dx, from.y + dy));
}

#[test]
fn a_drawing_tool_always_creates_even_inside_or_on_the_edge_of_the_selected_shape() {
    for start in [
        point(300.0, 250.0),
        point(100.0, 170.0),
        point(100.0, 100.0),
    ] {
        let mut session = EditSession::new(AnnotationDocument::default());
        let existing = session.add(hollow(100.0, 100.0, 400.0, 300.0));
        let before = origin(&session, existing);

        drag(
            &mut session,
            drawing("rectangle"),
            start,
            point(start.x + 40.0, start.y + 30.0),
        );

        assert_eq!(session.count(), 2, "{start:?}");
        assert_eq!(origin(&session, existing), before, "{start:?}");
        assert_ne!(session.selected(), Some(existing));
    }
}

#[test]
fn a_blur_or_a_filled_shape_can_be_drawn_over_a_selected_one_of_the_same_kind() {
    for kind in ["blur", "pixelate", "spotlight"] {
        let mut session = EditSession::new(AnnotationDocument::default());
        let outer = session.add(boxed(kind, 100.0, 100.0, 400.0, 300.0));
        let before = origin(&session, outer);

        drag(
            &mut session,
            drawing(kind),
            point(150.0, 150.0),
            point(250.0, 220.0),
        );

        assert_eq!(session.count(), 2, "{kind}");
        assert_eq!(origin(&session, outer), before, "{kind}");
    }
}

#[test]
fn a_hollow_shape_is_grabbed_by_its_outline_only_while_it_is_not_selected() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    session.select(None);

    assert_eq!(session.pick(point(300.0, 250.0), REACH), None);
    assert_eq!(session.pick(point(100.0, 250.0), REACH), Some(id));
    assert_eq!(session.pick(point(104.0, 250.0), REACH), Some(id));
    assert_eq!(session.pick(point(300.0, 400.0), REACH), Some(id));

    move_by(&mut session, point(300.0, 250.0), 30.0, 0.0);
    assert_eq!(origin(&session, id), (100.0, 100.0));
    assert_eq!(session.selected(), None);
}

#[test]
fn a_selected_hollow_shape_is_grabbed_by_its_inside_and_moves() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    session.select(None);

    session.select(Some(id));
    assert_eq!(session.pick(point(300.0, 250.0), REACH), Some(id));
    move_by(&mut session, point(300.0, 250.0), 25.0, 15.0);

    assert_eq!(origin(&session, id), (125.0, 115.0));
    assert_eq!(session.count(), 1);
}

#[test]
fn an_ellipse_follows_the_same_rule_as_a_rectangle() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("ellipse", 100.0, 100.0, 400.0, 300.0));
    session.select(None);

    assert_eq!(session.pick(point(300.0, 250.0), REACH), None);
    assert_eq!(session.pick(point(100.0, 250.0), REACH), Some(id));

    session.select(Some(id));
    move_by(&mut session, point(300.0, 250.0), 10.0, 10.0);
    assert_eq!(origin(&session, id), (110.0, 110.0));
}

#[test]
fn what_is_visible_on_top_is_grabbed_first_even_over_a_selected_hollow_shape() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let lower = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    let upper = session.add(filled(200.0, 150.0, 100.0, 100.0));
    session.select(Some(lower));

    assert_eq!(session.pick(point(250.0, 200.0), REACH), Some(upper));
    assert_eq!(session.pick(point(400.0, 300.0), REACH), Some(lower));

    let outer = session.add(hollow(90.0, 90.0, 420.0, 320.0));
    session.select(Some(lower));
    assert_eq!(session.pick(point(95.0, 250.0), REACH), Some(outer));
}

#[test]
fn a_thin_line_under_a_filled_shape_never_wins_over_it() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let line = session.add(
        boxed("line", 0.0, 0.0, 0.0, 0.0).with_geometry(Geometry::TwoPoints {
            from: point(100.0, 200.0),
            to: point(500.0, 200.0),
        }),
    );
    let panel = session.add(filled(200.0, 150.0, 100.0, 100.0));
    session.select(None);

    assert_eq!(session.pick(point(250.0, 200.0), REACH), Some(panel));
    assert_eq!(session.pick(point(150.0, 200.0), REACH), Some(line));
}

#[test]
fn a_text_and_a_badge_are_grabbed_by_their_box() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let text_sheet = definition("text").expect("text");
    let text = session.add(
        Shape::new(
            "text",
            Geometry::Anchor(point(100.0, 100.0)),
            Options::defaults(text_sheet.options),
        )
        .with_option("content", json!("Hello")),
    );
    let step_sheet = definition("step").expect("step");
    let step = session.add(Shape::new(
        "step",
        Geometry::Anchor(point(400.0, 100.0)),
        Options::defaults(step_sheet.options),
    ));
    session.select(None);

    assert_eq!(session.pick(point(110.0, 110.0), REACH), Some(text));
    assert_eq!(session.pick(point(402.0, 102.0), REACH), Some(step));
    move_by(&mut session, point(110.0, 110.0), 20.0, 0.0);
    session.select(None);
    move_by(&mut session, point(402.0, 102.0), 8.0, 8.0);

    let Geometry::Anchor(step_at) = session.shape_of(step).expect("step").geometry else {
        panic!("anchored")
    };
    assert!(near(step_at.x, 408.0) && near(step_at.y, 108.0));
    let Geometry::Anchor(moved_text) = session.shape_of(text).expect("text").geometry else {
        panic!("anchored")
    };
    assert!(near(moved_text.x, 120.0) && near(moved_text.y, 100.0));
    assert_eq!(session.count(), 2);
}

#[test]
fn the_cursor_tells_what_a_press_would_do_for_every_tool() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    let interaction = Interaction::new();
    let cursor =
        |session: &EditSession, tool, x, y| interaction.cursor(session, tool, point(x, y), REACH);

    session.select(None);
    assert_eq!(
        cursor(&session, Tool::Select, 300.0, 250.0),
        CursorKind::Default
    );
    assert_eq!(
        cursor(&session, Tool::Select, 100.0, 170.0),
        CursorKind::Move
    );
    session.select(Some(id));
    assert_eq!(
        cursor(&session, Tool::Select, 300.0, 250.0),
        CursorKind::Move
    );
    for (x, y) in [(300.0, 250.0), (100.0, 170.0), (100.0, 100.0)] {
        assert_eq!(
            cursor(&session, drawing("rectangle"), x, y),
            CursorKind::Crosshair,
            "({x}, {y})"
        );
    }
}

#[test]
fn control_turns_a_drawing_tool_into_the_selection_tool_while_it_is_held() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    let mut interaction = Interaction::new();
    interaction.set_temporary_select(true);

    assert_eq!(
        interaction.cursor(&session, drawing("rectangle"), point(500.0, 400.0), REACH),
        CursorKind::Resize(pigoune_core::annotations::Handle::BottomRight)
    );
    interaction.press(
        &mut session,
        drawing("rectangle"),
        &Options::new(),
        point(500.0, 400.0),
        REACH,
    );
    interaction.drag(&mut session, point(520.0, 420.0));
    interaction.release(&mut session, point(540.0, 440.0));

    assert_eq!(session.count(), 1);
    assert_eq!(session.selected(), Some(id));
    interaction.set_temporary_select(false);
    interaction.press(
        &mut session,
        drawing("rectangle"),
        &Options::new(),
        point(540.0, 440.0),
        REACH,
    );
    interaction.cancel(&mut session);
    assert_eq!(session.count(), 1);
}

#[test]
fn no_handle_is_reachable_under_a_drawing_tool() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(hollow(100.0, 100.0, 400.0, 300.0));
    let before = origin(&session, id);

    drag(
        &mut session,
        drawing("rectangle"),
        point(500.0, 400.0),
        point(560.0, 450.0),
    );

    assert_eq!(session.count(), 2);
    assert_eq!(origin(&session, id), before);
    let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
    assert!(near(bounds.width, 400.0) && near(bounds.height, 300.0));
}

#[test]
fn a_locked_or_hidden_object_is_neither_grabbed_nor_changed() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let locked = session.add(filled(100.0, 100.0, 100.0, 100.0));
    let hidden = session.add(filled(300.0, 100.0, 100.0, 100.0));
    assert!(session.set_locked(locked, true));
    assert!(session.set_hidden(hidden, true));
    session.select(None);

    assert_eq!(session.pick(point(150.0, 150.0), REACH), None);
    assert_eq!(session.pick(point(350.0, 150.0), REACH), None);
    session.select(Some(locked));
    assert!(!session.nudge_selected(5.0, 0.0));
    assert!(!session.remove_selected());
    assert!(!session.set_option_on_selected("width", number_to_value(9.0)));
    assert!(!session.begin_change(locked));
    assert_eq!(origin(&session, locked), (100.0, 100.0));
    assert_eq!(session.count(), 2);

    assert!(session.set_locked(locked, false));
    assert_eq!(session.pick(point(150.0, 150.0), REACH), Some(locked));
}

#[test]
fn a_text_outside_the_crop_cannot_be_picked() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let text_sheet = definition("text").expect("text");
    session.add(
        Shape::new(
            "text",
            Geometry::Anchor(point(20.0, 20.0)),
            Options::defaults(text_sheet.options),
        )
        .with_option("content", json!("Hello")),
    );
    assert!(session.pick(point(25.0, 25.0), REACH).is_some());

    session.set_crop(Some(Crop {
        x: 400,
        y: 300,
        width: 300,
        height: 200,
    }));

    assert_eq!(session.pick(point(25.0, 25.0), REACH), None);
}

#[test]
fn a_series_of_nudges_is_a_single_undo_step_and_other_edits_end_the_series() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));

    for _ in 0..30 {
        assert!(session.nudge_selected(1.0, 0.0));
    }
    assert!(near(origin(&session, id).0, 130.0));
    assert!(session.undo());
    assert!(near(origin(&session, id).0, 100.0));
    assert!(session.undo());
    assert_eq!(session.count(), 0);

    assert!(session.redo());
    assert!(session.redo());
    assert!(near(origin(&session, id).0, 130.0));
}

#[test]
fn a_new_selection_or_another_edit_starts_a_new_nudge_series() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));
    session.nudge_selected(5.0, 0.0);
    session.set_option_on_selected("width", number_to_value(9.0));
    session.nudge_selected(5.0, 0.0);
    session.nudge_selected(5.0, 0.0);

    assert!(session.undo());
    assert!(near(origin(&session, id).0, 105.0));
    assert!(session.undo());
    assert!(session.undo());
    assert!(near(origin(&session, id).0, 100.0));
}

#[test]
fn nudging_after_a_save_makes_the_document_modified_again() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(boxed("rectangle", 100.0, 100.0, 50.0, 50.0));
    session.nudge_selected(1.0, 0.0);
    session.mark_saved();
    assert!(!session.is_modified());

    session.nudge_selected(1.0, 0.0);

    assert!(session.is_modified());
}
