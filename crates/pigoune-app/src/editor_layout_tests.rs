const EDITOR_BLUEPRINT: &str = include_str!("../ui/editor.blp");

#[test]
fn no_message_takes_a_place_in_the_layout_of_the_canvas() {
    assert!(
        !EDITOR_BLUEPRINT.contains("Adw.Banner"),
        "a banner pushes the canvas"
    );
}

#[test]
fn hiding_a_layer_never_brings_a_permanent_warning_back() {
    assert!(
        !EDITOR_BLUEPRINT.contains("mask_notice"),
        "the exit warnings are enough, no permanent notice about hidden effects"
    );
}
