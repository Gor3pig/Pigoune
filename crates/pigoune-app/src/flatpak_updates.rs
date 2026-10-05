use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::update_news::{UpdateNews, news_from_commits, news_from_progress};

const PORTAL_NAME: &str = "org.freedesktop.portal.Flatpak";
const PORTAL_PATH: &str = "/org/freedesktop/portal/Flatpak";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Flatpak";
const MONITOR_INTERFACE: &str = "org.freedesktop.portal.Flatpak.UpdateMonitor";
const SANDBOX_INFO: &str = "/.flatpak-info";
const LAUNCH_COMMAND: &[u8] = b"pigoune\0";
const STARTING_FOLDER: &[u8] = b"/\0";
const SPAWN_LATEST_VERSION: u32 = 2;

#[derive(Debug)]
pub struct FlatpakUpdates {
    connection: gio::DBusConnection,
    monitor_path: String,
    _subscriptions: Vec<gio::SignalSubscription>,
}

impl FlatpakUpdates {
    pub async fn watch(on_news: impl Fn(UpdateNews) + 'static) -> Option<Self> {
        if !Path::new(SANDBOX_INFO).exists() {
            return None;
        }
        let connection = gio::bus_get_future(gio::BusType::Session).await.ok()?;
        let reply = connection
            .call_future(
                Some(PORTAL_NAME),
                PORTAL_PATH,
                PORTAL_INTERFACE,
                "CreateUpdateMonitor",
                Some(&monitor_arguments()),
                Some(glib::VariantTy::new("(o)").ok()?),
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await
            .ok()?;
        let monitor_path = reply.child_value(0).str()?.to_owned();
        let on_news = Rc::new(on_news);
        let on_available = Rc::clone(&on_news);
        let available = connection.subscribe_to_signal(
            Some(PORTAL_NAME),
            Some(MONITOR_INTERFACE),
            Some("UpdateAvailable"),
            Some(&monitor_path),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if let Some(news) = news_from_update_info(signal.parameters) {
                    on_available(news);
                }
            },
        );
        let progress = connection.subscribe_to_signal(
            Some(PORTAL_NAME),
            Some(MONITOR_INTERFACE),
            Some("Progress"),
            Some(&monitor_path),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if let Some(news) = news_from_progress_info(signal.parameters) {
                    on_news(news);
                }
            },
        );
        Some(Self {
            connection,
            monitor_path,
            _subscriptions: vec![available, progress],
        })
    }

    pub async fn install(&self) -> Result<(), glib::Error> {
        self.connection
            .call_future(
                Some(PORTAL_NAME),
                &self.monitor_path,
                MONITOR_INTERFACE,
                "Update",
                Some(&update_arguments()),
                None,
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await
            .map(|_| ())
    }

    pub async fn launch_latest_version(&self) -> Result<(), glib::Error> {
        let arguments = spawn_arguments();
        self.connection
            .call_future(
                Some(PORTAL_NAME),
                PORTAL_PATH,
                PORTAL_INTERFACE,
                "Spawn",
                Some(&arguments),
                None,
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await
            .map(|_| ())
    }
}

fn empty_options() -> glib::Variant {
    glib::VariantDict::new(None).end()
}

fn monitor_arguments() -> glib::Variant {
    glib::Variant::tuple_from_iter([empty_options()])
}

fn update_arguments() -> glib::Variant {
    glib::Variant::tuple_from_iter(["".to_variant(), empty_options()])
}

fn spawn_arguments() -> glib::Variant {
    glib::Variant::tuple_from_iter([
        STARTING_FOLDER.to_vec().to_variant(),
        vec![LAUNCH_COMMAND.to_vec()].to_variant(),
        HashMap::<u32, glib::variant::Handle>::new().to_variant(),
        HashMap::<String, String>::new().to_variant(),
        SPAWN_LATEST_VERSION.to_variant(),
        empty_options(),
    ])
}

fn news_from_update_info(parameters: &glib::Variant) -> Option<UpdateNews> {
    let info = glib::VariantDict::new(Some(&parameters.try_child_value(0)?));
    let commit = |key: &str| {
        info.lookup::<String>(key)
            .ok()
            .flatten()
            .unwrap_or_default()
    };
    news_from_commits(
        &commit("running-commit"),
        &commit("local-commit"),
        &commit("remote-commit"),
    )
}

fn news_from_progress_info(parameters: &glib::Variant) -> Option<UpdateNews> {
    let info = glib::VariantDict::new(Some(&parameters.try_child_value(0)?));
    let number = |key: &str| info.lookup::<u32>(key).ok().flatten().unwrap_or_default();
    news_from_progress(number("status"), number("progress"))
}

#[cfg(test)]
mod tests {
    use super::{monitor_arguments, spawn_arguments, update_arguments};

    #[test]
    fn the_monitor_request_sends_options() {
        assert_eq!(monitor_arguments().type_().as_str(), "(a{sv})");
    }

    #[test]
    fn the_update_request_sends_a_window_and_options() {
        assert_eq!(update_arguments().type_().as_str(), "(sa{sv})");
    }

    #[test]
    fn the_restart_request_matches_the_portal() {
        assert_eq!(
            spawn_arguments().type_().as_str(),
            "(ayaaya{uh}a{ss}ua{sv})"
        );
    }
}
