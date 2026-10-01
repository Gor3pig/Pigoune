mod config;
mod window;

use adw::prelude::*;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::{gio, glib};

use crate::config::{APP_ID, GETTEXT_PACKAGE, LOCALEDIR, RESOURCE_BASE_PATH};
use crate::window::PigouneWindow;

fn main() -> glib::ExitCode {
    setup_translations();

    gio::resources_register_include!("pigoune.gresource")
        .expect("compiled resources are embedded in the binary");

    let application = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(RESOURCE_BASE_PATH)
        .build();

    application.connect_activate(present_main_window);
    application.run()
}

fn present_main_window(application: &adw::Application) {
    let window = application
        .active_window()
        .unwrap_or_else(|| PigouneWindow::new(application).upcast());
    window.present();
}

fn setup_translations() {
    let result = bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR)
        .and_then(|_| bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8"))
        .and_then(|_| textdomain(GETTEXT_PACKAGE));
    if let Err(error) = result {
        glib::g_warning!("pigoune", "Translations are unavailable: {error}");
    }
}
