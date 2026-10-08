use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use pigoune_core::{CollectionId, CollectionLook, CollectionPath};

use crate::collection_choice::path_label;
use crate::removable_pill::removable_pill;
use crate::shortened_label::{PATH_CHARS, naming, shortened_label};
use crate::{collection_look_dialog, collection_looks};

const PILL_SPACING: i32 = 4;
const REMOVE_FROM_COLLECTION_ACTION: &str = "win.remove-from-collection";

type OpenedCallback = Box<dyn Fn(CollectionId)>;

pub struct SharedCollection {
    pub path: CollectionPath,
    pub look: CollectionLook,
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
        pub pills: OnceCell<adw::WrapBox>,
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

    pub fn show_collections(&self, current: &[SharedCollection]) {
        let imp = self.imp();
        let pills = part(&imp.pills);
        pills.remove_all();
        for shared in current {
            pills.append(&self.pill(shared));
        }
        pills.set_visible(!current.is_empty());
        part(&imp.empty_label).set_visible(current.is_empty());
    }

    fn pill(&self, shared: &SharedCollection) -> gtk::Box {
        let content = gtk::Box::builder().spacing(4).build();
        let icon = gtk::Image::from_icon_name(collection_looks::icon_name(&shared.look));
        collection_look_dialog::set_tint(
            icon.upcast_ref(),
            collection_looks::color_class(&shared.look),
        );
        content.append(&icon);
        let full_path = path_label(&shared.path);
        content.append(&shortened_label(&full_path, PATH_CHARS));
        let pill = gtk::Button::builder()
            .child(&content)
            .css_classes(["flat", "tag-pill"])
            .tooltip_text(naming(&full_path, &gettext("Open the Collection")))
            .build();
        let partial = shared.held_by < shared.out_of;
        if partial {
            content.append(
                &gtk::Label::builder()
                    .label(format!("{}/{}", shared.held_by, shared.out_of))
                    .css_classes(["dim-label", "numeric"])
                    .build(),
            );
            pill.set_tooltip_text(Some(&naming(
                &full_path,
                &gettext("On {count} of {total} assets")
                    .replace("{count}", &shared.held_by.to_string())
                    .replace("{total}", &shared.out_of.to_string()),
            )));
        }
        let id = shared.path.id;
        pill.connect_clicked(glib::clone!(
            #[weak(rename_to = places)]
            self,
            move |_| {
                if let Some(on_opened) = places.imp().on_opened.borrow().as_ref() {
                    on_opened(id);
                }
            }
        ));
        let name = path_label(&shared.path);
        let removable = removable_pill(
            &pill,
            &gettext("Remove from the Collection “{name}”").replace("{name}", &name),
            partial,
        );
        removable
            .remove
            .set_action_name(Some(REMOVE_FROM_COLLECTION_ACTION));
        removable
            .remove
            .set_action_target_value(Some(&id.to_string().to_variant()));
        removable.pill
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);

        let heading = gtk::Label::builder()
            .label(gettext("Collections"))
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build();
        let pills = adw::WrapBox::builder()
            .child_spacing(PILL_SPACING)
            .line_spacing(PILL_SPACING)
            .visible(false)
            .build();
        let empty_label = gtk::Label::builder()
            .label(gettext("In no collection"))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();

        self.append(&heading);
        self.append(&pills);
        self.append(&empty_label);
        set_part(&imp.pills, pills);
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
