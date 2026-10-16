use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, EditSession, Geometry, Options, Rect, Shape,
};

fn document() -> AnnotationDocument {
    let shape = Shape::new(
        "blur",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        Options::new(),
    );
    AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)])
}

#[test]
fn a_new_session_has_nothing_to_save() {
    assert!(!EditSession::new(document()).is_modified());
}

#[test]
fn a_restored_session_keeps_its_document_and_still_has_to_be_saved() {
    let mut session = EditSession::restored(document());

    assert_eq!(session.document(), &document());
    assert!(session.is_modified());
    assert!(!session.can_undo());
    session.mark_saved();
    assert!(!session.is_modified());
}
