use pigoune_core::annotations::{
    AnnotationDocument, Color, DrawingTool, EditSession, Font, Handle, Point, Rect, Shape, Stroke,
    Style, arrow_head, handles, hit, rect_between, resized, restyled, shape_bounds,
    shape_from_drag, style_of, translated,
};

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn color(a: u8) -> Color {
    Color {
        r: 10,
        g: 20,
        b: 30,
        a,
    }
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        color: color(255),
        width,
    }
}

fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::Rectangle {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        stroke: stroke(4.0),
        fill: None,
    }
}

fn filled(shape: Shape) -> Shape {
    match shape {
        Shape::Rectangle { bounds, stroke, .. } => Shape::Rectangle {
            bounds,
            stroke,
            fill: Some(color(255)),
        },
        Shape::Ellipse { bounds, stroke, .. } => Shape::Ellipse {
            bounds,
            stroke,
            fill: Some(color(255)),
        },
        other => other,
    }
}

fn ellipse(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::Ellipse {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        stroke: stroke(4.0),
        fill: None,
    }
}

fn line(from: Point, to: Point) -> Shape {
    Shape::Line {
        from,
        to,
        stroke: stroke(4.0),
    }
}

fn blur(x: f32, y: f32, width: f32, height: f32) -> Shape {
    Shape::Blur {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        radius: 8.0,
    }
}

fn float(number: u32) -> f32 {
    f32::from(u16::try_from(number).expect("a small number"))
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-4
}

fn bounds_of(shape: &Shape) -> Rect {
    shape_bounds(shape).expect("the shape has bounds")
}

#[test]
fn dragging_a_tool_makes_the_matching_shape_whatever_the_direction() {
    let style = Style::default();
    for tool in [
        DrawingTool::Rectangle,
        DrawingTool::Ellipse,
        DrawingTool::Line,
        DrawingTool::Arrow,
    ] {
        let forward = shape_from_drag(tool, point(10.0, 20.0), point(110.0, 70.0), &style);
        let backward = shape_from_drag(tool, point(110.0, 70.0), point(10.0, 20.0), &style);
        assert!(forward.is_some(), "{tool:?}");
        assert_eq!(
            bounds_of(&forward.expect("shape")),
            bounds_of(&backward.expect("shape"))
        );
    }
    let Some(Shape::Rectangle { bounds, .. }) = shape_from_drag(
        DrawingTool::Rectangle,
        point(110.0, 70.0),
        point(10.0, 20.0),
        &style,
    ) else {
        panic!("a rectangle");
    };
    assert_eq!(
        bounds,
        Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0
        }
    );
}

#[test]
fn a_tiny_drag_makes_nothing() {
    for tool in [DrawingTool::Rectangle, DrawingTool::Arrow] {
        assert!(
            shape_from_drag(tool, point(5.0, 5.0), point(6.0, 7.0), &Style::default()).is_none()
        );
        assert!(
            shape_from_drag(tool, point(5.0, 5.0), point(5.0, 5.0), &Style::default()).is_none()
        );
    }
}

#[test]
fn the_style_is_applied_with_the_opacity_as_alpha() {
    let style = Style {
        color: Color {
            r: 1,
            g: 2,
            b: 3,
            a: 255,
        },
        width: 9.0,
        opacity: 0.5,
    };
    let Some(Shape::Line { stroke, .. }) =
        shape_from_drag(DrawingTool::Line, point(0.0, 0.0), point(50.0, 0.0), &style)
    else {
        panic!("a line");
    };
    assert!(same(stroke.width, 9.0));
    assert_eq!((stroke.color.r, stroke.color.g, stroke.color.b), (1, 2, 3));
    assert_eq!(stroke.color.a, 128);
    let back = style_of(&Shape::Line {
        from: point(0.0, 0.0),
        to: point(1.0, 1.0),
        stroke,
    })
    .expect("a style");
    assert!(same(back.width, 9.0));
    assert!((back.opacity - 0.5).abs() < 0.01);
    assert_eq!(back.color.a, 255);
}

#[test]
fn restyling_changes_only_the_style() {
    let style = Style {
        color: Color {
            r: 7,
            g: 8,
            b: 9,
            a: 255,
        },
        width: 12.0,
        opacity: 0.25,
    };
    let before = rectangle(10.0, 10.0, 40.0, 30.0);
    let after = restyled(&before, &style);
    assert_eq!(shape_bounds(&before), shape_bounds(&after));
    assert_ne!(before, after);
    let read = style_of(&after).expect("a style");
    assert!(same(read.width, 12.0));
    assert_eq!(
        restyled(&blur(0.0, 0.0, 5.0, 5.0), &style),
        blur(0.0, 0.0, 5.0, 5.0)
    );
    assert!(style_of(&blur(0.0, 0.0, 5.0, 5.0)).is_none());
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
    let shape = filled(rectangle(100.0, 100.0, 200.0, 100.0));
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
    assert!(hit(&filled(shape), point(200.0, 150.0), 0.0));
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
    let arrow = Shape::Arrow {
        from: point(0.0, 0.0),
        to: point(100.0, 100.0),
        stroke: stroke(4.0),
    };
    assert!(hit(&arrow, point(50.0, 51.0), 2.0));
    assert!(!hit(&arrow, point(50.0, 70.0), 2.0));
    let dot = line(point(5.0, 5.0), point(5.0, 5.0));
    assert!(hit(&dot, point(6.0, 5.0), 2.0));
}

#[test]
fn a_blur_is_hit_anywhere_inside() {
    let shape = blur(10.0, 10.0, 100.0, 50.0);
    assert!(hit(&shape, point(60.0, 30.0), 0.0));
    assert!(!hit(&shape, point(200.0, 30.0), 0.0));
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
    let text = Shape::Text {
        origin: point(0.0, 0.0),
        content: "x".to_owned(),
        font: Font {
            family: "Sans".to_owned(),
            size: 12.0,
            bold: false,
            italic: false,
        },
        color: color(255),
    };
    assert!(handles(&text).is_empty());
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
fn translating_moves_every_kind_of_shape() {
    let shapes = [
        rectangle(0.0, 0.0, 10.0, 10.0),
        ellipse(0.0, 0.0, 10.0, 10.0),
        line(point(0.0, 0.0), point(10.0, 10.0)),
        blur(0.0, 0.0, 10.0, 10.0),
        Shape::Pen {
            points: vec![point(0.0, 0.0), point(5.0, 5.0)],
            stroke: stroke(2.0),
        },
    ];
    for shape in shapes {
        let moved = translated(&shape, 7.0, -3.0);
        let (before, after) = (bounds_of(&shape), bounds_of(&moved));
        assert_eq!((after.x - before.x, after.y - before.y), (7.0, -3.0));
        assert_eq!((after.width, after.height), (before.width, before.height));
        assert_eq!(translated(&moved, -7.0, 3.0), shape);
    }
    let step = Shape::Step {
        center: point(1.0, 2.0),
        number: 1,
        radius: 10.0,
        color: color(255),
    };
    let Shape::Step { center, .. } = translated(&step, 4.0, 4.0) else {
        panic!("a step");
    };
    assert_eq!(center, point(5.0, 6.0));
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
             "stroke":{"color":{"r":1,"g":2,"b":3,"a":255},"width":4.0},"fill":{"r":1,"g":2,"b":3,"a":255}},
            {"id":2,"kind":"lasso","points":[]},
            {"id":3,"kind":"rectangle","bounds":{"x":50.0,"y":50.0,"width":100.0,"height":100.0},
             "stroke":{"color":{"r":1,"g":2,"b":3,"a":255},"width":4.0},"fill":{"r":1,"g":2,"b":3,"a":255}}
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
    let same = session.shape_of(id).cloned().expect("shape");
    session.preview(same);
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
fn restyling_the_selection_is_one_undoable_change() {
    let mut session = session();
    let id = session.add(rectangle(0.0, 0.0, 10.0, 10.0));
    let original = session.shape_of(id).cloned().expect("shape");
    let style = Style {
        width: 11.0,
        opacity: 0.4,
        ..Style::default()
    };

    assert!(session.restyle_selected(&style));
    assert_ne!(session.shape_of(id), Some(&original));
    assert!(!session.restyle_selected(&style));
    assert!(session.undo());
    assert_eq!(session.shape_of(id), Some(&original));
    session.select(None);
    assert!(!session.restyle_selected(&style));
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

#[test]
fn undoing_everything_then_redoing_everything_gives_the_same_states() {
    let mut seed: u32 = 7;
    let mut next = move |limit: u32| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) % limit
    };
    let mut edits = session();
    let mut states = vec![edits.document().to_json().expect("json")];
    for _ in 0..300 {
        let changed = match next(5) {
            0 | 1 => {
                let x = float(next(500));
                let shape = match next(3) {
                    0 => rectangle(x, 10.0, 40.0, 30.0),
                    1 => ellipse(x, 10.0, 40.0, 30.0),
                    _ => line(point(x, 0.0), point(x + 20.0, 30.0)),
                };
                edits.add(shape);
                true
            }
            2 => {
                let ids: Vec<u32> = edits.document().objects.iter().map(|o| o.id).collect();
                if ids.is_empty() {
                    false
                } else {
                    edits.select(Some(
                        ids[usize::try_from(next(u32::try_from(ids.len()).expect("few")))
                            .expect("index")],
                    ));
                    edits.remove_selected()
                }
            }
            3 => {
                let ids: Vec<u32> = edits.document().objects.iter().map(|o| o.id).collect();
                if ids.is_empty() {
                    false
                } else {
                    let id = ids[usize::try_from(next(u32::try_from(ids.len()).expect("few")))
                        .expect("index")];
                    let original = edits.shape_of(id).cloned().expect("shape");
                    edits.begin_change(id);
                    edits.preview(translated(&original, float(next(50)) + 1.0, 2.0));
                    edits.end_change()
                }
            }
            _ => {
                let ids: Vec<u32> = edits.document().objects.iter().map(|o| o.id).collect();
                if ids.is_empty() {
                    false
                } else {
                    edits.select(Some(
                        ids[usize::try_from(next(u32::try_from(ids.len()).expect("few")))
                            .expect("index")],
                    ));
                    let style = Style {
                        width: float(next(30)) + 1.0,
                        ..Style::default()
                    };
                    edits.restyle_selected(&style)
                }
            }
        };
        if changed {
            states.push(edits.document().to_json().expect("json"));
        }
    }
    assert!(states.len() > 150);

    for expected in states.iter().rev().skip(1) {
        assert!(edits.undo());
        assert_eq!(&edits.document().to_json().expect("json"), expected);
    }
    assert!(!edits.undo() || states.len() > 501);
    for expected in states.iter().skip(1) {
        assert!(edits.redo());
        assert_eq!(&edits.document().to_json().expect("json"), expected);
    }
    assert!(!edits.redo());
}

#[test]
fn an_arrow_head_points_at_the_end_and_is_symmetric() {
    let head = arrow_head(point(0.0, 0.0), point(100.0, 0.0), 4.0);
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
    let thin = arrow_head(point(0.0, 0.0), point(0.0, 200.0), 2.0);
    let thick = arrow_head(point(0.0, 0.0), point(0.0, 200.0), 20.0);
    assert!(thick.base.y < thin.base.y);
    assert!(same(thin.left.y, thin.right.y));
    assert!(same(thin.left.x, -thin.right.x));
}

#[test]
fn an_arrow_head_never_reaches_past_a_short_arrow() {
    let head = arrow_head(point(0.0, 0.0), point(10.0, 0.0), 30.0);
    assert!(head.base.x > 0.0);
    assert!(head.base.x < 10.0);
    let still = arrow_head(point(5.0, 5.0), point(5.0, 5.0), 4.0);
    for corner in [still.tip, still.left, still.right, still.base] {
        assert!(corner.x.is_finite() && corner.y.is_finite());
    }
}
