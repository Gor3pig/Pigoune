use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::shortcuts::{self, CUSTOMIZABLE, Customizable, Overrides, Refusal};

struct ShortcutRow {
    customizable: &'static Customizable,
    shown: adw::ShortcutLabel,
    reset: gtk::Button,
}

pub fn populate(
    dialog: &adw::PreferencesDialog,
    group: &adw::PreferencesGroup,
    restore: &adw::ButtonRow,
    settings: &gio::Settings,
) {
    let rows: Rc<Vec<ShortcutRow>> = Rc::new(
        CUSTOMIZABLE
            .iter()
            .map(|customizable| add_row(dialog, group, customizable, settings))
            .collect(),
    );
    show(&rows, &shortcuts::stored(settings));
    let followed = settings.connect_changed(
        Some(crate::settings::SHORTCUTS),
        glib::clone!(
            #[strong]
            rows,
            move |settings, _| show(&rows, &shortcuts::stored(settings))
        ),
    );
    let followed = Cell::new(Some(followed));
    dialog.connect_closed(glib::clone!(
        #[strong]
        settings,
        move |_| {
            if let Some(followed) = followed.take() {
                settings.disconnect(followed);
            }
        }
    ));
    restore.connect_activated(glib::clone!(
        #[strong]
        settings,
        move |_| shortcuts::store(&settings, &Overrides::new())
    ));
}

fn add_row(
    dialog: &adw::PreferencesDialog,
    group: &adw::PreferencesGroup,
    customizable: &'static Customizable,
    settings: &gio::Settings,
) -> ShortcutRow {
    let row = adw::ActionRow::builder()
        .title(shortcuts::label(customizable.action))
        .activatable(true)
        .build();
    let shown = adw::ShortcutLabel::new("");
    shown.set_valign(gtk::Align::Center);
    let reset = gtk::Button::builder()
        .icon_name("edit-undo-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(gettext("Reset to Default"))
        .css_classes(["flat"])
        .build();
    row.add_suffix(&shown);
    row.add_suffix(&reset);
    row.connect_activated(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        settings,
        move |_| capture(&dialog, customizable, &settings)
    ));
    reset.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        settings,
        move |_| restore_default(&dialog, customizable, &settings)
    ));
    group.add(&row);
    ShortcutRow {
        customizable,
        shown,
        reset,
    }
}

fn restore_default(
    dialog: &adw::PreferencesDialog,
    customizable: &Customizable,
    settings: &gio::Settings,
) {
    let mut overrides = shortcuts::stored(settings);
    overrides.remove(customizable.action);
    for default in customizable.defaults {
        if let Err(Refusal::Taken(other)) =
            shortcuts::validate(customizable.action, default, &overrides)
        {
            let reason = describe(&Refusal::Taken(other), default);
            dialog.add_toast(adw::Toast::new(&reason));
            return;
        }
    }
    shortcuts::store(settings, &overrides);
}

fn show(rows: &[ShortcutRow], overrides: &Overrides) {
    for row in rows {
        let accelerators = shortcuts::usable_accelerators(row.customizable, overrides);
        row.shown
            .set_accelerator(accelerators.first().map_or("", String::as_str));
        row.reset
            .set_visible(overrides.contains_key(row.customizable.action));
    }
}

fn capture(
    dialog: &adw::PreferencesDialog,
    customizable: &'static Customizable,
    settings: &gio::Settings,
) {
    let name = shortcuts::label(customizable.action);
    let prompt = gettext("Press the new key combination for “{name}”").replace("{name}", &name);
    let alert = adw::AlertDialog::new(Some(&gettext("Set the Shortcut")), Some(&prompt));
    alert.add_response("cancel", &gettext("_Cancel"));
    alert.set_close_response("cancel");
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        alert,
        #[strong]
        settings,
        #[upgrade_or]
        glib::Propagation::Stop,
        move |_, key, _, state| {
            let held = state & gtk::accelerator_get_default_mod_mask();
            if key == gdk::Key::Escape && held.is_empty() {
                alert.close();
                return glib::Propagation::Stop;
            }
            let Some(accelerator) = shortcuts::from_key_press(key, state) else {
                return glib::Propagation::Stop;
            };
            let mut overrides = shortcuts::stored(&settings);
            match shortcuts::validate(customizable.action, &accelerator, &overrides) {
                Ok(chosen) => {
                    choose(customizable, &chosen, &mut overrides);
                    shortcuts::store(&settings, &overrides);
                    alert.close();
                }
                Err(refusal) => {
                    let reason = describe(&refusal, &accelerator);
                    alert.set_body(&format!("{prompt}\n\n{reason}"));
                }
            }
            glib::Propagation::Stop
        }
    ));
    alert.add_controller(keys);
    alert.present(Some(dialog));
}

fn choose(customizable: &Customizable, chosen: &str, overrides: &mut Overrides) {
    let is_the_default = customizable
        .defaults
        .iter()
        .any(|default| shortcuts::canonical(default).as_deref() == Some(chosen));
    if is_the_default {
        overrides.remove(customizable.action);
    } else {
        overrides.insert(customizable.action.to_owned(), chosen.to_owned());
    }
}

fn describe(refusal: &Refusal, accelerator: &str) -> String {
    let shortcut = shortcuts::display(accelerator);
    match refusal {
        Refusal::Taken(other) => gettext("{shortcut} is already used by “{name}”.")
            .replace("{shortcut}", &shortcut)
            .replace("{name}", &shortcuts::label(other)),
        Refusal::NeedsModifier => gettext("Use Ctrl or Alt with a key, or a function key."),
        Refusal::Reserved => {
            gettext("{shortcut} is reserved by Pigoune.").replace("{shortcut}", &shortcut)
        }
        Refusal::Unreadable => gettext("This key combination is not supported."),
    }
}
