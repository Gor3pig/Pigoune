use pigoune_core::annotations::{
    Geometry, Options, Point, SEQUENCE, SHADOW, Shape, TEXT_BACKGROUND, TEXT_OUTLINE, definition,
    step_label,
};
use serde_json::json;

fn anchored(kind: &str, options: Options) -> Shape {
    Shape::new(kind, Geometry::Anchor(Point { x: 1.0, y: 2.0 }), options)
}

#[test]
fn the_steps_count_in_numbers_by_default() {
    assert_eq!(step_label(1, "numbers"), "1");
    assert_eq!(step_label(12, "numbers"), "12");
    assert_eq!(step_label(3, "anything else"), "3");
}

#[test]
fn the_letters_run_from_a_to_z_then_aa() {
    assert_eq!(step_label(1, "letters"), "A");
    assert_eq!(step_label(2, "letters"), "B");
    assert_eq!(step_label(26, "letters"), "Z");
    assert_eq!(step_label(27, "letters"), "AA");
    assert_eq!(step_label(28, "letters"), "AB");
    assert_eq!(step_label(52, "letters"), "AZ");
    assert_eq!(step_label(53, "letters"), "BA");
    assert_eq!(step_label(702, "letters"), "ZZ");
    assert_eq!(step_label(703, "letters"), "AAA");
}

#[test]
fn a_rank_of_zero_never_gives_an_empty_label() {
    assert_eq!(step_label(0, "letters"), "A");
}

#[test]
fn a_step_chooses_numbers_or_letters() {
    let step = anchored("step", Options::new());
    assert_eq!(step.choice("sequence"), "numbers");
    let lettered = anchored("step", Options::new().with("sequence", json!("letters")));
    assert_eq!(lettered.choice("sequence"), "letters");
    assert!(SEQUENCE.accepts(&json!("letters")));
    assert!(!SEQUENCE.accepts(&json!("roman")));
}

#[test]
fn a_text_can_have_a_bubble_and_an_outline() {
    assert!(TEXT_BACKGROUND.accepts(&json!("bubble")));
    assert!(TEXT_OUTLINE.accepts(&json!(true)));
    let plain = anchored("text", Options::new());
    assert!(!plain.toggle("outline"));
    let styled = anchored(
        "text",
        Options::new()
            .with("background", json!("bubble"))
            .with("outline", json!(true)),
    );
    assert_eq!(styled.choice("background"), "bubble");
    assert!(styled.toggle("outline"));
}

#[test]
fn the_shadow_is_an_option_of_the_drawn_tools_and_never_of_the_masks() {
    for kind in [
        "rectangle",
        "ellipse",
        "line",
        "arrow",
        "pen",
        "step",
        "text",
    ] {
        let sheet = definition(kind).expect("registered");
        assert!(sheet.spec(SHADOW.id).is_some(), "{kind} has no shadow");
        assert!(!anchored(kind, Options::new()).toggle("shadow"));
        assert!(
            anchored(kind, Options::new().with("shadow", json!(true))).toggle("shadow"),
            "{kind} ignores its shadow"
        );
    }
    for kind in ["blur", "pixelate", "spotlight", "highlighter"] {
        let sheet = definition(kind).expect("registered");
        assert!(sheet.spec(SHADOW.id).is_none(), "{kind} has a shadow");
    }
}
