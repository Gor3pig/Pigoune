use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{CollectionId, CollectionPath};

use crate::collection_choice::path_label;

type OpenedCallback = Box<dyn Fn(CollectionId)>;

pub struct SharedCollection {
    pub path: CollectionPath,
    pub held_by: usize,
    pub out_of: usize,
}

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::OpenedCallback;

    #[derive(Default)]
    pub struct PigouneCollectionPlaces {
        pub heading: OnceCell<gtk::Label>,
        pub rows: OnceCell<gtk::ListBox>,
        pub empty_label: OnceCell<gtk::Label>,
        pub on_opened: RefCell<Option<OpenedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneCollectionPlaces {
        const NAME: &'static str = "PigouneCollectionPlaces";
        type Type = super::PigouneCollectionPlaces;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneCollectionPlaces {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigouneCollectionPlaces {}
    impl BoxImpl for PigouneCollectionPlaces {}
}

glib::wrapper! {
    pub struct PigouneCollectionPlaces(ObjectSubclass<imp::PigouneCollectionPlaces>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneCollectionPlaces {
    pub fn connect_opened(&self, callback: impl Fn(CollectionId) + 'static) {
        self.imp().on_opened.replace(Some(Box::new(callback)));
    }

    pub fn show_collections(&self, current: &[SharedCollection], selected: usize) {
        let imp = self.imp();
        part(&imp.heading).set_label(&ngettext(
            "Stored In",
            "Stored In",
            u32::try_from(selected).unwrap_or(u32::MAX),
        ));
        let rows = part(&imp.rows);
        rows.remove_all();
        for shared in current {
            rows.append(&self.row(shared));
        }
        rows.set_visible(!current.is_empty());
        part(&imp.empty_label).set_visible(current.is_empty());
    }

    fn row(&self, shared: &SharedCollection) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&path_label(&shared.path)))
            .activatable(true)
            .tooltip_text(gettext("Open the Collection"))
            .build();
        if shared.held_by < shared.out_of {
            row.set_subtitle(
                &gettext("On {count} of {total} resources")
                    .replace("{count}", &shared.held_by.to_string())
                    .replace("{total}", &shared.out_of.to_string()),
            );
        }
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let id = shared.path.id;
        row.connect_activated(glib::clone!(
            #[weak(rename_to = places)]
            self,
            move |_| {
                if let Some(on_opened) = places.imp().on_opened.borrow().as_ref() {
                    on_opened(id);
                }
            }
        ));
        row
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);

        let heading = gtk::Label::builder()
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

        self.append(&heading);
        self.append(&rows);
        self.append(&empty_label);
        set_part(&imp.heading, heading);
        set_part(&imp.rows, rows);
        set_part(&imp.empty_label, empty_label);
    }
}

impl Default for PigouneCollectionPlaces {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the collection places build their parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the collection places build their parts once");
    }
}
