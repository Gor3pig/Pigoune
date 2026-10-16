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
fn a_document_without_masks_lists_none() {
    let document = AnnotationDocument::new(None, vec![AnnotationObject::new(1, drawn())]);

    assert!(document.masks().is_empty());
    assert!(!document.hides_content());
}

#[test]
fn the_blur_and_the_pixelation_are_the_masks_in_layer_order() {
    let document = AnnotationDocument::new(
        None,
        vec![
            AnnotationObject::new(1, region("blur")),
            AnnotationObject::new(2, drawn()),
            AnnotationObject::new(5, region("pixelate")),
        ],
    );

    let masks = document.masks();

    assert_eq!(masks.len(), 2);
    assert_eq!((masks[0].0, masks[0].1.kind.as_str()), (1, "blur"));
    assert_eq!((masks[1].0, masks[1].1.kind.as_str()), (5, "pixelate"));
}

#[test]
fn a_moved_mask_is_no_longer_the_mask_that_was_listed() {
    let before = AnnotationDocument::new(None, vec![AnnotationObject::new(1, region("blur"))]);
    let moved = region("blur").with_geometry(Geometry::Box(Rect {
        x: 300.0,
        y: 20.0,
        width: 100.0,
        height: 40.0,
    }));
    let after = AnnotationDocument::new(None, vec![AnnotationObject::new(1, moved)]);

    assert_ne!(before.masks(), after.masks());
    assert_eq!(before.masks(), before.masks());
}
