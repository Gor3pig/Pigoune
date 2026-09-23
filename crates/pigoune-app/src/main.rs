use adw::prelude::*;

mod window;

const APPLICATION_ID: &str = "io.github.Gor3pig.Pigoune.Devel";

fn main() -> gtk::glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_activate(window::build);
    application.run()
}
