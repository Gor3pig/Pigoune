use adw::prelude::*;
use gettextrs::gettext;
use gtk::gio;

use crate::config::{APP_ID, RESOURCE_BASE_PATH, VERSION};
use crate::settings;
use crate::window::PigouneWindow;

pub fn build() -> adw::Application {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(RESOURCE_BASE_PATH)
        .build();

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
    application.add_action_entries([quit, about]);

    application.set_accels_for_action("app.quit", &["<Control>q"]);
    application.set_accels_for_action("win.create-library", &["<Control>n"]);
    application.set_accels_for_action("win.open-library", &["<Control>o"]);
    application.set_accels_for_action("win.import-files", &["<Control>i"]);
    application.set_accels_for_action("win.toggle-favorite", &["<Control>d"]);
    application.set_accels_for_action("win.undo", &["<Control>z"]);
    application.set_accels_for_action("win.preferences", &["<Control>comma"]);
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

fn show_about_dialog(application: &adw::Application) {
    let dialog = adw::AboutDialog::builder()
        .application_name("Pigoune")
        .application_icon(APP_ID)
        .developer_name("Gor3pig")
        .version(VERSION)
        .website("https://github.com/Gor3pig/Pigoune")
        .issue_url("https://github.com/Gor3pig/Pigoune/issues")
        .license_type(gtk::License::Gpl30)
        .copyright("© 2026 Gor3pig")
        .translator_credits(gettext("translator-credits"))
        .build();
    dialog.present(application.active_window().as_ref());
}
