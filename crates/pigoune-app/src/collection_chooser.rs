use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::{CollectionId, CollectionPath};

use crate::collection_choice::{self, path_label};

struct Chooser {
    dialog: adw::Dialog,
    search: gtk::SearchEntry,
    choices: gtk::ListBox,
    no_choice_label: gtk::Label,
    all: Vec<CollectionPath>,
    held_by_all: Vec<CollectionId>,
    shown: RefCell<Vec<CollectionId>>,
    on_chosen: Box<dyn Fn(CollectionId)>,
}

pub fn present(
    parent: &impl IsA<gtk::Widget>,
    all: Vec<CollectionPath>,
    held_by_all: Vec<CollectionId>,
    on_chosen: impl Fn(CollectionId) + 'static,
) {
    let search = gtk::SearchEntry::builder()
        .placeholder_text(gettext("Search a Collection"))
        .build();
    let choices = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["navigation-sidebar"])
        .build();
    let no_choice_label = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(6)
        .margin_end(6)
        .css_classes(["dim-label"])
        .build();
    choices.set_placeholder(Some(&no_choice_label));
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
        .title(gettext("Add to a Collection"))
        .content_width(340)
        .child(&toolbar)
        .build();

    let chooser = Rc::new(Chooser {
        dialog,
        search,
        choices,
        no_choice_label,
        all,
        held_by_all,
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
        self.no_choice_label.set_label(&if self.all.is_empty() {
            gettext("No collections yet. Create one with the “+” button in the sidebar.")
        } else {
            gettext("No collection found")
        });
        let found = collection_choice::choices(&self.all, &self.search.text(), &self.held_by_all);
        for path in &found {
            let label = gtk::Label::builder()
                .label(path_label(path))
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .build();
            self.choices.append(&label);
        }
        self.shown
            .replace(found.iter().map(|path| path.id).collect());
    }

    fn choose(&self, index: usize) {
        let Some(id) = self.shown.borrow().get(index).copied() else {
            return;
        };
        self.dialog.close();
        (self.on_chosen)(id);
    }
}
