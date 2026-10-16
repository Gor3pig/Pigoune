use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENSHOT_INTERFACE: &str = "org.freedesktop.portal.Screenshot";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const RESPONSE_TAKEN: u32 = 0;
const RESPONSE_CANCELLED: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotOutcome {
    Taken(PathBuf),
    Cancelled,
    Failed,
}

pub async fn take() -> ScreenshotOutcome {
    let (sender, receiver) = async_channel::bounded(1);
    if request(move |outcome| {
        let _ = sender.try_send(outcome);
    })
    .await
    .is_err()
    {
        return ScreenshotOutcome::Failed;
    }
    receiver.recv().await.unwrap_or(ScreenshotOutcome::Failed)
}

async fn request(on_response: impl Fn(ScreenshotOutcome) + 'static) -> Result<(), String> {
    let connection = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|error| error.to_string())?;
    let reply = connection
        .call_future(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            SCREENSHOT_INTERFACE,
            "Screenshot",
            Some(&screenshot_arguments()),
            Some(glib::VariantTy::new("(o)").map_err(|error| error.to_string())?),
            gio::DBusCallFlags::NONE,
            -1,
        )
        .await
        .map_err(|error| error.to_string())?;
    let handle = reply
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
        Some(&handle),
        None,
        gio::DBusSignalFlags::NO_MATCH_RULE,
        move |signal| {
            on_response(outcome_of(signal.parameters));
            let held = Rc::clone(&held);
            glib::idle_add_local_once(move || {
                held.take();
            });
        },
    );
    holder.replace(Some(subscription));
    Ok(())
}

fn screenshot_arguments() -> glib::Variant {
    let options = glib::VariantDict::new(None);
    options.insert("interactive", true);
    options.insert("modal", true);
    glib::Variant::tuple_from_iter(["".to_variant(), options.end()])
}

fn outcome_of(parameters: &glib::Variant) -> ScreenshotOutcome {
    let code = parameters
        .try_child_value(0)
        .and_then(|code| code.get::<u32>());
    let uri = parameters
        .try_child_value(1)
        .and_then(|results| {
            glib::VariantDict::new(Some(&results))
                .lookup::<String>("uri")
                .ok()
        })
        .flatten();
    outcome_from(code, uri.as_deref())
}

fn outcome_from(code: Option<u32>, uri: Option<&str>) -> ScreenshotOutcome {
    match (code, uri) {
        (Some(RESPONSE_TAKEN), Some(uri)) => gio::File::for_uri(uri)
            .path()
            .map_or(ScreenshotOutcome::Failed, ScreenshotOutcome::Taken),
        (Some(RESPONSE_CANCELLED), _) => ScreenshotOutcome::Cancelled,
        _ => ScreenshotOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gtk::glib;
    use gtk::prelude::*;

    use super::{ScreenshotOutcome, outcome_from, outcome_of, screenshot_arguments};

    #[test]
    fn the_request_matches_the_portal() {
        assert_eq!(screenshot_arguments().type_().as_str(), "(sa{sv})");
    }

    #[test]
    fn a_taken_screenshot_gives_the_path_of_its_file() {
        assert_eq!(
            outcome_from(Some(0), Some("file:///srv/shots/Screenshot%20one.png")),
            ScreenshotOutcome::Taken(PathBuf::from("/srv/shots/Screenshot one.png"))
        );
    }

    #[test]
    fn a_cancelled_request_is_not_a_failure() {
        assert_eq!(outcome_from(Some(1), None), ScreenshotOutcome::Cancelled);
    }

    #[test]
    fn everything_else_is_a_failure() {
        assert_eq!(outcome_from(Some(0), None), ScreenshotOutcome::Failed);
        assert_eq!(outcome_from(Some(2), None), ScreenshotOutcome::Failed);
        assert_eq!(outcome_from(None, None), ScreenshotOutcome::Failed);
        assert_eq!(
            outcome_from(Some(0), Some("https://example.org/a.png")),
            ScreenshotOutcome::Failed
        );
    }

    #[test]
    fn the_response_of_the_portal_is_read_from_its_variant() {
        let results = glib::VariantDict::new(None);
        results.insert("uri", "file:///tmp/shot.png");
        let signal = glib::Variant::tuple_from_iter([0_u32.to_variant(), results.end()]);

        assert_eq!(
            outcome_of(&signal),
            ScreenshotOutcome::Taken(PathBuf::from("/tmp/shot.png"))
        );
    }
}
