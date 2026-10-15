use pigoune_core::annotations::{
    ARROW, AnnotationDocument, DefaultValue, EditSession, Geometry, HIGHLIGHTER, Interaction,
    Options, PEN, Point, Shape, Tool, hit, number_to_value, shape_bounds, shape_from_points,
    simplified, value_to_color,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn float(number: u32) -> f32 {
    f32::from(u16::try_from(number).expect("a small number"))
}

fn stroke(points: &[Point]) -> Shape {
    Shape::new(
        "pen",
        Geometry::Polyline(points.to_vec()),
        Options::defaults(PEN.options).with("width", number_to_value(4.0)),
    )
}

fn fresh() -> (Interaction, EditSession) {
    (
        Interaction::new(),
        EditSession::new(AnnotationDocument::default()),
    )
}

fn scribble(interaction: &mut Interaction, session: &mut EditSession, tool: Tool) {
    interaction.press(
        session,
        tool,
        &Options::defaults(PEN.options),
        point(10.0, 10.0),
        REACH,
    );
    for step in 1..=40_u32 {
        let t = float(step);
        interaction.drag(
            session,
            point(10.0 + t * 3.0, 10.0 + (t / 3.0).sin() * 20.0),
        );
    }
    interaction.release(session, point(140.0, 40.0));
}

fn points_of(shape: &Shape) -> Vec<Point> {
    match &shape.geometry {
        Geometry::Polyline(points) => points.clone(),
        other => panic!("not a polyline: {other:?}"),
    }
}

#[test]
fn a_straight_run_of_points_collapses_to_its_two_ends() {
    let line: Vec<Point> = (0..50_u32)
        .map(|i| point(float(i) * 2.0, float(i)))
        .collect();

    let kept = simplified(&line, 0.8);

    assert_eq!(kept, [line[0], line[49]]);
}

#[test]
fn a_corner_survives_the_simplification() {
    let mut path: Vec<Point> = (0..20_u32).map(|i| point(float(i) * 5.0, 0.0)).collect();
    path.extend((1..20_u32).map(|i| point(95.0, float(i) * 5.0)));

    let kept = simplified(&path, 0.8);

    assert_eq!(kept.len(), 3);
    assert_eq!(kept[1], point(95.0, 0.0));
}

#[test]
fn simplifying_keeps_the_ends_and_never_strays_more_than_the_tolerance() {
    let mut seed: u32 = 5;
    let mut next = move |limit: u32| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) % limit
    };
    let path: Vec<Point> = (0..300_u32)
        .map(|i| point(float(i) * 2.0 + float(next(5)), float(next(40))))
        .collect();

    let kept = simplified(&path, 1.5);

    assert_eq!(kept.first(), path.first());
    assert_eq!(kept.last(), path.last());
    assert!(kept.len() < path.len());
    let shape = stroke(&kept);
    for original in &path {
        let near = kept.windows(2).any(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = dx * dx + dy * dy;
            let along =
                (((original.x - a.x) * dx + (original.y - a.y) * dy) / length).clamp(0.0, 1.0);
            (original.x - (a.x + along * dx)).hypot(original.y - (a.y + along * dy)) <= 1.5 + 1e-3
        });
        assert!(near, "{original:?} strays from the simplified stroke");
    }
    assert!(shape_bounds(&shape).is_some());
}

#[test]
fn a_tolerance_of_zero_and_short_paths_change_nothing() {
    let path = [
        point(0.0, 0.0),
        point(5.0, 1.0),
        point(10.0, 0.0),
        point(15.0, 2.0),
    ];
    assert_eq!(simplified(&path, 0.0), path);
    assert_eq!(simplified(&path[..2], 10.0), path[..2]);
    assert!(simplified(&[], 10.0).is_empty());
}

#[test]
fn a_stroke_needs_some_reach_and_is_simplified_only_when_finished() {
    let wiggle = [point(0.0, 0.0), point(1.0, 1.0), point(0.0, 1.0)];
    let options = Options::defaults(PEN.options);
    assert!(shape_from_points(&PEN, &wiggle, &options, true).is_none());
    assert!(shape_from_points(&PEN, &[], &options, true).is_none());

    let long: Vec<Point> = (0..30_u32).map(|i| point(float(i) * 4.0, 0.0)).collect();
    let live = shape_from_points(&PEN, &long, &options, false).expect("a stroke");
    let done = shape_from_points(&PEN, &long, &options, true).expect("a stroke");
    assert_eq!(points_of(&live).len(), 30);
    assert_eq!(points_of(&done).len(), 2);
    assert_eq!(done.kind, "pen");
    assert_eq!(done.options, options);
    assert!(shape_from_points(&ARROW, &long, &options, true).is_none());
}

#[test]
fn a_point_is_taken_only_when_the_pointer_really_moved() {
    use pigoune_core::annotations::next_stroke_point;
    let points = [point(10.0, 10.0)];
    assert!(!next_stroke_point(&points, point(10.4, 10.3)));
    assert!(next_stroke_point(&points, point(11.0, 10.0)));
    assert!(next_stroke_point(&[], point(0.0, 0.0)));
}

#[test]
fn a_stroke_is_hit_near_any_of_its_segments_and_a_wide_one_is_easier() {
    let shape = stroke(&[point(0.0, 0.0), point(100.0, 0.0), point(100.0, 100.0)]);

    assert!(hit(&shape, point(50.0, 0.0), 2.0));
    assert!(hit(&shape, point(100.0, 60.0), 2.0));
    assert!(hit(&shape, point(50.0, 3.5), 2.0));
    assert!(!hit(&shape, point(50.0, 40.0), 2.0));
    assert!(!hit(&shape, point(30.0, 60.0), 2.0));
    let wide = shape.with_option("width", number_to_value(40.0));
    assert!(hit(&wide, point(50.0, 18.0), 2.0));
    assert!(!hit(&stroke(&[]), point(0.0, 0.0), 5.0));
    assert!(hit(&stroke(&[point(5.0, 5.0)]), point(6.0, 5.0), 2.0));
}

#[test]
fn drawing_with_the_pen_adds_one_selected_stroke_in_one_undo() {
    let (mut interaction, mut session) = fresh();

    interaction.press(
        &mut session,
        Tool::Draw(&PEN),
        &Options::defaults(PEN.options),
        point(10.0, 10.0),
        REACH,
    );
    interaction.drag(&mut session, point(50.0, 30.0));
    interaction.drag(&mut session, point(90.0, 20.0));
    assert!(interaction.pending().is_some());
    assert_eq!(session.count(), 0);
    interaction.release(&mut session, point(120.0, 60.0));

    assert_eq!(session.count(), 1);
    let id = session.selected().expect("the stroke is selected");
    assert_eq!(session.shape_of(id).expect("shape").kind, "pen");
    assert!(session.undo());
    assert_eq!(session.count(), 0);
}

#[test]
fn a_long_scribble_is_stored_with_far_fewer_points() {
    let (mut interaction, mut session) = fresh();

    scribble(&mut interaction, &mut session, Tool::Draw(&PEN));

    let shape = session.selected_shape().expect("a stroke").clone();
    let points = points_of(&shape);
    assert!(points.len() >= 3);
    assert!(points.len() < 41);
    assert_eq!(points[0], point(10.0, 10.0));
    assert_eq!(points[points.len() - 1], point(140.0, 40.0));
}

#[test]
fn a_click_with_the_pen_or_the_highlighter_adds_nothing() {
    let (mut interaction, mut session) = fresh();
    for tool in [&PEN, &HIGHLIGHTER] {
        interaction.press(
            &mut session,
            Tool::Draw(tool),
            &Options::defaults(tool.options),
            point(5.0, 5.0),
            REACH,
        );
        interaction.drag(&mut session, point(6.0, 5.5));
        interaction.release(&mut session, point(5.5, 5.0));
    }
    assert_eq!(session.count(), 0);
    assert!(!session.can_undo());
}

#[test]
fn the_highlighter_is_a_wide_translucent_yellow_stroke_by_default() {
    let (mut interaction, mut session) = fresh();
    interaction.press(
        &mut session,
        Tool::Draw(&HIGHLIGHTER),
        &Options::defaults(HIGHLIGHTER.options),
        point(0.0, 0.0),
        REACH,
    );
    interaction.release(&mut session, point(200.0, 4.0));

    let shape = session.selected_shape().expect("a stroke");

    assert_eq!(shape.kind, "highlighter");
    assert!(same(shape.number("width"), 24.0));
    assert!(same(shape.number("opacity"), 0.4));
    let color = shape.color("color");
    assert_eq!((color.r, color.g, color.b), (0xf6, 0xd3, 0x2d));
    assert!(shape.toggle("smooth"));
    assert!(shape.stroke_color().a < 130);
}

#[test]
fn the_pen_defaults_are_a_red_opaque_smooth_line() {
    let shape = stroke(&[point(0.0, 0.0), point(9.0, 9.0)]);
    let plain = Shape::new("pen", shape.geometry.clone(), Options::new());
    assert!(plain.toggle("smooth"));
    assert!(same(plain.number("opacity"), 1.0));
    assert_eq!(plain.color("color").r, 0xe0);
    let DefaultValue::Toggle(smooth) = PEN.spec("smooth").expect("an option").default else {
        panic!("a toggle");
    };
    assert!(smooth);
    assert!(value_to_color(&PEN.spec("color").expect("an option").default_value()).is_some());
}

#[test]
fn a_drawing_tool_keeps_options_given_for_the_stroke() {
    let (mut interaction, mut session) = fresh();
    let options = Options::defaults(PEN.options)
        .with("width", number_to_value(18.0))
        .with("smooth", json!(false));
    interaction.press(
        &mut session,
        Tool::Draw(&PEN),
        &options,
        point(0.0, 0.0),
        REACH,
    );
    interaction.drag(&mut session, point(40.0, 10.0));
    interaction.release(&mut session, point(80.0, 0.0));

    let shape = session.selected_shape().expect("a stroke");
    assert!(same(shape.number("width"), 18.0));
    assert!(!shape.toggle("smooth"));
}

#[test]
fn a_stroke_can_be_selected_and_moved_as_a_whole() {
    let (mut interaction, mut session) = fresh();
    let id = session.add(stroke(&[
        point(100.0, 100.0),
        point(200.0, 100.0),
        point(250.0, 150.0),
    ]));
    session.select(None);
    let before = points_of(session.shape_of(id).expect("shape"));

    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        point(150.0, 100.0),
        REACH,
    );
    interaction.drag(&mut session, point(180.0, 130.0));
    interaction.release(&mut session, point(190.0, 140.0));

    assert_eq!(session.selected(), Some(id));
    let after = points_of(session.shape_of(id).expect("shape"));
    for (moved, original) in after.iter().zip(&before) {
        assert!(same(moved.x - original.x, 40.0) && same(moved.y - original.y, 40.0));
    }
    assert!(session.undo());
    assert_eq!(points_of(session.shape_of(id).expect("shape")), before);
}

#[test]
fn changing_an_option_of_a_stroke_goes_through_the_history() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(stroke(&[point(0.0, 0.0), point(60.0, 30.0)]));

    assert!(session.set_option_on_selected("opacity", json!(0.5)));
    assert!(same(
        session.shape_of(id).expect("shape").number("opacity"),
        0.5
    ));
    assert!(session.undo());
    assert!(same(
        session.shape_of(id).expect("shape").number("opacity"),
        1.0
    ));
}
