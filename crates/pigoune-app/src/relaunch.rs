use std::process::Command;

use adw::prelude::*;
use gtk::glib;

use crate::flatpak_updates;
use crate::window::PigouneWindow;

const REPLACE_OPTION: &str = "--gapplication-replace";

pub fn relaunch(application: &adw::Application) {
    let application = application.clone();
    glib::spawn_future_local(async move {
        if let Some(window) = application.active_window().and_downcast::<PigouneWindow>()
            && !window.confirm_leaving().await
        {
            return;
        }
        let started = if flatpak_updates::is_sandboxed() {
            flatpak_updates::relaunch_in_sandbox()
                .await
                .map_err(|error| error.to_string())
        } else {
            start_again()
        };
        match started {
            Ok(()) => application.quit(),
            Err(error) => glib::g_warning!("pigoune", "Unable to restart Pigoune: {error}"),
        }
    });
}

fn start_again() -> Result<(), String> {
    let program = std::env::current_exe().map_err(|error| error.to_string())?;
    Command::new(program)
        .arg(REPLACE_OPTION)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}
