const EDITOR_BLUEPRINT: &str = include_str!("../ui/editor.blp");

#[test]
fn no_message_takes_a_place_in_the_layout_of_the_canvas() {
    assert!(
        !EDITOR_BLUEPRINT.contains("Adw.Banner"),
        "a banner pushes the canvas, a floating notice does not"
    );
    let notice = EDITOR_BLUEPRINT
        .find("Revealer mask_notice")
        .expect("the notice of hidden effects exists");
    let before: Vec<&str> = EDITOR_BLUEPRINT[..notice].lines().rev().take(2).collect();
    assert!(
        before.iter().any(|line| line.trim() == "[overlay]"),
        "the notice is an overlay of the canvas"
    );
}

#[test]
fn the_notice_of_hidden_effects_is_announced_as_an_alert() {
    let notice = EDITOR_BLUEPRINT
        .find("Revealer mask_notice")
        .expect("the notice exists");
    assert!(EDITOR_BLUEPRINT[notice..].contains("accessible-role: alert;"));
}
