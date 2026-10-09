use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::{Tag, TagId, comparable};

use crate::tag_tree;

pub struct TagChoice {
    pub id: Option<TagId>,
    pub label: String,
}

struct Chooser {
    dialog: adw::Dialog,
    search: gtk::SearchEntry,
    choices: gtk::ListBox,
    all: Vec<TagChoice>,
    shown: RefCell<Vec<Option<TagId>>>,
    on_chosen: Box<dyn Fn(Option<TagId>)>,
}

pub fn destinations(
    all: &[Tag],
    moving: TagId,
    top_level_label: Option<&str>,
    leave_current_parent: bool,
) -> Vec<TagChoice> {
    let excluded = tag_tree::descendants(all, moving);
    let current_parent = all
        .iter()
        .find(|tag| tag.id == moving)
        .and_then(|tag| tag.parent);
    let mut choices = Vec::new();
    if let (Some(label), Some(_)) = (top_level_label, current_parent) {
        choices.push(TagChoice {
            id: None,
            label: label.to_owned(),
        });
    }
    let mut named: Vec<TagChoice> = all
        .iter()
        .filter(|tag| !excluded.contains(&tag.id))
        .filter(|tag| !(leave_current_parent && Some(tag.id) == current_parent))
        .map(|tag| TagChoice {
            id: Some(tag.id),
            label: crate::tag_input::path_of(all, tag).replace('/', " › "),
        })
        .collect();
    named.sort_by_key(|choice| comparable(&choice.label));
    choices.extend(named);
    choices
}

pub fn matching<'a>(all: &'a [TagChoice], typed: &str) -> Vec<&'a TagChoice> {
    let typed = comparable(typed.trim());
    all.iter()
        .filter(|choice| comparable(&choice.label).contains(&typed))
        .collect()
}

pub fn present(
    parent: &impl IsA<gtk::Widget>,
    title: &str,
    all: Vec<TagChoice>,
    on_chosen: impl Fn(Option<TagId>) + 'static,
) {
    let search = gtk::SearchEntry::builder()
        .placeholder_text(gettext("Search a Tag"))
        .build();
    let choices = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["navigation-sidebar"])
        .build();
    choices.set_placeholder(Some(
        &gtk::Label::builder()
            .label(gettext("No tag found"))
            .margin_top(12)
            .margin_bottom(12)
            .css_classes(["dim-label"])
            .build(),
    ));
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(360)
        .child(&choices)
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&search);
    content.append(&scrolled);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(340)
        .child(&toolbar)
        .build();
    let chooser = Rc::new(Chooser {
        dialog,
        search,
        choices,
        all,
        shown: RefCell::new(Vec::new()),
        on_chosen: Box::new(on_chosen),
    });
    connect(&chooser);
    chooser.refresh();
    chooser.dialog.set_focus(Some(&chooser.search));
    chooser.dialog.present(Some(parent));
}

fn connect(chooser: &Rc<Chooser>) {
    let weak = Rc::downgrade(chooser);
    chooser.search.connect_search_changed(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(chooser) = weak.upgrade() {
                chooser.refresh();
            }
        }
    ));
    chooser.search.connect_activate(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(chooser) = weak.upgrade() {
                chooser.choose(0);
            }
        }
    ));
    chooser.choices.connect_row_activated(glib::clone!(
        #[strong]
        weak,
        move |_, row| {
            if let (Some(chooser), Ok(index)) = (weak.upgrade(), usize::try_from(row.index())) {
                chooser.choose(index);
            }
        }
    ));
    let keys = gtk::EventControllerKey::new();
    let choices = chooser.choices.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Down {
            return glib::Propagation::Proceed;
        }
        if let Some(first) = choices.row_at_index(0) {
            first.grab_focus();
        }
        glib::Propagation::Stop
    });
    chooser.search.add_controller(keys);
    let kept_while_open = RefCell::new(Some(Rc::clone(chooser)));
    chooser.dialog.connect_closed(move |_| {
        kept_while_open.borrow_mut().take();
    });
}

impl Chooser {
    fn refresh(&self) {
        self.choices.remove_all();
        let found = matching(&self.all, &self.search.text());
        for choice in &found {
            let label = gtk::Label::builder()
                .label(&choice.label)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .build();
            self.choices.append(&label);
        }
        self.shown
            .replace(found.iter().map(|choice| choice.id).collect());
    }

    fn choose(&self, index: usize) {
        let Some(id) = self.shown.borrow().get(index).copied() else {
            return;
        };
        self.dialog.close();
        (self.on_chosen)(id);
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Tag, TagId};

    use super::{TagChoice, destinations, matching};

    fn tag(number: u8, name: &str, parent: Option<u8>) -> Tag {
        let id = |number: u8| {
            TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id")
        };
        Tag {
            id: id(number),
            name: name.to_owned(),
            parent: parent.map(id),
        }
    }

    fn choice(label: &str) -> TagChoice {
        TagChoice {
            id: None,
            label: label.to_owned(),
        }
    }

    #[test]
    fn choices_match_any_part_of_the_path_without_accents_or_case() {
        let all = [
            choice("Sujet"),
            choice("Sujet › Animaux"),
            choice("Sujet › Animaux › chèvre"),
            choice("Style"),
        ];
        let labels = |found: Vec<&TagChoice>| {
            found
                .into_iter()
                .map(|choice| choice.label.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(matching(&all, "")).len(), 4);
        assert_eq!(
            labels(matching(&all, "CHEVRE")),
            ["Sujet › Animaux › chèvre"]
        );
        assert_eq!(labels(matching(&all, "animaux")).len(), 2);
    }

    #[test]
    fn a_tag_can_go_anywhere_but_into_itself_or_below_itself_or_where_it_already_is() {
        let all = [
            tag(1, "Sujet", None),
            tag(2, "Animaux", Some(1)),
            tag(3, "chèvre", Some(2)),
            tag(4, "Style", None),
        ];
        let moving = all[1].id;
        let labels = |found: Vec<TagChoice>| {
            found
                .into_iter()
                .map(|choice| choice.label)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            labels(destinations(&all, moving, Some("Top Level"), true)),
            ["Top Level", "Style"]
        );
        assert_eq!(
            labels(destinations(&all, moving, None, false)),
            ["Style", "Sujet"]
        );
        assert_eq!(
            labels(destinations(&all, all[0].id, Some("Top Level"), true)),
            ["Style"]
        );
    }
}
