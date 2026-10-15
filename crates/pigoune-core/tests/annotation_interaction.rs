use pigoune_core::annotations::{
    AnnotationDocument, Color, CursorKind, DrawingTool, EditSession, Handle, Interaction, Point,
    Rect, Shape, Stroke, Style, Tool, shape_bounds,
};

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::Rectangle {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        stroke: Stroke {
            color: Color {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            },
            width: 4.0,
        },
        fill: None,
    }
}

fn bounds(session: &EditSession, id: u32) -> Rect {
    shape_bounds(session.shape_of(id).expect("shape")).expect("bounds")
}

fn drag(
    interaction: &mut Interaction,
    session: &mut EditSession,
    tool: Tool,
    from: Point,
    to: Point,
) -> bool {
    let claimed = interaction.press(session, tool, &Style::default(), from, REACH);
    if claimed {
        let middle = point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y));
        interaction.drag(session, middle);
        interaction.release(session, to);
    }
    claimed
}

fn fresh() -> (Interaction, EditSession) {
    (
        Interaction::new(),
        EditSession::new(AnnotationDocument::default()),
    )
}

#[test]
fn dragging_with_a_drawing_tool_adds_one_selected_shape() {
    let (mut interaction, mut session) = fresh();
    let tool = Tool::Draw(DrawingTool::Rectangle);

    assert!(drag(
        &mut interaction,
        &mut session,
        tool,
        point(10.0, 10.0),
        point(110.0, 60.0)
    ));

    assert_eq!(session.count(), 1);
    let id = session.selected().expect("the new shape is selected");
    let rect = bounds(&session, id);
    assert!(same(rect.width, 100.0) && same(rect.height, 50.0));
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn the_shape_is_only_pending_until_the_button_is_released() {
    let (mut interaction, mut session) = fresh();
    let tool = Tool::Draw(DrawingTool::Arrow);

    interaction.press(
        &mut session,
        tool,
        &Style::default(),
        point(0.0, 0.0),
        REACH,
    );
    assert!(interaction.is_active());
    interaction.drag(&mut session, point(80.0, 40.0));

    assert!(interaction.pending().is_some());
    assert_eq!(session.count(), 0);
    interaction.release(&mut session, point(90.0, 45.0));
    assert!(interaction.pending().is_none());
    assert!(!interaction.is_active());
    assert_eq!(session.count(), 1);
}

#[test]
fn a_click_with_a_drawing_tool_adds_nothing() {
    let (mut interaction, mut session) = fresh();
    let tool = Tool::Draw(DrawingTool::Ellipse);

    drag(
        &mut interaction,
        &mut session,
        tool,
        point(5.0, 5.0),
        point(6.0, 5.0),
    );

    assert_eq!(session.count(), 0);
    assert!(!session.can_undo());
}

#[test]
fn the_new_shape_takes_the_current_style() {
    let (mut interaction, mut session) = fresh();
    let style = Style {
        width: 15.0,
        opacity: 0.5,
        ..Style::default()
    };
    interaction.press(
        &mut session,
        Tool::Draw(DrawingTool::Line),
        &style,
        point(0.0, 0.0),
        REACH,
    );
    interaction.release(&mut session, point(50.0, 50.0));

    let Some(Shape::Line { stroke, .. }) = session.selected_shape() else {
        panic!("a line");
    };
    assert!(same(stroke.width, 15.0));
    assert_eq!(stroke.color.a, 128);
}

#[test]
fn a_drawing_tool_never_grabs_an_existing_shape() {
    let (mut interaction, mut session) = fresh();
    let existing = session.add(rectangle(0.0, 0.0, 100.0, 100.0));
    let before = bounds(&session, existing);

    drag(
        &mut interaction,
        &mut session,
        Tool::Draw(DrawingTool::Rectangle),
        point(0.0, 50.0),
        point(200.0, 90.0),
    );

    assert_eq!(session.count(), 2);
    assert_eq!(bounds(&session, existing), before);
}

#[test]
fn an_unsupported_tool_lets_the_view_pan() {
    let (mut interaction, mut session) = fresh();
    session.add(rectangle(0.0, 0.0, 100.0, 100.0));

    assert!(!interaction.press(
        &mut session,
        Tool::Unsupported,
        &Style::default(),
        point(0.0, 50.0),
        REACH
    ));
    assert!(!interaction.is_active());
}

#[test]
fn clicking_a_shape_with_the_select_tool_selects_and_moves_it() {
    let (mut interaction, mut session) = fresh();
    let id = session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    session.select(None);

    assert!(drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(100.0, 130.0),
        point(130.0, 150.0)
    ));

    assert_eq!(session.selected(), Some(id));
    let rect = bounds(&session, id);
    assert!(same(rect.x, 130.0) && same(rect.y, 120.0));
    assert!(session.undo());
    let back = bounds(&session, id);
    assert!(same(back.x, 100.0) && same(back.y, 100.0));
}

#[test]
fn a_press_on_empty_space_deselects_and_lets_the_view_pan() {
    let (mut interaction, mut session) = fresh();
    session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    assert!(session.selected().is_some());

    let claimed = interaction.press(
        &mut session,
        Tool::Select,
        &Style::default(),
        point(500.0, 500.0),
        REACH,
    );

    assert!(!claimed);
    assert_eq!(session.selected(), None);
}

#[test]
fn dragging_a_handle_resizes_the_selected_shape_in_one_change() {
    let (mut interaction, mut session) = fresh();
    let id = session.add(rectangle(100.0, 100.0, 100.0, 60.0));

    assert!(drag(
        &mut interaction,
        &mut session,
        Tool::Select,
        point(200.0, 160.0),
        point(260.0, 200.0)
    ));

    let rect = bounds(&session, id);
    assert!(same(rect.width, 160.0) && same(rect.height, 100.0));
    assert!(same(rect.x, 100.0) && same(rect.y, 100.0));
    assert!(session.undo());
    let back = bounds(&session, id);
    assert!(same(back.width, 100.0) && same(back.height, 60.0));
}

#[test]
fn a_handle_wins_over_a_shape_underneath() {
    let (mut interaction, mut session) = fresh();
    session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    let top = session.add(rectangle(150.0, 100.0, 100.0, 60.0));
    session.select(Some(top));

    interaction.press(
        &mut session,
        Tool::Select,
        &Style::default(),
        point(150.0, 100.0),
        REACH,
    );
    interaction.release(&mut session, point(120.0, 100.0));

    let rect = bounds(&session, top);
    assert!(same(rect.x, 120.0));
    assert!(same(rect.width, 130.0));
}

#[test]
fn escape_cancels_a_move_in_progress() {
    let (mut interaction, mut session) = fresh();
    let id = session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    session.select(None);
    interaction.press(
        &mut session,
        Tool::Select,
        &Style::default(),
        point(100.0, 130.0),
        REACH,
    );
    interaction.drag(&mut session, point(300.0, 330.0));
    assert!(!same(bounds(&session, id).x, 100.0));

    interaction.cancel(&mut session);

    assert!(same(bounds(&session, id).x, 100.0));
    assert!(!interaction.is_active());
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn cancelling_a_drawing_discards_the_pending_shape() {
    let (mut interaction, mut session) = fresh();
    interaction.press(
        &mut session,
        Tool::Draw(DrawingTool::Rectangle),
        &Style::default(),
        point(0.0, 0.0),
        REACH,
    );
    interaction.drag(&mut session, point(50.0, 50.0));
    assert!(interaction.pending().is_some());

    interaction.cancel(&mut session);

    assert!(interaction.pending().is_none());
    assert_eq!(session.count(), 0);
}

#[test]
fn a_move_that_goes_nowhere_leaves_no_history() {
    let (mut interaction, mut session) = fresh();
    session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    session.undo();
    session.redo();
    session.select(None);

    interaction.press(
        &mut session,
        Tool::Select,
        &Style::default(),
        point(100.0, 130.0),
        REACH,
    );
    interaction.release(&mut session, point(100.0, 130.0));

    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn the_cursor_tells_what_a_click_would_do() {
    let (_, mut session) = fresh();
    let id = session.add(rectangle(100.0, 100.0, 100.0, 60.0));
    session.select(Some(id));

    let cursor = |tool, x, y| Interaction::cursor(&session, tool, point(x, y), REACH);

    assert_eq!(
        cursor(Tool::Select, 200.0, 160.0),
        CursorKind::Resize(Handle::BottomRight)
    );
    assert_eq!(cursor(Tool::Select, 100.0, 110.0), CursorKind::Move);
    assert_eq!(
        cursor(Tool::Select, 100.0, 131.0),
        CursorKind::Resize(Handle::Left)
    );
    assert_eq!(cursor(Tool::Select, 400.0, 400.0), CursorKind::Default);
    assert_eq!(
        cursor(Tool::Draw(DrawingTool::Arrow), 400.0, 400.0),
        CursorKind::Crosshair
    );
    assert_eq!(cursor(Tool::Unsupported, 100.0, 110.0), CursorKind::Default);
    session.select(None);
    assert_eq!(
        Interaction::cursor(&session, Tool::Select, point(200.0, 160.0), REACH),
        CursorKind::Move
    );
}

#[test]
fn starting_to_draw_drops_the_previous_selection() {
    let (mut interaction, mut session) = fresh();
    let existing = session.add(rectangle(0.0, 0.0, 100.0, 100.0));
    assert_eq!(session.selected(), Some(existing));

    interaction.press(
        &mut session,
        Tool::Draw(DrawingTool::Line),
        &Style::default(),
        point(300.0, 300.0),
        REACH,
    );

    assert_eq!(session.selected(), None);
}
