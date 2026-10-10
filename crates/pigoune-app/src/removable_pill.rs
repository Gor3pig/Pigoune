use gtk::prelude::*;

const REMOVE_ICON: &str = "window-close-symbolic";

const ADD_ICON: &str = "list-add-symbolic";

pub struct RemovablePill {
    pub pill: gtk::Box,
    pub remove: gtk::Button,
    pub add: Option<gtk::Button>,
}

#[must_use]
pub fn removable_pill(
    main: &gtk::Button,
    remove_tooltip: &str,
    partial: bool,
    add_tooltip: Option<&str>,
) -> RemovablePill {
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
    let add = add_tooltip.map(|tooltip| {
        let add = gtk::Button::builder()
            .icon_name(ADD_ICON)
            .tooltip_text(tooltip)
            .css_classes(["flat", "circular", "removable-pill-add"])
            .valign(gtk::Align::Center)
            .build();
        pill.append(&add);
        add
    });
    pill.append(&remove);
    RemovablePill { pill, remove, add }
}
