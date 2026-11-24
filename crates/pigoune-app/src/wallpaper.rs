use std::fs::File;
use std::path::Path;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::portal_request::{Listener, PORTAL_NAME, PORTAL_PATH};

const WALLPAPER_INTERFACE: &str = "org.freedesktop.portal.Wallpaper";
const SET_ON_BOTH: &str = "both";
const RESPONSE_APPLIED: u32 = 0;
const RESPONSE_CANCELLED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperResponse {
    Applied,
    Cancelled,
    Failed,
}

pub async fn set_wallpaper(
    file: &Path,
    on_response: impl Fn(WallpaperResponse) + 'static,
) -> Result<(), String> {
    let image = File::open(file).map_err(|error| error.to_string())?;
    let descriptors = gio::UnixFDList::new();
    let index = descriptors
        .append(&image)
        .map_err(|error| error.to_string())?;
    let connection = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|error| error.to_string())?;
    let listener = Listener::start(&connection, move |parameters| {
        on_response(match response_code(parameters) {
            Some(RESPONSE_APPLIED) => WallpaperResponse::Applied,
            Some(RESPONSE_CANCELLED) => WallpaperResponse::Cancelled,
            _ => WallpaperResponse::Failed,
        });
    })
    .ok_or_else(|| "no connection name".to_owned())?;
    let called = connection
        .call_with_unix_fd_list_future(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            WALLPAPER_INTERFACE,
            "SetWallpaperFile",
            Some(&wallpaper_arguments(index, listener.token())),
            Some(glib::VariantTy::new("(o)").map_err(|error| error.to_string())?),
            gio::DBusCallFlags::NONE,
            -1,
            Some(&descriptors),
        )
        .await;
    let request = called
        .map_err(|error| error.to_string())
        .and_then(|(reply, _)| {
            reply
                .child_value(0)
                .str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "no request".to_owned())
        });
    match request {
        Ok(handle) => {
            listener.follow_returned(&handle);
            Ok(())
        }
        Err(error) => {
            listener.close();
            Err(error)
        }
    }
}

fn wallpaper_arguments(descriptor: i32, token: &str) -> glib::Variant {
    let options = glib::VariantDict::new(None);
    options.insert("show-preview", true);
    options.insert("set-on", SET_ON_BOTH);
    options.insert("handle_token", token);
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
        assert_eq!(
            wallpaper_arguments(0, "token").type_().as_str(),
            "(sha{sv})"
        );
    }
}
