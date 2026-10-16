use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, Crop, DocumentError, Geometry, MAX_DOCUMENT_BYTES,
    ObjectContent, Options, Point, Rect, Shape, definitions, number_to_value,
};
use serde_json::{Value, json};

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn rect() -> Rect {
    Rect {
        x: 10.25,
        y: 20.5,
        width: 300.0,
        height: 120.125,
    }
}

fn every_tool() -> Vec<Shape> {
    definitions()
        .iter()
        .map(|tool| {
            let geometry = match tool.geometry {
                pigoune_core::annotations::GeometryKind::TwoPoints => Geometry::TwoPoints {
                    from: point(0.1, 2.0),
                    to: point(1234.5678, 9.0),
                },
                pigoune_core::annotations::GeometryKind::Box => Geometry::Box(rect()),
                pigoune_core::annotations::GeometryKind::Polyline => {
                    Geometry::Polyline(vec![point(1.0, 1.0), point(2.0, 3.0), point(5.0, 8.0)])
                }
                pigoune_core::annotations::GeometryKind::Anchor => {
                    Geometry::Anchor(point(5.0, 6.0))
                }
            };
            Shape::new(tool.kind, geometry, Options::defaults(tool.options))
        })
        .collect()
}

fn full_document() -> AnnotationDocument {
    let objects = every_tool()
        .into_iter()
        .zip(1..)
        .map(|(shape, id)| AnnotationObject::new(id, shape))
        .collect();
    AnnotationDocument::new(
        Some(Crop {
            x: 5,
            y: 6,
            width: 700,
            height: 400,
        }),
        objects,
    )
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).expect("valid json")
}

#[test]
fn a_document_with_every_tool_survives_a_round_trip() {
    let document = full_document();

    let text = document.to_json().expect("written");
    let read = AnnotationDocument::from_json(&text).expect("read");

    assert_eq!(read, document);
    assert_eq!(read.to_json().expect("written again"), text);
}

#[test]
fn the_options_of_an_object_are_written_with_it() {
    let shape = Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: point(0.0, 0.0),
            to: point(1.0, 1.0),
        },
        Options::new()
            .with("width", number_to_value(9.5))
            .with("head", json!("double")),
    );
    let document = AnnotationDocument::new(None, vec![AnnotationObject::new(4, shape)]);

    let written = parsed(&document.to_json().expect("written"));

    assert_eq!(written["objects"][0]["kind"], "arrow");
    assert_eq!(written["objects"][0]["id"], 4);
    assert_eq!(written["objects"][0]["options"]["head"], "double");
    assert_eq!(written["objects"][0]["options"]["width"], 9.5);
    assert_eq!(written["objects"][0]["to"]["x"], 1.0);
}

#[test]
fn the_layer_order_is_the_order_of_the_list() {
    let document = full_document();
    let text = document.to_json().expect("written");
    let read = AnnotationDocument::from_json(&text).expect("read");

    let ids: Vec<u32> = read.objects.iter().map(|object| object.id).collect();
    let count = u32::try_from(definitions().len()).expect("a few tools");
    assert_eq!(ids, (1..=count).collect::<Vec<u32>>());
}

#[test]
fn an_empty_document_has_no_crop_and_no_objects() {
    let text = AnnotationDocument::default().to_json().expect("written");

    assert_eq!(parsed(&text), json!({"version": 1, "objects": []}));
    let read = AnnotationDocument::from_json(&text).expect("read");
    assert_eq!(read.crop, None);
    assert!(read.objects.is_empty());
    assert_eq!(read.next_object_id(), 1);
}

#[test]
fn an_unknown_tool_is_kept_untouched() {
    let stored = json!({
        "version": 1,
        "objects": [
            {"id": 1, "kind": "lasso", "points": [[1, 2], [3, 4]], "style": {"glow": true}},
            {"id": 2, "kind": "blur", "bounds": {"x": 0.0, "y": 0.0, "width": 5.0, "height": 5.0},
             "options": {"radius": 3.0}}
        ]
    });

    let document = AnnotationDocument::from_json(&stored.to_string()).expect("read");
    let rewritten = parsed(&document.to_json().expect("written"));

    assert_eq!(rewritten, stored);
    assert!(matches!(
        document.objects[0].content,
        ObjectContent::Unknown(_)
    ));
    assert!(document.objects[0].shape().is_none());
    assert!(document.objects[1].shape().is_some());
}

#[test]
fn a_tool_that_used_to_exist_but_is_not_registered_is_kept_like_an_unknown_one() {
    let stored = json!({"version": 1, "objects": [
        {"id": 1, "kind": "stamp", "at": {"x": 1.0, "y": 2.0}, "options": {"label": "Hi"}}
    ]});

    let document = AnnotationDocument::from_json(&stored.to_string()).expect("read");

    assert_eq!(parsed(&document.to_json().expect("written")), stored);
}

#[test]
fn a_field_added_to_a_known_tool_by_a_newer_version_is_kept() {
    let stored = json!({
        "version": 1,
        "objects": [{
            "id": 7, "kind": "rectangle",
            "bounds": {"x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0},
            "options": {"width": 2.0},
            "shadow": {"blur": 4, "dx": 1}
        }]
    });

    let document = AnnotationDocument::from_json(&stored.to_string()).expect("read");

    assert_eq!(parsed(&document.to_json().expect("written")), stored);
}

#[test]
fn a_field_added_to_the_document_by_a_newer_version_is_kept() {
    let stored = json!({"version": 1, "objects": [], "frame": {"margin": 24}});

    let document = AnnotationDocument::from_json(&stored.to_string()).expect("read");

    assert_eq!(parsed(&document.to_json().expect("written")), stored);
}

#[test]
fn a_document_from_a_newer_version_is_refused_clearly() {
    let result = AnnotationDocument::from_json(r#"{"version": 2, "objects": []}"#);

    assert_eq!(
        result,
        Err(DocumentError::TooNew {
            found: 2,
            supported: 1
        })
    );
}

#[test]
fn a_damaged_document_is_refused() {
    let object = |fields: &str| format!(r#"{{"version":1,"objects":[{fields}]}}"#);
    let damaged = [
        String::new(),
        "not json".to_owned(),
        "[]".to_owned(),
        r#"{"objects":[]}"#.to_owned(),
        r#"{"version":"one","objects":[]}"#.to_owned(),
        r#"{"version":1}"#.to_owned(),
        r#"{"version":1,"objects":{}}"#.to_owned(),
        r#"{"version":1,"objects":[],"crop":{"x":1}}"#.to_owned(),
        object("3"),
        object(r#"{"kind":"blur"}"#),
        object(r#"{"id":1}"#),
        object(r#"{"id":-1,"kind":"lasso"}"#),
        object(r#"{"id":1,"kind":"rectangle"}"#),
        object(r#"{"id":1,"kind":"rectangle","bounds":{"x":0,"y":0}}"#),
        object(r#"{"id":1,"kind":"arrow","from":{"x":0,"y":0}}"#),
        object(
            r#"{"id":1,"kind":"rectangle","bounds":{"x":0,"y":0,"width":1,"height":1},"options":3}"#,
        ),
        object(r#"{"id":1,"kind":"lasso"},{"id":1,"kind":"lasso"}"#),
    ];

    for text in damaged {
        let result = AnnotationDocument::from_json(&text);
        assert!(
            matches!(result, Err(DocumentError::Corrupt(_))),
            "{text}: {result:?}"
        );
    }
}

#[test]
fn an_enormous_document_is_refused_before_being_read() {
    let text = format!(
        r#"{{"version":1,"objects":[],"padding":"{}"}}"#,
        "x".repeat(MAX_DOCUMENT_BYTES)
    );

    assert_eq!(
        AnnotationDocument::from_json(&text),
        Err(DocumentError::TooLarge)
    );
}

#[test]
fn only_the_tools_that_hide_content_make_a_document_hide_content() {
    let with = |shape: Shape| AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)]);

    for shape in every_tool() {
        let hides = matches!(shape.kind.as_str(), "blur" | "pixelate");
        assert_eq!(with(shape).hides_content(), hides);
    }
    assert!(!AnnotationDocument::default().hides_content());
    assert!(full_document().hides_content());
    let unknown =
        AnnotationDocument::from_json(r#"{"version":1,"objects":[{"id":1,"kind":"mosaic"}]}"#)
            .expect("read");
    assert!(!unknown.hides_content());
}

#[test]
fn numbers_that_are_not_finite_are_refused_when_writing() {
    for number in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let shape = Shape::new(
            "line",
            Geometry::TwoPoints {
                from: point(number, 0.0),
                to: point(1.0, 1.0),
            },
            Options::new(),
        );
        let document = AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)]);

        assert!(
            matches!(document.to_json(), Err(DocumentError::Corrupt(_))),
            "{number}"
        );
    }
}

#[test]
fn the_next_object_id_follows_the_highest_one() {
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[{"id":4,"kind":"x"},{"id":9,"kind":"y"},{"id":2,"kind":"z"}]}"#,
    )
    .expect("read");

    assert_eq!(document.next_object_id(), 10);
}
