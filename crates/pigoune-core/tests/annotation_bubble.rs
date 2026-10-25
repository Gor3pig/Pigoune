use pigoune_core::annotations::{
    AnnotationDocument, BUBBLE, EditSession, Geometry, Handle, Options, Point, Rect, Shape,
    bubble_hit, bubble_outline, bubble_parts, bubble_tail, bubble_text_box, definition,
    handles_with_extent, hit, is_set_by_handle, layer_entries, number_to_value, resized,
    rotate_point, shape_from_drag, shape_pivot,
};
use serde_json::json;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn near(left: Point, right: Point) -> bool {
    same(left.x, right.x) && same(left.y, right.y)
}

fn bubble(form: &str, tail: (f32, f32), rotation: f32) -> Shape {
    Shape::new(
        "bubble",
        Geometry::Box(Rect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 100.0,
        }),
        Options::defaults(BUBBLE.options)
            .with("form", json!(form))
            .with("tail_x", number_to_value(tail.0))
            .with("tail_y", number_to_value(tail.1))
            .with("rotation", number_to_value(rotation)),
    )
}

fn centre() -> Point {
    point(200.0, 150.0)
}

#[test]
fn the_bubble_is_a_registered_box_tool_with_a_black_outline_and_a_white_fill() {
    let sheet = definition("bubble").expect("tool");
    assert_eq!(sheet.options.len(), BUBBLE.options.len());
    let shape = bubble("oval", (0.3, 0.8), 0.0);
    assert_eq!(shape.color("color").r, 0x24);
    assert_eq!(
        (
            shape.color("fill_color").r,
            shape.color("fill_color").g,
            shape.color("fill_color").b
        ),
        (255, 255, 255)
    );
    assert_eq!(shape.choice("form"), "oval");
    assert!(sheet.spec("rotation").is_some());
}

#[test]
fn only_the_two_tail_numbers_are_driven_by_the_round_handle() {
    assert!(is_set_by_handle("tail_x") && is_set_by_handle("tail_y"));
    for id in ["color", "fill_color", "width", "form", "rotation", "curve"] {
        assert!(!is_set_by_handle(id));
    }
}

#[test]
fn the_tail_tip_is_placed_relative_to_the_size_of_the_bubble() {
    let tip = bubble_tail(&bubble("oval", (0.3, 0.8), 0.0)).expect("tip");
    assert!(near(tip, point(260.0, 230.0)));
    let tip = bubble_tail(&bubble("oval", (-0.75, -0.75), 0.0)).expect("tip");
    assert!(near(tip, point(50.0, 75.0)));
}

#[test]
fn a_drag_makes_a_bubble_with_the_default_tail() {
    let shape = shape_from_drag(
        &BUBBLE,
        point(10.0, 10.0),
        point(210.0, 110.0),
        &Options::defaults(BUBBLE.options),
    )
    .expect("bubble");
    assert_eq!(shape.kind, "bubble");
    let tip = bubble_tail(&shape).expect("tip");
    assert!(near(tip, point(110.0 + 60.0, 60.0 + 80.0)));
}

#[test]
fn the_outline_reaches_the_tip_for_a_tail_in_every_direction() {
    for form in ["oval", "rounded"] {
        for tail in [
            (0.0, 1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.9, 0.9),
            (-0.9, -0.9),
            (0.3, 0.8),
        ] {
            let shape = bubble(form, tail, 0.0);
            let tip = bubble_tail(&shape).expect("tip");
            let outline = bubble_outline(&shape).expect("outline");
            assert!(
                outline.iter().any(|vertex| near(*vertex, tip)),
                "{form} {tail:?}"
            );
            assert!(outline.len() > 60);
            assert!(outline.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        }
    }
}

#[test]
fn a_tail_inside_the_body_leaves_a_plain_oval() {
    let plain = bubble_outline(&bubble("oval", (0.0, 0.0), 0.0)).expect("outline");
    assert_eq!(plain.len(), 128);
    for vertex in &plain {
        let scaled = ((vertex.x - 200.0) / 100.0).hypot((vertex.y - 150.0) / 50.0);
        assert!(same(scaled, 1.0));
    }
}

#[test]
fn the_body_stays_inside_its_box_and_the_tail_is_the_only_thing_that_leaves_it() {
    for form in ["oval", "rounded"] {
        let outline = bubble_outline(&bubble(form, (0.2, 1.2), 0.0)).expect("outline");
        let tip = point(240.0, 270.0);
        let outside: Vec<&Point> = outline
            .iter()
            .filter(|p| p.x < 99.9 || p.x > 300.1 || p.y < 99.9 || p.y > 200.1)
            .collect();
        assert!(!outside.is_empty(), "{form}");
        assert!(outside.iter().all(|p| p.y > 200.0 && p.y <= tip.y + 0.01));
    }
}

#[test]
fn a_rounded_bubble_cuts_its_corners_and_an_oval_does_not_fill_them() {
    for form in ["oval", "rounded"] {
        let shape = bubble(form, (0.0, 0.0), 0.0);
        assert!(bubble_hit(&shape, centre(), 0.0));
        assert!(!bubble_hit(&shape, point(101.0, 101.0), 0.0), "{form}");
    }
    let rounded = bubble("rounded", (0.0, 0.0), 0.0);
    assert!(bubble_hit(&rounded, point(110.0, 150.0), 0.0));
    assert!(bubble_hit(&rounded, point(200.0, 101.0), 0.0));
}

#[test]
fn the_bubble_is_hit_on_its_body_on_its_tail_and_near_its_outline_only() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);
    assert!(hit(&shape, centre(), 4.0));
    assert!(hit(&shape, point(255.0, 222.0), 2.0));
    assert!(hit(&shape, point(260.0, 229.0), 2.0));
    assert!(!hit(&shape, point(400.0, 150.0), 4.0));
    assert!(!hit(&shape, point(120.0, 230.0), 4.0));
    assert!(hit(&shape, point(302.0, 150.0), 4.0));
}

#[test]
fn the_round_tail_handle_sits_on_the_tip_and_turns_with_the_bubble() {
    let flat = bubble("oval", (0.3, 0.8), 0.0);
    let found = handles_with_extent(&flat, None);
    let tail = found
        .iter()
        .find(|(handle, _)| *handle == Handle::Tail)
        .expect("tail handle")
        .1;
    assert!(near(tail, point(260.0, 230.0)));
    assert_eq!(found.len(), 9);

    let turned = bubble("oval", (0.3, 0.8), 90.0);
    let tail = handles_with_extent(&turned, None)
        .into_iter()
        .find(|(handle, _)| *handle == Handle::Tail)
        .expect("tail handle")
        .1;
    assert!(near(
        tail,
        rotate_point(point(260.0, 230.0), centre(), 90.0)
    ));
}

#[test]
fn only_a_bubble_has_a_tail_handle() {
    let rectangle = Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }),
        Options::defaults(definition("rectangle").expect("tool").options),
    );
    assert!(
        handles_with_extent(&rectangle, None)
            .iter()
            .all(|(handle, _)| *handle != Handle::Tail)
    );
}

#[test]
fn dragging_the_tail_handle_moves_the_tip_and_leaves_the_box_alone() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);

    let moved = resized(&shape, Handle::Tail, point(40.0, 20.0));

    assert!(near(bubble_tail(&moved).expect("tip"), point(40.0, 20.0)));
    assert_eq!(moved.geometry, shape.geometry);
}

#[test]
fn dragging_the_tail_handle_of_a_turned_bubble_follows_the_turned_axes() {
    let shape = bubble("oval", (0.3, 0.8), 90.0);
    let wanted = rotate_point(point(150.0, 260.0), centre(), 90.0);

    let moved = resized(&shape, Handle::Tail, wanted);

    assert!(near(bubble_tail(&moved).expect("tip"), point(150.0, 260.0)));
    assert!(same(moved.number("rotation"), 90.0));
    assert!(near(shape_pivot(&moved, None).expect("pivot"), centre()));
}

#[test]
fn the_tail_cannot_be_dragged_further_than_the_stored_range() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);

    let moved = resized(&shape, Handle::Tail, point(100_000.0, -100_000.0));

    assert!(same(moved.number("tail_x"), 4.0));
    assert!(same(moved.number("tail_y"), -3.0));
}

#[test]
fn resizing_the_box_keeps_the_tail_at_the_same_place_in_the_bubble() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);

    let wider = resized(&shape, Handle::Right, point(500.0, 150.0));

    assert!(same(wider.number("tail_x"), 0.3) && same(wider.number("tail_y"), 0.8));
    let tip = bubble_tail(&wider).expect("tip");
    assert!(near(tip, point(300.0 + 0.3 * 400.0, 150.0 + 0.8 * 100.0)));
}

#[test]
fn an_empty_box_has_no_outline_and_is_never_hit() {
    let flat = Shape::new(
        "bubble",
        Geometry::Box(Rect {
            x: 10.0,
            y: 10.0,
            width: 0.0,
            height: 30.0,
        }),
        Options::defaults(BUBBLE.options),
    );
    assert!(bubble_outline(&flat).is_none());
    assert!(!bubble_hit(&flat, point(10.0, 20.0), 5.0));
}

fn middle_of(part: &[Point]) -> Point {
    let count = f32::from(u16::try_from(part.len()).expect("small"));
    point(
        part.iter().map(|p| p.x).sum::<f32>() / count,
        part.iter().map(|p| p.y).sum::<f32>() / count,
    )
}

fn width_of(part: &[Point]) -> f32 {
    let left = part.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let right = part.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    right - left
}

#[test]
fn a_cloud_bubble_is_made_of_bumps_that_stay_inside_its_box() {
    let parts = bubble_parts(&bubble("cloud", (0.0, 0.0), 0.0)).expect("parts");
    assert_eq!(parts.len(), 1);
    let body = &parts[0];
    assert!(body.len() >= 80);
    assert!(
        body.iter()
            .all(|p| p.x >= 99.5 && p.x <= 300.5 && p.y >= 99.5 && p.y <= 200.5)
    );
    let reach = |p: &Point| ((p.x - 200.0) / 100.0).hypot((p.y - 150.0) / 50.0);
    let nearest = body.iter().map(reach).fold(f32::INFINITY, f32::min);
    let farthest = body.iter().map(reach).fold(f32::NEG_INFINITY, f32::max);
    assert!(farthest - nearest > 0.1, "bumps, not a plain oval");
    assert!(farthest > 0.95);
}

#[test]
fn a_cloud_bubble_ends_its_tail_in_three_shrinking_circles_that_reach_the_tip() {
    let shape = bubble("cloud", (0.3, 0.8), 0.0);
    let tip = bubble_tail(&shape).expect("tip");

    let parts = bubble_parts(&shape).expect("parts");

    assert_eq!(parts.len(), 4);
    let circles = &parts[1..];
    assert!(width_of(&circles[0]) > width_of(&circles[1]));
    assert!(width_of(&circles[1]) > width_of(&circles[2]));
    assert!(
        circles[2]
            .iter()
            .any(|p| (p.x - tip.x).hypot(p.y - tip.y) < 3.0)
    );
    let reach = |part: &Vec<Point>| {
        let middle = middle_of(part);
        (middle.x - 200.0).hypot(middle.y - 150.0)
    };
    assert!(reach(&circles[0]) < reach(&circles[1]) && reach(&circles[1]) < reach(&circles[2]));
}

#[test]
fn the_circles_of_a_cloud_tail_do_not_overlap_the_body_nor_each_other() {
    for tail in [(0.3, 0.8), (-0.9, -0.9), (1.0, 0.0), (0.0, -1.2)] {
        let parts = bubble_parts(&bubble("cloud", tail, 0.0)).expect("parts");
        assert_eq!(parts.len(), 4, "{tail:?}");
        for circle in &parts[1..] {
            for vertex in circle {
                let scaled = ((vertex.x - 200.0) / 100.0).hypot((vertex.y - 150.0) / 50.0);
                assert!(scaled > 0.8, "{tail:?}");
            }
        }
        for pair in parts[1..].windows(2) {
            let (first, second) = (middle_of(&pair[0]), middle_of(&pair[1]));
            let radii = f32::midpoint(width_of(&pair[0]), width_of(&pair[1]));
            assert!(
                (first.x - second.x).hypot(first.y - second.y) > radii,
                "{tail:?}"
            );
        }
    }
}

#[test]
fn a_cloud_bubble_with_its_tip_inside_has_no_circles() {
    let parts = bubble_parts(&bubble("cloud", (0.1, 0.1), 0.0)).expect("parts");
    assert_eq!(parts.len(), 1);
}

#[test]
fn the_outline_of_a_cloud_bubble_is_its_body_and_the_circles_are_hit_too() {
    let shape = bubble("cloud", (0.3, 0.8), 0.0);
    let parts = bubble_parts(&shape).expect("parts");
    assert_eq!(bubble_outline(&shape).as_ref(), parts.first());

    assert!(hit(&shape, centre(), 4.0));
    assert!(hit(&shape, middle_of(&parts[1]), 2.0));
    assert!(hit(&shape, middle_of(&parts[3]), 2.0));
    assert!(!hit(&shape, point(400.0, 150.0), 4.0));
}

#[test]
fn a_turned_cloud_bubble_is_hit_where_its_circles_are_seen() {
    let shape = bubble("cloud", (0.3, 0.8), 90.0);
    let last = middle_of(&bubble_parts(&bubble("cloud", (0.3, 0.8), 0.0)).expect("parts")[3]);

    assert!(hit(&shape, rotate_point(last, centre(), 90.0), 2.0));
    assert!(!hit(&shape, last, 2.0));
}

fn corners(frame: Rect) -> [Point; 4] {
    [
        point(frame.x, frame.y),
        point(frame.x + frame.width, frame.y),
        point(frame.x, frame.y + frame.height),
        point(frame.x + frame.width, frame.y + frame.height),
    ]
}

#[test]
fn the_text_box_is_centred_and_its_corners_stay_inside_the_body_of_every_form() {
    for form in ["oval", "rounded", "cloud"] {
        let shape = bubble(form, (0.0, 0.0), 0.0);
        let frame = bubble_text_box(&shape).expect("text box");
        assert!(same(frame.x + frame.width / 2.0, 200.0), "{form}");
        assert!(same(frame.y + frame.height / 2.0, 150.0), "{form}");
        assert!(frame.width > 100.0 && frame.height > 40.0, "{form}");
        for corner in corners(frame) {
            assert!(bubble_hit(&shape, corner, 0.0), "{form}");
        }
    }
}

#[test]
fn the_text_box_follows_the_box_of_the_bubble_when_it_is_resized() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);
    let wider = resized(&shape, Handle::Right, point(500.0, 150.0));

    let (before, after) = (
        bubble_text_box(&shape).expect("box"),
        bubble_text_box(&wider).expect("box"),
    );

    assert!(after.width > before.width * 1.9);
    assert!(same(after.height, before.height));
}

#[test]
fn a_double_click_finds_the_text_of_a_bubble_and_not_the_one_of_a_rectangle() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(bubble("oval", (0.3, 0.8), 0.0));
    let plain = session.add(Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 400.0,
            y: 100.0,
            width: 100.0,
            height: 50.0,
        }),
        Options::defaults(definition("rectangle").expect("tool").options),
    ));

    assert_eq!(session.text_at(centre(), 4.0), Some(id));
    assert_eq!(session.text_at(point(260.0, 229.0), 4.0), Some(id));
    assert_eq!(session.text_at(point(450.0, 125.0), 4.0), None);
    assert!(session.shape_of(plain).is_some());
}

#[test]
fn the_layer_of_a_bubble_summarises_what_it_says() {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(
        bubble("oval", (0.3, 0.8), 0.0).with_option("content", json!("Hello there\nsecond line")),
    );
    session.add(bubble("cloud", (0.3, 0.8), 0.0));

    let entries = layer_entries(session.document());

    assert_eq!(entries[1].summary, "Hello there");
    assert_eq!(entries[0].summary, "");
}

#[test]
fn the_text_options_of_a_bubble_default_to_the_ones_of_the_text_tool() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);
    let text = Shape::new(
        "text",
        Geometry::Anchor(point(0.0, 0.0)),
        Options::defaults(definition("text").expect("tool").options),
    );
    assert!(same(shape.number("font_size"), text.number("font_size")));
    assert!(!shape.toggle("bold") && !shape.toggle("italic"));
    assert_eq!(shape.text("content"), "");
}

#[test]
fn a_bubble_speaks_in_cartoon_lettering_unless_told_otherwise() {
    let shape = bubble("oval", (0.3, 0.8), 0.0);
    assert_eq!(shape.choice("lettering"), "cartoon");
    let plain = shape.with_option("lettering", json!("normal"));
    assert_eq!(plain.choice("lettering"), "normal");
    let sheet = definition("bubble").expect("tool");
    let spec = sheet.spec("lettering").expect("lettering");
    assert!(spec.accepts(&json!("cartoon")) && spec.accepts(&json!("normal")));
    assert!(!spec.accepts(&json!("gothic")));
}
