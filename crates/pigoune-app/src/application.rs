use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config::{APP_ID, RESOURCE_BASE_PATH, VERSION};
use crate::help_url;
use crate::icon_theme;
use crate::settings;
use crate::window::PigouneWindow;

pub fn build() -> adw::Application {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(RESOURCE_BASE_PATH)
        .build();

    application.connect_startup(|_| icon_theme::keep_gnome_icons());
    application.connect_startup(install_actions);
    application.connect_activate(present_main_window);
    application
}

fn install_actions(application: &adw::Application) {
    let quit = gio::ActionEntry::builder("quit")
        .activate(|application: &adw::Application, _, _| application.quit())
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|application: &adw::Application, _, _| show_about_dialog(application))
        .build();
    let help = gio::ActionEntry::builder("help")
        .activate(|application: &adw::Application, _, _| open_help(application))
        .build();
    application.add_action_entries([quit, about, help]);

    application.set_accels_for_action("app.quit", &["<Control>q"]);
    application.set_accels_for_action("app.help", &["F1"]);
    application.set_accels_for_action("win.create-library", &["<Control>n"]);
    application.set_accels_for_action("win.open-library", &["<Control>o"]);
    application.set_accels_for_action("win.import-files", &["<Control>i"]);
    application.set_accels_for_action("win.toggle-favorite", &["<Control>d"]);
    application.set_accels_for_action("win.undo", &["<Control>z"]);
    application.set_accels_for_action("win.preferences", &["<Control>comma"]);
    application.set_accels_for_action("win.search", &["<Control>f"]);
    application.set_accels_for_action("win.copy-selected", &["<Control>c"]);
    application.set_accels_for_action("win.select-all", &["<Control>a"]);
    application.set_accels_for_action(
        "win.enlarge-thumbnails",
        &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
    );
    application.set_accels_for_action(
        "win.shrink-thumbnails",
        &["<Control>minus", "<Control>KP_Subtract"],
    );
}

fn present_main_window(application: &adw::Application) {
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }

    let window = PigouneWindow::new(application, settings::load());
    window.present();
    window.reopen_last_library();
}

fn localized_help_url() -> String {
    let languages = glib::language_names();
    let languages: Vec<&str> = languages.iter().map(glib::GString::as_str).collect();
    help_url::help_url(&languages)
}

fn open_help(application: &adw::Application) {
    gtk::UriLauncher::new(&localized_help_url()).launch(
        application.active_window().as_ref(),
        gio::Cancellable::NONE,
        |_| {},
    );
}

fn show_about_dialog(application: &adw::Application) {
    let dialog = adw::AboutDialog::from_appdata(
        &format!("{RESOURCE_BASE_PATH}/metainfo.xml"),
        Some(VERSION),
    );
    dialog.set_application_icon(APP_ID);
    dialog.set_developer_name("Gor3pig");
    dialog.set_version(VERSION);
    dialog.set_website("https://gor3pig.github.io/Pigoune/");
    dialog.set_issue_url("https://github.com/Gor3pig/Pigoune/issues");
    dialog.set_support_url(&localized_help_url());
    dialog.set_license_type(gtk::License::Gpl30);
    dialog.set_copyright("© 2026 Gor3pig");
    dialog.set_translator_credits(&gettext("translator-credits"));
    dialog.present(application.active_window().as_ref());
}
