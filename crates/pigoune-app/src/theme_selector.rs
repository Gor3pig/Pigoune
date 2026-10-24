use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gettextrs::gettext;

use crate::settings;
use crate::window::PigouneWindow;

const ACTION: &str = "app.color-scheme";
const MENU_SLOT: &str = "theme-selector";

const CHOICES: [&str; 3] = ["system", "light", "dark"];

#[must_use]
pub fn scheme_of(value: &str) -> adw::ColorScheme {
    match value {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    }
}

pub fn follow(application: &adw::Application, settings: &gtk::gio::Settings) {
    apply(settings);
    settings.connect_changed(Some(settings::COLOR_SCHEME), |settings, _| apply(settings));
    application.add_action(&settings.create_action(settings::COLOR_SCHEME));
}

fn apply(settings: &gtk::gio::Settings) {
    adw::StyleManager::default()
        .set_color_scheme(scheme_of(settings.string(settings::COLOR_SCHEME).as_str()));
}

fn label_of(choice: &str) -> String {
    match choice {
        "light" => gettext("Light Theme"),
        "dark" => gettext("Dark Theme"),
        _ => gettext("Follow the System"),
    }
}

fn selector() -> gtk::Box {
    let row = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .spacing(14)
        .margin_top(6)
        .margin_bottom(6)
        .css_classes(["theme-selector"])
        .build();
    for choice in CHOICES {
        let label = label_of(choice);
        let swatch = gtk::ToggleButton::builder()
            .action_name(ACTION)
            .action_target(&choice.to_variant())
            .tooltip_text(&label)
            .css_classes(["theme-swatch", choice])
            .child(&gtk::Image::from_icon_name("object-select-symbolic"))
            .build();
        swatch.update_property(&[gtk::accessible::Property::Label(&label)]);
        row.append(&swatch);
    }
    row
}

impl PigouneWindow {
    pub(crate) fn install_theme_selectors(&self) {
        let imp = self.imp();
        for button in [&imp.library_menu_button, &imp.welcome_menu_button] {
            if let Some(menu) = button.popover().and_downcast::<gtk::PopoverMenu>() {
                menu.add_child(&selector(), MENU_SLOT);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_choice_gives_the_matching_color_scheme() {
        assert_eq!(scheme_of("system"), adw::ColorScheme::Default);
        assert_eq!(scheme_of("light"), adw::ColorScheme::ForceLight);
        assert_eq!(scheme_of("dark"), adw::ColorScheme::ForceDark);
    }

    #[test]
    fn an_unknown_choice_follows_the_system() {
        assert_eq!(scheme_of(""), adw::ColorScheme::Default);
        assert_eq!(scheme_of("sepia"), adw::ColorScheme::Default);
    }

    #[test]
    fn each_choice_has_its_own_label() {
        let labels: std::collections::HashSet<String> =
            CHOICES.iter().map(|choice| label_of(choice)).collect();
        assert_eq!(labels.len(), CHOICES.len());
    }
}
