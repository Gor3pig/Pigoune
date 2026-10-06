use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const WALLPAPER_INTERFACE: &str = "org.freedesktop.portal.Wallpaper";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const SET_ON_BOTH: &str = "both";
const RESPONSE_FAILED: u32 = 2;

pub async fn set_wallpaper(file: &Path, on_failure: impl Fn() + 'static) -> Result<(), String> {
    let image = File::open(file).map_err(|error| error.to_string())?;
    let descriptors = gio::UnixFDList::new();
    let index = descriptors
        .append(&image)
        .map_err(|error| error.to_string())?;
    let connection = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|error| error.to_string())?;
    let (reply, _) = connection
        .call_with_unix_fd_list_future(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            WALLPAPER_INTERFACE,
            "SetWallpaperFile",
            Some(&wallpaper_arguments(index)),
            Some(glib::VariantTy::new("(o)").map_err(|error| error.to_string())?),
            gio::DBusCallFlags::NONE,
            -1,
            Some(&descriptors),
        )
        .await
        .map_err(|error| error.to_string())?;
    let request = reply
        .child_value(0)
        .str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "no request".to_owned())?;
    let holder: Rc<RefCell<Option<gio::SignalSubscription>>> = Rc::default();
    let held = Rc::clone(&holder);
    let subscription = connection.subscribe_to_signal(
        Some(PORTAL_NAME),
        Some(REQUEST_INTERFACE),
        Some("Response"),
        Some(&request),
        None,
        gio::DBusSignalFlags::NO_MATCH_RULE,
        move |signal| {
            if response_code(signal.parameters) == Some(RESPONSE_FAILED) {
                on_failure();
            }
            let held = Rc::clone(&held);
            glib::idle_add_local_once(move || {
                held.take();
            });
        },
    );
    holder.replace(Some(subscription));
    Ok(())
}

fn wallpaper_arguments(descriptor: i32) -> glib::Variant {
    let options = glib::VariantDict::new(None);
    options.insert("show-preview", true);
    options.insert("set-on", SET_ON_BOTH);
    glib::Variant::tuple_from_iter([
        "".to_variant(),
        glib::variant::Handle(descriptor).to_variant(),
        options.end(),
    ])
}

fn response_code(parameters: &glib::Variant) -> Option<u32> {
    parameters.try_child_value(0)?.get::<u32>()
}

#[cfg(test)]
mod tests {
    use super::wallpaper_arguments;

    #[test]
    fn the_wallpaper_request_matches_the_portal() {
        assert_eq!(wallpaper_arguments(0).type_().as_str(), "(sha{sv})");
    }
}
