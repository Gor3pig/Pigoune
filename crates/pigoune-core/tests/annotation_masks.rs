use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, Geometry, Options, RECTANGLE, Rect, Shape,
};

fn region(kind: &str) -> Shape {
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 40.0,
        }),
        Options::new(),
    )
}

fn drawn() -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        }),
        Options::defaults(RECTANGLE.options),
    )
}

#[test]
fn a_document_without_masks_hides_nothing() {
    let document = AnnotationDocument::new(None, vec![AnnotationObject::new(1, drawn())]);

    assert!(document.exposed_masks().is_empty());
}

#[test]
fn the_blur_and_the_pixelation_are_the_privacy_masks() {
    let document = AnnotationDocument::new(
        None,
        vec![
            AnnotationObject::new(1, region("blur")),
            AnnotationObject::new(2, drawn()),
            AnnotationObject::new(5, region("pixelate")),
        ],
    );

    assert_eq!(document.exposed_masks(), Vec::<u32>::new());
}

#[test]
fn only_a_hidden_privacy_mask_is_exposed() {
    let mut document = AnnotationDocument::new(
        None,
        vec![
            AnnotationObject::new(1, region("blur")),
            AnnotationObject::new(2, drawn()),
            AnnotationObject::new(5, region("pixelate")),
        ],
    );
    assert!(document.exposed_masks().is_empty());
    document.objects[0].state.hidden = true;
    document.objects[1].state.hidden = true;
    assert_eq!(document.exposed_masks(), vec![1]);
}
