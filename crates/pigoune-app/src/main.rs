mod config;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::config::{APP_ID, RESOURCE_BASE_PATH};
use crate::window::PigouneWindow;

fn main() -> glib::ExitCode {
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
