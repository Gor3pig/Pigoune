use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::{CollectionId, CollectionPath};

use crate::collection_choice::{self, path_label};

type ChosenCallback = Box<dyn Fn(CollectionId)>;

pub struct SharedCollection {
    pub path: CollectionPath,
    pub held_by: usize,
    pub out_of: usize,
}

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::{CollectionId, CollectionPath};

    use super::ChosenCallback;

    #[derive(Default)]
    pub struct PigouneCollectionEditor {
        pub rows: OnceCell<gtk::ListBox>,
        pub empty_label: OnceCell<gtk::Label>,
        pub search: OnceCell<gtk::SearchEntry>,
        pub choices: OnceCell<gtk::ListBox>,
        pub no_choice_label: OnceCell<gtk::Label>,
        pub add_button: OnceCell<gtk::MenuButton>,
        pub all: RefCell<Vec<CollectionPath>>,
        pub held_by_all: RefCell<Vec<CollectionId>>,
        pub shown_choices: RefCell<Vec<CollectionId>>,
        pub on_added: RefCell<Option<ChosenCallback>>,
        pub on_removed: RefCell<Option<ChosenCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneCollectionEditor {
        const NAME: &'static str = "PigouneCollectionEditor";
        type Type = super::PigouneCollectionEditor;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneCollectionEditor {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigouneCollectionEditor {}
    impl BoxImpl for PigouneCollectionEditor {}
}

glib::wrapper! {
    pub struct PigouneCollectionEditor(ObjectSubclass<imp::PigouneCollectionEditor>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneCollectionEditor {
    pub fn connect_added(&self, callback: impl Fn(CollectionId) + 'static) {
        self.imp().on_added.replace(Some(Box::new(callback)));
    }

    pub fn connect_removed(&self, callback: impl Fn(CollectionId) + 'static) {
        self.imp().on_removed.replace(Some(Box::new(callback)));
    }

    pub fn show_collections(&self, current: &[SharedCollection], all: Vec<CollectionPath>) {
        let imp = self.imp();
        let rows = part(&imp.rows);
        rows.remove_all();
        for shared in current {
            rows.append(&self.row(shared));
        }
        rows.set_visible(!current.is_empty());
        part(&imp.empty_label).set_visible(current.is_empty());
        imp.held_by_all.replace(
            current
                .iter()
                .filter(|shared| shared.held_by == shared.out_of)
                .map(|shared| shared.path.id)
                .collect(),
        );
        imp.all.replace(all);
        self.refresh_choices();
    }

    fn row(&self, shared: &SharedCollection) -> adw::ActionRow {
        let label = path_label(&shared.path);
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&label))
            .build();
        if shared.held_by < shared.out_of {
            row.set_subtitle(
                &gettext("On {count} of {total} resources")
                    .replace("{count}", &shared.held_by.to_string())
                    .replace("{total}", &shared.out_of.to_string()),
            );
        }
        let name = shared.path.names.last().cloned().unwrap_or_default();
        let remove = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(gettext("Remove from the Collection “{name}”").replace("{name}", &name))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        let id = shared.path.id;
        remove.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| {
                if let Some(on_removed) = editor.imp().on_removed.borrow().as_ref() {
                    on_removed(id);
                }
            }
        ));
        row.add_suffix(&remove);
        row
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);

        let heading = gtk::Label::builder()
            .label(gettext("Collections"))
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let rows = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .visible(false)
            .build();
        let empty_label = gtk::Label::builder()
            .label(gettext("In no collection"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let add_button = self.build_chooser();

        self.append(&heading);
        self.append(&rows);
        self.append(&empty_label);
        self.append(&add_button);
        set_part(&imp.rows, rows);
        set_part(&imp.empty_label, empty_label);
        set_part(&imp.add_button, add_button);
    }

    fn build_chooser(&self) -> gtk::MenuButton {
        let imp = self.imp();
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
            .max_content_height(320)
            .child(&choices)
            .build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .width_request(260)
            .build();
        content.append(&search);
        content.append(&scrolled);
        let popover = gtk::Popover::builder().child(&content).build();
        let add_button = gtk::MenuButton::builder()
            .label(gettext("Add to a Collection…"))
            .popover(&popover)
            .build();
        self.connect_chooser(&popover, &search, &choices);
        set_part(&imp.search, search);
        set_part(&imp.choices, choices);
        set_part(&imp.no_choice_label, no_choice_label);
        add_button
    }

    fn connect_chooser(
        &self,
        popover: &gtk::Popover,
        search: &gtk::SearchEntry,
        choices: &gtk::ListBox,
    ) {
        popover.connect_show(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| {
                let search = part(&editor.imp().search);
                search.set_text("");
                editor.refresh_choices();
                search.grab_focus();
            }
        ));
        search.connect_search_changed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.refresh_choices()
        ));
        search.connect_activate(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.choose(0)
        ));
        choices.connect_row_activated(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, row| {
                if let Ok(index) = usize::try_from(row.index()) {
                    editor.choose(index);
                }
            }
        ));

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            choices,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if key != gdk::Key::Down {
                    return glib::Propagation::Proceed;
                }
                if let Some(first) = choices.row_at_index(0) {
                    first.grab_focus();
                }
                glib::Propagation::Stop
            }
        ));
        search.add_controller(keys);
    }

    fn refresh_choices(&self) {
        let imp = self.imp();
        let list = part(&imp.choices);
        list.remove_all();
        let all = imp.all.borrow();
        part(&imp.no_choice_label).set_label(&if all.is_empty() {
            gettext("No collections yet. Create one with the “+” button in the sidebar.")
        } else {
            gettext("No collection found")
        });
        let held_by_all = imp.held_by_all.borrow();
        let found = collection_choice::choices(&all, &part(&imp.search).text(), &held_by_all);
        for path in &found {
            let label = gtk::Label::builder()
                .label(path_label(path))
                .xalign(0.0)
                .wrap(true)
                .build();
            list.append(&label);
        }
        imp.shown_choices
            .replace(found.iter().map(|path| path.id).collect());
    }

    fn choose(&self, index: usize) {
        let imp = self.imp();
        let Some(id) = imp.shown_choices.borrow().get(index).copied() else {
            return;
        };
        part(&imp.add_button).popdown();
        if let Some(on_added) = imp.on_added.borrow().as_ref() {
            on_added(id);
        }
    }
}

impl Default for PigouneCollectionEditor {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the collection editor builds its parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the collection editor builds its parts once");
    }
}
