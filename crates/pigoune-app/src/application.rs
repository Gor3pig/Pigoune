use std::ops::ControlFlow;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config::{APP_ID, RESOURCE_BASE_PATH, VERSION};
use crate::first_launch;
use crate::fonts;
use crate::help_url;
use crate::icon_theme;
use crate::release_notes;
use crate::settings;
use crate::shortcuts;
use crate::theme_selector;
use crate::whats_new;
use crate::window::PigouneWindow;

const CAPTURE_OPTION: &str = "capture";
const CAPTURE_ACTION: &str = "capture";

thread_local! {
    static CAPTURE_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn build() -> adw::Application {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(RESOURCE_BASE_PATH)
        .flags(gio::ApplicationFlags::ALLOW_REPLACEMENT)
        .build();

    application.add_main_option(
        CAPTURE_OPTION,
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Take a screenshot to annotate",
        None,
    );
    application.connect_handle_local_options(forward_capture_request);
    application.connect_startup(|_| icon_theme::keep_gnome_icons());
    application.connect_startup(|_| fonts::register());
    application.connect_startup(install_actions);
    application.connect_startup(|application| {
        theme_selector::follow(application, &settings::load());
    });
    application.connect_activate(|application| {
        if CAPTURE_REQUESTED.take() {
            request_capture(application);
        } else {
            present_main_window(application, true);
        }
    });
    application
}

fn forward_capture_request(
    application: &adw::Application,
    options: &glib::VariantDict,
) -> ControlFlow<glib::ExitCode> {
    if !options.contains(CAPTURE_OPTION) {
        return ControlFlow::Continue(());
    }
    if application.register(gio::Cancellable::NONE).is_err() {
        return ControlFlow::Continue(());
    }
    if application.is_remote() {
        application.activate_action(CAPTURE_ACTION, None);
        return ControlFlow::Break(glib::ExitCode::SUCCESS);
    }
    CAPTURE_REQUESTED.set(true);
    ControlFlow::Continue(())
}

fn request_capture(application: &adw::Application) {
    present_main_window(application, false);
    let Some(window) = application.active_window().and_downcast::<PigouneWindow>() else {
        return;
    };
    glib::spawn_future_local(async move { window.capture_screenshot().await });
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
    let capture = gio::ActionEntry::builder(CAPTURE_ACTION)
        .activate(|application: &adw::Application, _, _| request_capture(application))
        .build();
    application.add_action_entries([quit, about, help, capture]);

    shortcuts::apply(application, &shortcuts::stored(&settings::load()));
}

fn present_main_window(application: &adw::Application, reopen_library: bool) {
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }

    let settings = settings::load();
    first_launch::apply(&settings);
    settings.connect_changed(
        Some(settings::SHORTCUTS),
        glib::clone!(
            #[weak]
            application,
            move |settings, _| shortcuts::apply(&application, &shortcuts::stored(settings))
        ),
    );
    let window = PigouneWindow::new(application, settings.clone());
    window.present();
    whats_new::show_after_update(&window, &settings);
    if reopen_library {
        window.reopen_last_library();
    }
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
    dialog.add_legal_section(
        "Phosphor Icons",
        Some("© 2023 Phosphor Icons"),
        gtk::License::MitX11,
        None,
    );
    dialog.set_translator_credits(&gettext("translator-credits"));
    show_release_notes_in_french_or_english(&dialog);
    dialog.present(application.active_window().as_ref());
}

fn show_release_notes_in_french_or_english(dialog: &adw::AboutDialog) {
    let path = format!("{RESOURCE_BASE_PATH}/metainfo.xml");
    let Ok(bytes) = gio::resources_lookup_data(&path, gio::ResourceLookupFlags::NONE) else {
        return;
    };
    let Ok(metainfo) = std::str::from_utf8(&bytes) else {
        return;
    };
    let languages: Vec<String> = glib::language_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    let french = release_notes::is_french(&languages);
    if let Some(notes) = release_notes::release_notes(metainfo, VERSION, french) {
        dialog.set_release_notes(&notes);
        dialog.set_release_notes_version(VERSION);
    }
}
