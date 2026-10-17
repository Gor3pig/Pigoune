use adw::prelude::*;
use gettextrs::gettext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskChoice {
    ShowMasks,
    Continue,
    Cancel,
}

pub fn body_text(names: &[String]) -> String {
    match names {
        [name] => gettext("“{name}” is hidden: the image will show what it covers.")
            .replace("{name}", name),
        _ => gettext("{count} masks are hidden: the image will show what they cover.")
            .replace("{count}", &names.len().to_string()),
    }
}

pub async fn ask(
    parent: &impl IsA<gtk::Widget>,
    names: &[String],
    can_show_them: bool,
) -> MaskChoice {
    let dialog = adw::AlertDialog::new(Some(&gettext("A Mask Is Hidden")), Some(&body_text(names)));
    dialog.add_response("cancel", &gettext("_Cancel"));
    dialog.add_response("anyway", &gettext("Continue _Anyway"));
    dialog.set_response_appearance("anyway", adw::ResponseAppearance::Destructive);
    if can_show_them {
        dialog.add_response("show", &gettext("_Show the Masks"));
        dialog.set_response_appearance("show", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("show"));
    } else {
        dialog.set_default_response(Some("cancel"));
    }
    dialog.set_close_response("cancel");
    match dialog.choose_future(Some(parent)).await.as_str() {
        "anyway" => MaskChoice::Continue,
        "show" => MaskChoice::ShowMasks,
        _ => MaskChoice::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_names_one_mask_and_counts_several() {
        let one = body_text(&["Blur 1".to_owned()]);
        assert!(one.contains("Blur 1"));
        let several = body_text(&["Blur 1".to_owned(), "Pixelate 1".to_owned()]);
        assert!(several.contains('2') && !several.contains("Blur 1"));
    }
}
