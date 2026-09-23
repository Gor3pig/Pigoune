use adw::prelude::*;

const APPLICATION_ID: &str = "io.github.Gor3pig.Pigoune.Devel";

fn main() -> gtk::glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    application.connect_activate(build_ui);
    application.run()
}

fn build_ui(application: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Pigoune")
        .build();

    window.present();
}
