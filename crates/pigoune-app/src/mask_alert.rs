use adw::prelude::*;
use gettextrs::gettext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskChoice {
    ShowMasks,
    Continue,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Copy,
    Export,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Effects(usize),
    Images(Vec<String>),
}

impl Scope {
    fn several_images(&self) -> bool {
        matches!(self, Self::Images(names) if names.len() > 1)
    }
}

pub fn banner_text(count: usize) -> String {
    let sentence = gettext("Some information may be visible.");
    let state = if count == 1 {
        gettext("A blur or pixelation effect is hidden.")
    } else {
        gettext("{count} blur or pixelation effects are hidden.")
            .replace("{count}", &count.to_string())
    };
    format!("{state} {sentence}")
}

fn state_text(scope: &Scope) -> String {
    match scope {
        Scope::Effects(1) => gettext("A blur or pixelation effect is currently hidden."),
        Scope::Effects(count) => {
            gettext("{count} blur or pixelation effects are currently hidden.")
                .replace("{count}", &count.to_string())
        }
        Scope::Images(names) if names.len() == 1 => {
            gettext("“{name}” contains hidden blur or pixelation effects.")
                .replace("{name}", &names[0])
        }
        Scope::Images(names) => {
            gettext("{count} images contain hidden blur or pixelation effects.")
                .replace("{count}", &names.len().to_string())
        }
    }
}

fn consequence_text(operation: Operation, several: bool) -> String {
    match (operation, several) {
        (Operation::Copy, false) => {
            gettext("The copied image could reveal information you meant to hide.")
        }
        (Operation::Copy, true) => {
            gettext("The copied images could reveal information you meant to hide.")
        }
        (Operation::Export, false) => {
            gettext("The exported image could reveal information you meant to hide.")
        }
        (Operation::Export, true) => {
            gettext("The exported images could reveal information you meant to hide.")
        }
        (Operation::Open, false) => {
            gettext("The opened image could reveal information you meant to hide.")
        }
        (Operation::Open, true) => {
            gettext("The opened images could reveal information you meant to hide.")
        }
    }
}

pub fn body_text(scope: &Scope, operation: Operation) -> String {
    format!(
        "{} {}",
        state_text(scope),
        consequence_text(operation, scope.several_images())
    )
}

fn anyway_label(operation: Operation) -> String {
    match operation {
        Operation::Copy => gettext("_Copy Anyway"),
        Operation::Export => gettext("_Export Anyway"),
        Operation::Open => gettext("_Open Anyway"),
    }
}

pub async fn ask(
    parent: &impl IsA<gtk::Widget>,
    scope: &Scope,
    operation: Operation,
    can_show_them: bool,
) -> MaskChoice {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Some Information May Be Visible")),
        Some(&body_text(scope, operation)),
    );
    dialog.add_response("cancel", &gettext("_Cancel"));
    dialog.add_response("anyway", &anyway_label(operation));
    dialog.set_response_appearance("anyway", adw::ResponseAppearance::Destructive);
    if can_show_them {
        dialog.add_response("show", &gettext("_Show Hidden Effects"));
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

    fn names(count: usize) -> Vec<String> {
        (1..=count)
            .map(|number| format!("Image {number}"))
            .collect()
    }

    #[test]
    fn the_text_names_one_image_and_counts_several() {
        let one = body_text(&Scope::Images(names(1)), Operation::Copy);
        assert!(one.contains("Image 1"));
        let several = body_text(&Scope::Images(names(3)), Operation::Copy);
        assert!(several.contains('3') && !several.contains("Image 1"));
    }

    #[test]
    fn the_text_counts_the_effects_of_the_editor() {
        assert!(!body_text(&Scope::Effects(1), Operation::Export).contains("{count}"));
        assert!(body_text(&Scope::Effects(4), Operation::Export).contains('4'));
    }

    #[test]
    fn the_text_follows_the_operation_and_the_number_of_images() {
        let copy = body_text(&Scope::Images(names(1)), Operation::Copy);
        let export = body_text(&Scope::Images(names(1)), Operation::Export);
        let open = body_text(&Scope::Images(names(1)), Operation::Open);
        assert!(copy.contains("copied") && export.contains("exported") && open.contains("opened"));
        let several = body_text(&Scope::Images(names(2)), Operation::Export);
        assert!(several.contains("exported images"));
    }

    #[test]
    fn the_banner_counts_the_hidden_effects() {
        assert!(!banner_text(1).contains("{count}"));
        assert!(banner_text(2).contains('2'));
    }
}
