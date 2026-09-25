use adw::prelude::*;

mod application;
mod i18n;
mod window;

const APPLICATION_ID: &str = "io.github.Gor3pig.Pigoune.Devel";

fn main() -> gtk::glib::ExitCode {
    // SAFETY: No GTK objects or worker threads have been created yet.
    unsafe { i18n::init() }.expect("Failed to initialize gettext");

    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_activate(window::build);
    application.run()
}
