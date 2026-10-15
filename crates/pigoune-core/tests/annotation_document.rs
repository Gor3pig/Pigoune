use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, Color, Crop, DocumentError, Font, MAX_DOCUMENT_BYTES,
    ObjectContent, Point, Rect, Shape, Stroke,
};
use serde_json::{Value, json};

fn color(r: u8) -> Color {
    Color {
        r,
        g: 20,
        b: 30,
        a: 255,
    }
}

fn stroke() -> Stroke {
    Stroke {
        color: color(200),
        width: 4.5,
    }
}

fn rect() -> Rect {
    Rect {
        x: 10.25,
        y: 20.5,
        width: 300.0,
        height: 120.125,
    }
}

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn every_shape() -> Vec<Shape> {
    vec![
        Shape::Rectangle {
            bounds: rect(),
            stroke: stroke(),
            fill: Some(color(1)),
        },
        Shape::Ellipse {
            bounds: rect(),
            stroke: stroke(),
            fill: None,
        },
        Shape::Line {
            from: point(0.1, 2.0),
            to: point(1234.5678, 9.0),
            stroke: stroke(),
        },
        Shape::Arrow {
            from: point(1.0, 2.0),
            to: point(3.0, 4.0),
            stroke: stroke(),
        },
        Shape::Text {
            origin: point(5.0, 6.0),
            content: "Étape \"1\"\nligne 2 \\ 🙂 <b>".to_owned(),
            font: Font {
                family: "Sans".to_owned(),
                size: 24.0,
                bold: true,
                italic: false,
            },
            color: color(3),
        },
        Shape::Step {
            center: point(7.0, 8.0),
            number: 3,
            radius: 18.0,
            color: color(4),
        },
        Shape::Pen {
            points: vec![point(1.0, 1.0), point(2.0, 3.0), point(5.0, 8.0)],
            stroke: stroke(),
        },
        Shape::Highlighter {
            points: vec![point(1.0, 1.0), point(9.0, 1.0)],
            stroke: stroke(),
        },
        Shape::Blur {
            bounds: rect(),
            radius: 12.0,
        },
        Shape::Pixelate {
            bounds: rect(),
            cell: 10,
        },
    ]
}

fn full_document() -> AnnotationDocument {
    let objects = every_shape()
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
fn the_layer_order_is_the_order_of_the_list() {
    let document = full_document();
    let text = document.to_json().expect("written");
    let read = AnnotationDocument::from_json(&text).expect("read");

    let ids: Vec<u32> = read.objects.iter().map(|object| object.id).collect();
    assert_eq!(ids, (1..=10).collect::<Vec<u32>>());
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
            {"id": 2, "kind": "blur", "bounds": {"x": 0.0, "y": 0.0, "width": 5.0, "height": 5.0}, "radius": 3.0}
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
fn a_field_added_to_a_known_tool_by_a_newer_version_is_kept() {
    let stored = json!({
        "version": 1,
        "objects": [{
            "id": 7, "kind": "rectangle",
            "bounds": {"x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0},
            "stroke": {"color": {"r": 1, "g": 2, "b": 3, "a": 255}, "width": 2.0},
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
        object(r#"{"id":1,"kind":"blur","bounds":{"x":0,"y":0,"width":1,"height":1}}"#),
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
fn only_blur_and_pixelation_hide_content() {
    let with = |shape: Shape| AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)]);

    for shape in every_shape() {
        let hides = matches!(shape, Shape::Blur { .. } | Shape::Pixelate { .. });
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
        let document = AnnotationDocument::new(
            None,
            vec![AnnotationObject::new(
                1,
                Shape::Line {
                    from: point(number, 0.0),
                    to: point(1.0, 1.0),
                    stroke: stroke(),
                },
            )],
        );

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

#[test]
fn the_text_of_an_annotation_keeps_every_character() {
    let content = "ligne\ttab \u{0}nul \u{202e}rtl \"guillemets\" 日本語 \u{1F642}";
    let document = AnnotationDocument::new(
        None,
        vec![AnnotationObject::new(
            1,
            Shape::Text {
                origin: point(0.0, 0.0),
                content: content.to_owned(),
                font: Font {
                    family: "Noto Sans CJK".to_owned(),
                    size: 12.0,
                    bold: false,
                    italic: true,
                },
                color: color(0),
            },
        )],
    );

    let read = AnnotationDocument::from_json(&document.to_json().expect("written")).expect("read");

    let Some(Shape::Text { content: kept, .. }) = read.objects[0].shape() else {
        panic!("the text is a text");
    };
    assert_eq!(kept, content);
}
