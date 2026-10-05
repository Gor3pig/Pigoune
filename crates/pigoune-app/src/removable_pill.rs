use gtk::prelude::*;

const REMOVE_ICON: &str = "window-close-symbolic";

pub struct RemovablePill {
    pub pill: gtk::Box,
    pub remove: gtk::Button,
}

#[must_use]
pub fn removable_pill(main: &gtk::Button, remove_tooltip: &str, partial: bool) -> RemovablePill {
    main.set_css_classes(&["flat", "removable-pill-main"]);
    let remove = gtk::Button::builder()
        .icon_name(REMOVE_ICON)
        .tooltip_text(remove_tooltip)
        .css_classes(["flat", "circular", "removable-pill-remove"])
        .valign(gtk::Align::Center)
        .build();
    let pill = gtk::Box::builder()
        .css_classes(["removable-pill"])
        .valign(gtk::Align::Center)
        .build();
    if partial {
        pill.add_css_class("partial");
    }
    pill.append(main);
    pill.append(&remove);
    RemovablePill { pill, remove }
}
