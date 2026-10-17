use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, DIMMING, Geometry, Options, Point, Rect, SPOT_SHAPE,
    SPOTLIGHT, Shape, definition, hit,
};
use serde_json::json;

fn spotlight(options: Options) -> Shape {
    Shape::new(
        "spotlight",
        Geometry::Box(Rect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 120.0,
        }),
        options,
    )
}

#[test]
fn a_spotlight_is_a_registered_box_tool_that_hides_nothing() {
    let registered = definition("spotlight").expect("registered");

    assert_eq!(registered.kind, SPOTLIGHT.kind);
    assert!(!registered.hides_content);
    let mut document = AnnotationDocument::new(
        None,
        vec![AnnotationObject::new(1, spotlight(Options::new()))],
    );
    document.objects[0].state.hidden = true;
    assert!(document.exposed_masks().is_empty());
}

#[test]
fn the_defaults_are_a_rectangle_dimming_most_of_the_image() {
    let plain = spotlight(Options::new());

    assert_eq!(plain.choice("spot"), "rectangle");
    assert!((plain.number("dimming") - 0.6).abs() < 1e-3);
}

#[test]
fn only_a_known_shape_and_a_dimming_between_zero_and_one_are_valid() {
    assert!(SPOT_SHAPE.accepts(&json!("ellipse")));
    assert!(!SPOT_SHAPE.accepts(&json!("star")));
    assert!(DIMMING.accepts(&json!(0.0)));
    assert!(DIMMING.accepts(&json!(1.0)));
    assert!(!DIMMING.accepts(&json!(1.5)));
    let broken = spotlight(Options::new().with("spot", json!("star")));
    assert_eq!(broken.choice("spot"), "rectangle");
}

#[test]
fn a_click_inside_the_lit_area_selects_the_spotlight() {
    let shape = spotlight(Options::new());

    assert!(hit(&shape, Point { x: 200.0, y: 160.0 }, 6.0));
    assert!(!hit(&shape, Point { x: 20.0, y: 20.0 }, 6.0));
}

#[test]
fn the_options_survive_a_trip_through_the_document() {
    let shape = spotlight(
        Options::new()
            .with("spot", json!("ellipse"))
            .with("dimming", json!(0.25)),
    );
    let document = AnnotationDocument::new(None, vec![AnnotationObject::new(7, shape)]);

    let back = AnnotationDocument::from_json(&document.to_json().expect("json")).expect("read");

    let object = back.objects[0].shape().expect("known tool");
    assert_eq!(object.choice("spot"), "ellipse");
    assert!((object.number("dimming") - 0.25).abs() < 1e-3);
}
