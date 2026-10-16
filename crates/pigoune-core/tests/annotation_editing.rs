use pigoune_core::annotations::{
    ARROW, AnnotationDocument, BLUR, Crop, ELLIPSE, EditSession, Geometry, Handle, LINE, Options,
    Point, RECTANGLE, Rect, Shape, ToolDefinition, arrow_head, handles, head_size_factor, hit,
    number_to_value, rect_between, resized, shape_bounds, shape_from_drag, translated,
};
use serde_json::json;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn options_of(tool: &ToolDefinition) -> Options {
    Options::defaults(tool.options).with("width", number_to_value(4.0))
}

fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        options_of(&RECTANGLE),
    )
}

fn ellipse(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::new(
        "ellipse",
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        options_of(&ELLIPSE),
    )
}

fn line(from: Point, to: Point) -> Shape {
    Shape::new("line", Geometry::TwoPoints { from, to }, options_of(&LINE))
}

fn arrow(from: Point, to: Point) -> Shape {
    Shape::new(
        "arrow",
        Geometry::TwoPoints { from, to },
        options_of(&ARROW),
    )
}

fn blur(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::new(
        "blur",
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        Options::defaults(BLUR.options),
    )
}

fn filled(shape: &Shape) -> Shape {
    shape.with_option("fill", json!(true))
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-4
}

fn float(number: u32) -> f32 {
    f32::from(u16::try_from(number).expect("a small number"))
}

fn bounds_of(shape: &Shape) -> Rect {
    shape_bounds(shape).expect("the shape has bounds")
}

fn drag_shape(tool: &ToolDefinition, from: Point, to: Point) -> Option<Shape> {
    shape_from_drag(tool, from, to, &Options::defaults(tool.options))
}

#[test]
fn dragging_a_tool_makes_the_matching_shape_whatever_the_direction() {
    for tool in [&RECTANGLE, &ELLIPSE, &LINE, &ARROW] {
        let forward = drag_shape(tool, point(10.0, 20.0), point(110.0, 70.0)).expect("shape");
        let backward = drag_shape(tool, point(110.0, 70.0), point(10.0, 20.0)).expect("shape");
        assert_eq!(forward.kind, tool.kind);
        assert_eq!(bounds_of(&forward), bounds_of(&backward));
    }
    let shape = drag_shape(&RECTANGLE, point(110.0, 70.0), point(10.0, 20.0)).expect("shape");
    assert_eq!(
        shape.geometry,
        Geometry::Box(Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0
        })
    );
}

#[test]
fn a_tiny_drag_makes_nothing() {
    for tool in [&RECTANGLE, &ARROW] {
        assert!(drag_shape(tool, point(5.0, 5.0), point(6.0, 7.0)).is_none());
        assert!(drag_shape(tool, point(5.0, 5.0), point(5.0, 5.0)).is_none());
    }
}

#[test]
fn the_options_given_to_the_drag_end_up_in_the_shape() {
    let options = Options::defaults(ARROW.options)
        .with("width", number_to_value(9.0))
        .with("head", json!("double"));
    let shape =
        shape_from_drag(&ARROW, point(0.0, 0.0), point(50.0, 0.0), &options).expect("shape");
    assert!(same(shape.number("width"), 9.0));
    assert_eq!(shape.choice("head"), "double");
}

#[test]
fn an_unfilled_rectangle_is_hit_on_its_edge_only() {
    let shape = rectangle(100.0, 100.0, 200.0, 100.0);
    assert!(hit(&shape, point(100.0, 150.0), 3.0));
    assert!(hit(&shape, point(200.0, 100.0), 3.0));
    assert!(hit(&shape, point(299.0, 199.0), 3.0));
    assert!(hit(&shape, point(97.0, 150.0), 3.0));
    assert!(!hit(&shape, point(200.0, 150.0), 3.0));
    assert!(!hit(&shape, point(50.0, 150.0), 3.0));
    assert!(!hit(&shape, point(200.0, 90.0), 3.0));
}

#[test]
fn a_filled_rectangle_is_hit_inside_too() {
    let shape = filled(&rectangle(100.0, 100.0, 200.0, 100.0));
    assert!(hit(&shape, point(200.0, 150.0), 0.0));
    assert!(!hit(&shape, point(320.0, 150.0), 0.0));
}

#[test]
fn an_ellipse_is_hit_on_its_outline() {
    let shape = ellipse(100.0, 100.0, 200.0, 100.0);
    assert!(hit(&shape, point(100.0, 150.0), 3.0));
    assert!(hit(&shape, point(300.0, 150.0), 3.0));
    assert!(hit(&shape, point(200.0, 100.0), 3.0));
    assert!(hit(&shape, point(200.0, 200.0), 3.0));
    assert!(!hit(&shape, point(200.0, 150.0), 3.0));
    assert!(!hit(&shape, point(105.0, 105.0), 1.0));
    assert!(hit(&filled(&shape), point(200.0, 150.0), 0.0));
}

#[test]
fn a_degenerate_ellipse_is_never_hit() {
    assert!(!hit(
        &ellipse(10.0, 10.0, 0.0, 50.0),
        point(10.0, 30.0),
        3.0
    ));
}

#[test]
fn a_line_and_an_arrow_are_hit_near_the_segment() {
    let segment = line(point(0.0, 0.0), point(100.0, 0.0));
    assert!(hit(&segment, point(50.0, 0.0), 2.0));
    assert!(hit(&segment, point(50.0, 3.5), 2.0));
    assert!(!hit(&segment, point(50.0, 6.0), 2.0));
    assert!(!hit(&segment, point(120.0, 0.0), 2.0));
    assert!(hit(&segment, point(102.0, 0.0), 2.0));
    let diagonal = arrow(point(0.0, 0.0), point(100.0, 100.0));
    assert!(hit(&diagonal, point(50.0, 51.0), 2.0));
    assert!(!hit(&diagonal, point(50.0, 70.0), 2.0));
    let dot = line(point(5.0, 5.0), point(5.0, 5.0));
    assert!(hit(&dot, point(6.0, 5.0), 2.0));
}

#[test]
fn a_thicker_stroke_is_easier_to_hit() {
    let thin = line(point(0.0, 0.0), point(100.0, 0.0));
    let thick = thin.with_option("width", number_to_value(30.0));
    assert!(!hit(&thin, point(50.0, 12.0), 1.0));
    assert!(hit(&thick, point(50.0, 12.0), 1.0));
}

#[test]
fn a_blur_is_hit_anywhere_inside() {
    let shape = blur(10.0, 10.0, 100.0, 50.0);
    assert!(hit(&shape, point(60.0, 30.0), 0.0));
    assert!(!hit(&shape, point(200.0, 30.0), 0.0));
}

#[test]
fn an_unknown_tool_is_never_hit() {
    let strange = Shape::new(
        "lasso",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }),
        Options::new(),
    );
    assert!(!hit(&strange, point(25.0, 25.0), 5.0));
}

#[test]
fn a_rectangle_has_eight_handles_and_a_line_two() {
    let found = handles(&rectangle(10.0, 20.0, 100.0, 60.0));
    let names: Vec<Handle> = found.iter().map(|(handle, _)| *handle).collect();
    assert_eq!(
        names,
        [
            Handle::TopLeft,
            Handle::Top,
            Handle::TopRight,
            Handle::Right,
            Handle::BottomRight,
            Handle::Bottom,
            Handle::BottomLeft,
            Handle::Left
        ]
    );
    assert_eq!(found[0].1, point(10.0, 20.0));
    assert_eq!(found[4].1, point(110.0, 80.0));
    assert_eq!(found[1].1, point(60.0, 20.0));
    assert_eq!(found[3].1, point(110.0, 50.0));
    let ends = handles(&line(point(1.0, 2.0), point(3.0, 4.0)));
    assert_eq!(
        ends,
        [
            (Handle::Start, point(1.0, 2.0)),
            (Handle::End, point(3.0, 4.0))
        ]
    );
    let anchored = Shape::new("text", Geometry::Anchor(point(0.0, 0.0)), Options::new());
    assert!(handles(&anchored).is_empty());
}

#[test]
fn resizing_moves_the_dragged_edges_only() {
    let shape = rectangle(100.0, 100.0, 100.0, 50.0);
    let expect = |handle, to: Point, x, y, width, height| {
        let moved = resized(&shape, handle, to);
        assert_eq!(
            bounds_of(&moved),
            Rect {
                x,
                y,
                width,
                height
            },
            "{handle:?}"
        );
    };
    expect(Handle::TopLeft, point(80.0, 90.0), 80.0, 90.0, 120.0, 60.0);
    expect(Handle::Top, point(500.0, 90.0), 100.0, 90.0, 100.0, 60.0);
    expect(
        Handle::TopRight,
        point(250.0, 90.0),
        100.0,
        90.0,
        150.0,
        60.0,
    );
    expect(
        Handle::Right,
        point(260.0, 500.0),
        100.0,
        100.0,
        160.0,
        50.0,
    );
    expect(
        Handle::BottomRight,
        point(260.0, 200.0),
        100.0,
        100.0,
        160.0,
        100.0,
    );
    expect(Handle::Bottom, point(0.0, 180.0), 100.0, 100.0, 100.0, 80.0);
    expect(
        Handle::BottomLeft,
        point(90.0, 180.0),
        90.0,
        100.0,
        110.0,
        80.0,
    );
    expect(Handle::Left, point(70.0, 0.0), 70.0, 100.0, 130.0, 50.0);
}

#[test]
fn resizing_keeps_the_options_of_the_shape() {
    let shape = rectangle(0.0, 0.0, 10.0, 10.0).with_option("corner_radius", json!(7));
    let moved = resized(&shape, Handle::Right, point(40.0, 0.0));
    assert_eq!(moved.options, shape.options);
    assert_eq!(moved.kind, shape.kind);
    assert_eq!(translated(&shape, 3.0, 3.0).options, shape.options);
}

#[test]
fn dragging_a_handle_past_the_opposite_side_flips_the_shape() {
    let shape = ellipse(100.0, 100.0, 100.0, 50.0);
    let flipped = resized(&shape, Handle::Right, point(60.0, 0.0));
    assert_eq!(
        bounds_of(&flipped),
        Rect {
            x: 60.0,
            y: 100.0,
            width: 40.0,
            height: 50.0
        }
    );
    let corner = resized(&shape, Handle::TopLeft, point(300.0, 300.0));
    assert_eq!(
        bounds_of(&corner),
        Rect {
            x: 200.0,
            y: 150.0,
            width: 100.0,
            height: 150.0
        }
    );
}

#[test]
fn resizing_a_line_moves_one_end() {
    let shape = line(point(0.0, 0.0), point(10.0, 10.0));
    assert_eq!(
        resized(&shape, Handle::Start, point(5.0, 6.0)),
        line(point(5.0, 6.0), point(10.0, 10.0))
    );
    assert_eq!(
        resized(&shape, Handle::End, point(7.0, 8.0)),
        line(point(0.0, 0.0), point(7.0, 8.0))
    );
    assert_eq!(resized(&shape, Handle::Top, point(7.0, 8.0)), shape);
}

#[test]
fn translating_moves_every_kind_of_geometry() {
    let polyline = Shape::new(
        "pen",
        Geometry::Polyline(vec![point(0.0, 0.0), point(5.0, 5.0)]),
        Options::new(),
    );
    let shapes = [
        rectangle(0.0, 0.0, 10.0, 10.0),
        ellipse(0.0, 0.0, 10.0, 10.0),
        line(point(0.0, 0.0), point(10.0, 10.0)),
        blur(0.0, 0.0, 10.0, 10.0),
        polyline,
    ];
    for shape in shapes {
        let moved = translated(&shape, 7.0, -3.0);
        let (before, after) = (bounds_of(&shape), bounds_of(&moved));
        assert!(same(after.x - before.x, 7.0) && same(after.y - before.y, -3.0));
        assert!(same(after.width, before.width) && same(after.height, before.height));
        assert_eq!(translated(&moved, -7.0, 3.0), shape);
    }
    let anchored = Shape::new("text", Geometry::Anchor(point(1.0, 2.0)), Options::new());
    assert_eq!(
        translated(&anchored, 4.0, 4.0).geometry,
        Geometry::Anchor(point(5.0, 6.0))
    );
}

#[test]
fn rect_between_orders_the_corners() {
    assert_eq!(
        rect_between(point(9.0, 1.0), point(3.0, 5.0)),
        Rect {
            x: 3.0,
            y: 1.0,
            width: 6.0,
            height: 4.0
        }
    );
}

fn session() -> EditSession {
    EditSession::new(AnnotationDocument::default())
}

#[test]
fn adding_selects_the_new_object_and_can_be_undone_and_redone() {
    let mut session = session();
    assert!(!session.can_undo());

    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));

    assert_eq!(session.count(), 1);
    assert_eq!(session.selected(), Some(id));
    assert!(session.can_undo());
    assert!(session.undo());
    assert_eq!(session.count(), 0);
    assert_eq!(session.selected(), None);
    assert!(session.can_redo());
    assert!(session.redo());
    assert_eq!(session.count(), 1);
    assert_eq!(session.selected(), Some(id));
    assert!(!session.redo());
}

#[test]
fn a_new_edit_forgets_what_could_be_redone() {
    let mut session = session();
    session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.undo();
    session.add(ellipse(0.0, 0.0, 10.0, 10.0));
    assert!(!session.can_redo());
}

#[test]
fn removing_and_undoing_puts_the_object_back_at_the_same_place() {
    let mut session = session();
    let first = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    let second = session.add(ellipse(0.0, 0.0, 10.0, 10.0));
    let third = session.add(line(point(0.0, 0.0), point(5.0, 5.0)));
    session.select(Some(second));

    assert!(session.remove_selected());
    assert_eq!(session.selected(), None);
    let order: Vec<u32> = session.document().objects.iter().map(|o| o.id).collect();
    assert_eq!(order, [first, third]);

    assert!(session.undo());
    let order: Vec<u32> = session.document().objects.iter().map(|o| o.id).collect();
    assert_eq!(order, [first, second, third]);
    assert_eq!(session.selected(), Some(second));
    assert!(!EditSession::new(AnnotationDocument::default()).remove_selected());
}

#[test]
fn the_topmost_object_wins_the_click_and_unknown_tools_are_skipped() {
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[
            {"id":1,"kind":"rectangle","bounds":{"x":0.0,"y":0.0,"width":100.0,"height":100.0},
             "options":{"width":4.0,"fill":true}},
            {"id":2,"kind":"lasso","points":[]},
            {"id":3,"kind":"rectangle","bounds":{"x":50.0,"y":50.0,"width":100.0,"height":100.0},
             "options":{"width":4.0,"fill":true}}
        ]}"#,
    )
    .expect("read");
    let session = EditSession::new(document);

    assert_eq!(session.hit_test(point(75.0, 75.0), 2.0), Some(3));
    assert_eq!(session.hit_test(point(10.0, 10.0), 2.0), Some(1));
    assert_eq!(session.hit_test(point(300.0, 300.0), 2.0), None);
    assert!(session.shape_of(2).is_none());
}

#[test]
fn selecting_an_unknown_id_selects_nothing() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.select(Some(id + 5));
    assert_eq!(session.selected(), None);
    session.select(Some(id));
    assert_eq!(session.selected(), Some(id));
    session.select(None);
    assert_eq!(session.selected(), None);
}

#[test]
fn a_whole_drag_is_one_undoable_change() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    let original = session.shape_of(id).cloned().expect("shape");

    assert!(session.begin_change(id));
    for step in 1..=5_u32 {
        let moved = translated(&original, float(step) * 10.0, 0.0);
        session.preview(moved);
    }
    assert!(session.end_change());

    assert!(same(
        bounds_of(session.shape_of(id).expect("shape")).x,
        50.0
    ));
    assert!(session.undo());
    assert_eq!(session.shape_of(id), Some(&original));
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn a_gesture_that_changes_nothing_leaves_no_history() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.undo();
    session.redo();
    let depth_before = session.can_redo();

    assert!(session.begin_change(id));
    let same_shape = session.shape_of(id).cloned().expect("shape");
    session.preview(same_shape);
    assert!(!session.end_change());

    assert_eq!(session.can_redo(), depth_before);
    session.undo();
    assert_eq!(session.count(), 0);
}

#[test]
fn a_cancelled_gesture_restores_the_shape() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    let original = session.shape_of(id).cloned().expect("shape");
    session.begin_change(id);
    session.preview(translated(&original, 100.0, 100.0));

    session.cancel_change();

    assert_eq!(session.shape_of(id), Some(&original));
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn history_is_blocked_while_a_gesture_is_running() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.begin_change(id);

    assert!(!session.can_undo());
    assert!(!session.undo());
    assert!(!session.begin_change(id));
    assert!(session.is_modified());
}

#[test]
fn changing_an_option_of_the_selection_is_one_undoable_change() {
    let mut session = session();
    let id = session.add(arrow(point(0.0, 0.0), point(50.0, 0.0)));
    let original = session.shape_of(id).cloned().expect("shape");

    assert!(session.set_option_on_selected("head", json!("double")));
    assert_eq!(
        session.shape_of(id).expect("shape").choice("head"),
        "double"
    );
    assert!(!session.set_option_on_selected("head", json!("double")));
    assert!(session.undo());
    assert_eq!(session.shape_of(id), Some(&original));
    assert!(session.redo());
    assert_eq!(
        session.shape_of(id).expect("shape").choice("head"),
        "double"
    );
    session.select(None);
    assert!(!session.set_option_on_selected("head", json!("open")));
}

#[test]
fn a_slider_interaction_on_an_option_is_one_undoable_change() {
    let mut session = session();
    let id = session.add(arrow(point(0.0, 0.0), point(50.0, 0.0)));
    let original = session.shape_of(id).cloned().expect("shape");

    session.begin_option_series();
    for width in [8, 12, 20, 30] {
        assert!(session.set_option_on_selected("width", number_to_value(float(width))));
    }
    session.end_option_series();

    assert!(same(
        session.shape_of(id).expect("shape").number("width"),
        30.0
    ));
    assert!(session.undo());
    assert_eq!(session.shape_of(id), Some(&original));
}

#[test]
fn the_session_knows_when_the_document_changed_since_it_was_saved() {
    let mut session = session();
    assert!(!session.is_modified());

    session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    assert!(session.is_modified());
    session.undo();
    assert!(!session.is_modified());
    session.redo();
    assert!(session.is_modified());

    session.mark_saved();
    assert!(!session.is_modified());
    session.add(ellipse(0.0, 0.0, 5.0, 5.0));
    assert!(session.is_modified());
    session.undo();
    assert!(!session.is_modified());
    session.undo();
    assert!(session.is_modified());
    session.add(line(point(0.0, 0.0), point(9.0, 9.0)));
    assert!(session.is_modified());
    session.undo();
    session.redo();
    assert!(session.is_modified());
}

#[test]
fn only_the_most_recent_edits_are_remembered() {
    let mut session = session();
    for number in 0..520_u32 {
        session.add(rectangle(float(number), 0.0, 10.0, 10.0));
    }
    let mut undone = 0;
    while session.undo() {
        undone += 1;
    }
    assert_eq!(undone, 500);
    assert_eq!(session.count(), 20);
}

#[test]
fn an_unknown_tool_survives_the_edits_of_the_session() {
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[{"id":1,"kind":"lasso","points":[[1,2]]}],"frame":{"margin":3}}"#,
    )
    .expect("read");
    let mut session = EditSession::new(document);
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.remove_selected();
    session.undo();
    assert_eq!(session.selected(), Some(id));

    let written: serde_json::Value =
        serde_json::from_str(&session.document().to_json().expect("written")).expect("json");
    assert_eq!(written["objects"][0]["kind"], "lasso");
    assert_eq!(written["frame"]["margin"], 3);
    assert_eq!(written["objects"].as_array().expect("array").len(), 2);
}

fn content_json(session: &EditSession) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&session.document().to_json().expect("json")).expect("parsed");
    value.as_object_mut().expect("object").remove("next_id");
    value.to_string()
}

#[test]
fn undoing_everything_then_redoing_everything_gives_the_same_states() {
    let mut seed: u32 = 7;
    let mut next = move |limit: u32| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) % limit
    };
    let mut edits = session();
    let mut states = vec![content_json(&edits)];
    let ids = |edits: &EditSession| -> Vec<u32> {
        edits.document().objects.iter().map(|o| o.id).collect()
    };
    for _ in 0..300 {
        let changed = match next(6) {
            0 | 1 => {
                let x = float(next(500));
                let shape = match next(3) {
                    0 => rectangle(x, 10.0, 40.0, 30.0),
                    1 => ellipse(x, 10.0, 40.0, 30.0),
                    _ => arrow(point(x, 0.0), point(x + 20.0, 30.0)),
                };
                edits.add(shape);
                true
            }
            2 => {
                let all = ids(&edits);
                if all.is_empty() {
                    false
                } else {
                    let pick = usize::try_from(next(u32::try_from(all.len()).expect("few")))
                        .expect("index");
                    edits.select(Some(all[pick]));
                    edits.remove_selected()
                }
            }
            3 => {
                let all = ids(&edits);
                if all.is_empty() {
                    false
                } else {
                    let pick = usize::try_from(next(u32::try_from(all.len()).expect("few")))
                        .expect("index");
                    let id = all[pick];
                    let original = edits.shape_of(id).cloned().expect("shape");
                    edits.begin_change(id);
                    edits.preview(translated(&original, float(next(50)) + 1.0, 2.0));
                    edits.end_change()
                }
            }
            4 => {
                let all = ids(&edits);
                if all.is_empty() {
                    false
                } else {
                    let pick = usize::try_from(next(u32::try_from(all.len()).expect("few")))
                        .expect("index");
                    edits.select(Some(all[pick]));
                    edits.set_option_on_selected("width", number_to_value(float(next(30)) + 1.0))
                }
            }
            _ => {
                let all = ids(&edits);
                if all.is_empty() {
                    false
                } else {
                    let pick = usize::try_from(next(u32::try_from(all.len()).expect("few")))
                        .expect("index");
                    edits.select(Some(all[pick]));
                    let choices = ["solid", "dashed", "dotted"];
                    let choice = choices[usize::try_from(next(3)).expect("index")];
                    edits.set_option_on_selected("dash", json!(choice))
                }
            }
        };
        if changed {
            states.push(content_json(&edits));
        }
    }
    assert!(states.len() > 150);

    for expected in states.iter().rev().skip(1) {
        assert!(edits.undo());
        assert_eq!(&content_json(&edits), expected);
    }
    assert!(!edits.undo() || states.len() > 501);
    for expected in states.iter().skip(1) {
        assert!(edits.redo());
        assert_eq!(&content_json(&edits), expected);
    }
    assert!(!edits.redo());
}

#[test]
fn an_arrow_head_points_at_the_end_and_is_symmetric() {
    let head = arrow_head(
        point(0.0, 0.0),
        point(100.0, 0.0),
        4.0,
        head_size_factor("medium"),
    );
    assert_eq!(head.tip, point(100.0, 0.0));
    assert!(same(head.base.y, 0.0));
    assert!(head.base.x < 100.0);
    assert!(same(head.left.x, head.right.x));
    assert!(same(head.left.y, -head.right.y));
    assert!(head.left.y.abs() > 1.0);
    let length = (head.tip.x - head.base.x).hypot(head.tip.y - head.base.y);
    assert!(same(length, 14.0));
}

#[test]
fn an_arrow_head_grows_with_the_stroke_and_follows_the_direction() {
    let factor = head_size_factor("medium");
    let thin = arrow_head(point(0.0, 0.0), point(0.0, 200.0), 2.0, factor);
    let thick = arrow_head(point(0.0, 0.0), point(0.0, 200.0), 20.0, factor);
    assert!(thick.base.y < thin.base.y);
    assert!(same(thin.left.y, thin.right.y));
    assert!(same(thin.left.x, -thin.right.x));
}

#[test]
fn an_arrow_head_never_reaches_past_a_short_arrow() {
    let factor = head_size_factor("large");
    let head = arrow_head(point(0.0, 0.0), point(10.0, 0.0), 30.0, factor);
    assert!(head.base.x > 0.0);
    assert!(head.base.x < 10.0);
    let still = arrow_head(point(5.0, 5.0), point(5.0, 5.0), 4.0, factor);
    for corner in [still.tip, still.left, still.right, still.base] {
        assert!(corner.x.is_finite() && corner.y.is_finite());
    }
}

fn a_crop() -> Crop {
    Crop {
        x: 10,
        y: 20,
        width: 300,
        height: 200,
    }
}

#[test]
fn a_crop_is_one_undoable_change_of_the_document() {
    let mut session = session();
    assert_eq!(session.crop(), None);

    assert!(session.set_crop(Some(a_crop())));
    assert_eq!(session.crop(), Some(a_crop()));
    assert_eq!(session.document().crop, Some(a_crop()));
    assert!(!session.set_crop(Some(a_crop())));

    assert!(session.undo());
    assert_eq!(session.crop(), None);
    assert!(session.redo());
    assert_eq!(session.crop(), Some(a_crop()));
    assert!(session.set_crop(None));
    assert!(session.undo());
    assert_eq!(session.crop(), Some(a_crop()));
}

#[test]
fn a_crop_counts_as_a_change_since_the_last_save_and_is_written_with_the_document() {
    let mut session = session();
    session.mark_saved();

    session.set_crop(Some(a_crop()));

    assert!(session.is_modified());
    let written: serde_json::Value =
        serde_json::from_str(&session.document().to_json().expect("written")).expect("json");
    assert_eq!(written["crop"]["width"], 300);
    session.undo();
    assert!(!session.is_modified());
}

#[test]
fn a_crop_and_the_shapes_share_one_history_in_order() {
    let mut session = session();
    session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    session.set_crop(Some(a_crop()));
    session.add(ellipse(0.0, 0.0, 10.0, 10.0));

    session.undo();
    assert_eq!(session.count(), 1);
    assert_eq!(session.crop(), Some(a_crop()));
    session.undo();
    assert_eq!(session.crop(), None);
    assert_eq!(session.count(), 1);
    session.undo();
    assert_eq!(session.count(), 0);
}
