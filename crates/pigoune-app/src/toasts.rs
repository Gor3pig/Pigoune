use std::cell::RefCell;

use adw::prelude::*;
use gtk::glib;

pub trait ToastHost: IsA<glib::Object> {
    fn add(&self, toast: adw::Toast);
}

impl ToastHost for adw::ToastOverlay {
    fn add(&self, toast: adw::Toast) {
        self.add_toast(toast);
    }
}

impl ToastHost for adw::PreferencesDialog {
    fn add(&self, toast: adw::Toast) {
        self.add_toast(toast);
    }
}

type Latest = (glib::WeakRef<glib::Object>, adw::Toast);

thread_local! {
    static LATEST: RefCell<Vec<Latest>> = const { RefCell::new(Vec::new()) };
}

pub fn announce(overlay: &adw::ToastOverlay, toast: &adw::Toast) {
    announce_in(overlay, toast);
}

pub fn announce_in_preferences(dialog: &adw::PreferencesDialog, toast: &adw::Toast) {
    announce_in(dialog, toast);
}

fn announce_in(host: &impl ToastHost, toast: &adw::Toast) {
    let target: &glib::Object = host.upcast_ref();
    let replaced = LATEST.with_borrow_mut(|latest| {
        latest.retain(|(weak, _)| weak.upgrade().is_some());
        let position = latest
            .iter()
            .position(|(weak, _)| weak.upgrade().as_ref() == Some(target));
        let replaced = position.map(|position| latest.remove(position).1);
        latest.push((target.downgrade(), toast.clone()));
        replaced
    });
    if let Some(previous) = replaced {
        previous.dismiss();
    }
    toast.set_priority(adw::ToastPriority::High);
    host.add(toast.clone());
}
