use pigoune_core::annotations::{
    ARROW, CURVE, Geometry, Options, Point, SHAFT, Shape, arrow_curve_control, curve_control,
    curve_point, curve_polyline, hit, shape_bounds,
};
use serde_json::json;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn arrow(options: Options) -> Shape {
    Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: point(0.0, 0.0),
            to: point(200.0, 0.0),
        },
        options,
    )
}

#[test]
fn an_arrow_is_straight_and_tapered_by_default() {
    let plain = arrow(Options::new());

    assert!(near(plain.number("curve"), 0.0));
    assert_eq!(plain.choice("shaft"), "tapered");
    assert_eq!(arrow_curve_control(&plain), None);
}

#[test]
fn the_curve_bends_the_arrow_to_one_side_or_the_other() {
    let down = curve_control(point(0.0, 0.0), point(200.0, 0.0), 50.0).expect("bent");
    let up = curve_control(point(0.0, 0.0), point(200.0, 0.0), -50.0).expect("bent");

    assert!(near(down.x, 100.0) && near(up.x, 100.0));
    assert!(near(down.y, 50.0));
    assert!(near(up.y, -50.0));
    assert_eq!(curve_control(point(0.0, 0.0), point(200.0, 0.0), 0.2), None);
    assert_eq!(curve_control(point(5.0, 5.0), point(5.0, 5.0), 80.0), None);
}

#[test]
fn only_arrows_have_a_curve() {
    let line = Shape::new(
        "line",
        Geometry::TwoPoints {
            from: point(0.0, 0.0),
            to: point(200.0, 0.0),
        },
        Options::new().with("curve", json!(80.0)),
    );

    assert_eq!(arrow_curve_control(&line), None);
}

#[test]
fn the_curve_passes_through_both_ends_and_its_middle_is_halfway_to_the_control() {
    let (from, control, to) = (point(0.0, 0.0), point(100.0, 50.0), point(200.0, 0.0));

    let middle = curve_point(from, control, to, 0.5);

    assert!(near(middle.x, 100.0) && near(middle.y, 25.0));
    let line = curve_polyline(from, control, to);
    assert_eq!(line.first().copied(), Some(from));
    let last = line.last().expect("points");
    assert!(near(last.x, 200.0) && near(last.y, 0.0));
}

#[test]
fn a_click_follows_the_curve_and_not_the_chord() {
    let bent = arrow(Options::new().with("curve", json!(80.0)));

    assert!(hit(&bent, point(100.0, 40.0), 6.0));
    assert!(!hit(&bent, point(100.0, 0.0), 6.0));
    let straight = arrow(Options::new());
    assert!(hit(&straight, point(100.0, 0.0), 6.0));
}

#[test]
fn the_bounds_of_a_curved_arrow_hold_the_whole_curve() {
    let bent = arrow(Options::new().with("curve", json!(80.0)));

    let bounds = shape_bounds(&bent).expect("bounds");

    assert!(bounds.height > 30.0);
    assert!(near(bounds.width, 200.0));
}

#[test]
fn the_new_options_have_valid_defaults_and_refuse_nonsense() {
    assert!(CURVE.accepts(&json!(-100.0)) && CURVE.accepts(&json!(100.0)));
    assert!(!CURVE.accepts(&json!(120.0)));
    assert!(SHAFT.accepts(&json!("tapered")));
    assert!(!SHAFT.accepts(&json!("wavy")));
    assert!(ARROW.spec("curve").is_some() && ARROW.spec("shaft").is_some());
}
