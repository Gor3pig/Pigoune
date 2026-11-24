use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};

pub const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const REQUEST_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/request";

type Handler = Rc<dyn Fn(&glib::Variant)>;

pub struct Listener {
    connection: gio::DBusConnection,
    handler: Handler,
    subscriptions: Rc<RefCell<Vec<gio::SignalSubscription>>>,
    token: String,
    expected: String,
}

impl Listener {
    pub fn start(
        connection: &gio::DBusConnection,
        on_response: impl Fn(&glib::Variant) + 'static,
    ) -> Option<Self> {
        let unique_name = connection.unique_name()?;
        let token = new_token();
        let expected = request_path(&unique_name, &token);
        let listener = Self {
            connection: connection.clone(),
            handler: Rc::new(on_response),
            subscriptions: Rc::default(),
            token,
            expected,
        };
        listener.follow(&listener.expected);
        Some(listener)
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn follow_returned(&self, handle: &str) {
        if handle != self.expected {
            self.follow(handle);
        }
    }

    pub fn close(&self) {
        self.subscriptions.borrow_mut().clear();
    }

    fn follow(&self, path: &str) {
        let subscriptions = Rc::clone(&self.subscriptions);
        let handler = Rc::clone(&self.handler);
        let subscription = self.connection.subscribe_to_signal(
            Some(PORTAL_NAME),
            Some(REQUEST_INTERFACE),
            Some("Response"),
            Some(path),
            None,
            gio::DBusSignalFlags::NO_MATCH_RULE,
            move |signal| {
                handler(signal.parameters);
                let subscriptions = Rc::clone(&subscriptions);
                glib::idle_add_local_once(move || subscriptions.borrow_mut().clear());
            },
        );
        self.subscriptions.borrow_mut().push(subscription);
    }
}

pub fn cancel_request(connection: &gio::DBusConnection, handle: &str) {
    connection.call(
        Some(PORTAL_NAME),
        handle,
        REQUEST_INTERFACE,
        "Close",
        None,
        None,
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        |_| {},
    );
}

pub async fn first<T>(receiver: &async_channel::Receiver<T>, timeout: Duration) -> Option<T> {
    glib::future_with_timeout(timeout, receiver.recv())
        .await
        .ok()?
        .ok()
}

fn new_token() -> String {
    format!("pigoune{}", glib::uuid_string_random().replace('-', ""))
}

fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("{REQUEST_PATH_PREFIX}/{sender}/{token}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gtk::glib;

    use super::{first, new_token, request_path};

    #[test]
    fn the_request_path_follows_the_portal_convention() {
        assert_eq!(
            request_path(":1.42", "abc123"),
            "/org/freedesktop/portal/desktop/request/1_42/abc123"
        );
    }

    #[test]
    fn a_token_only_holds_characters_a_bus_path_accepts() {
        let token = new_token();
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        assert_ne!(token, new_token());
    }

    #[test]
    fn a_missing_response_ends_the_wait() {
        let (_sender, receiver) = async_channel::bounded::<u8>(1);
        let waited = glib::MainContext::new()
            .block_on(async { first(&receiver, Duration::from_millis(50)).await });
        assert_eq!(waited, None);
    }

    #[test]
    fn a_response_is_returned_before_the_end_of_the_wait() {
        let (sender, receiver) = async_channel::bounded::<u8>(1);
        sender.try_send(7).expect("sent");
        let waited = glib::MainContext::new()
            .block_on(async { first(&receiver, Duration::from_secs(30)).await });
        assert_eq!(waited, Some(7));
    }
}
