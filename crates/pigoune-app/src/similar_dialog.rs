use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use pigoune_core::AssetId;

use crate::thumbnails;

const THUMBNAIL_SIDE: i32 = 48;
const ALMOST_IDENTICAL_DISTANCE: u32 = 2;
const VERY_CLOSE_DISTANCE: u32 = 6;

type ShowInAll = Rc<dyn Fn(AssetId)>;

pub struct SimilarEntry {
    pub id: AssetId,
    pub name: String,
    pub summary: String,
    pub distance: u32,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
}

pub enum SimilarOutcome {
    Pending,
    NoFingerprint,
    NothingSimilar,
    Found(Vec<SimilarEntry>),
}

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::ShowInAll;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/similar-dialog.ui")]
    pub struct PigouneSimilarDialog {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub message_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub caption_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub results_list: TemplateChild<gtk::ListBox>,
        pub on_show_in_all: RefCell<Option<ShowInAll>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSimilarDialog {
        const NAME: &'static str = "PigouneSimilarDialog";
        type Type = super::PigouneSimilarDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneSimilarDialog {}
    impl WidgetImpl for PigouneSimilarDialog {}
    impl AdwDialogImpl for PigouneSimilarDialog {}
}

glib::wrapper! {
    pub struct PigouneSimilarDialog(ObjectSubclass<imp::PigouneSimilarDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl PigouneSimilarDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_show_in_all(&self, callback: impl Fn(AssetId) + 'static) {
        self.imp().on_show_in_all.replace(Some(Rc::new(callback)));
    }

    pub fn show_outcome(&self, name: &str, outcome: SimilarOutcome) {
        match outcome {
            SimilarOutcome::Pending => self.show_message(
                &gettext("Still Analysing"),
                &gettext("This asset has not been analysed yet. Try again in a moment."),
            ),
            SimilarOutcome::NoFingerprint => self.show_message(
                &gettext("No Look-Alikes to Find"),
                &gettext(
                    "The drawing of “{name}” is too plain to be compared: one color, or nothing visible.",
                )
                .replace("{name}", name),
            ),
            SimilarOutcome::NothingSimilar => self.show_message(
                &gettext("Nothing Looks Like This"),
                &gettext("No other asset resembles “{name}”.").replace("{name}", name),
            ),
            SimilarOutcome::Found(entries) => self.show_entries(name, entries),
        }
    }

    fn show_message(&self, title: &str, description: &str) {
        let imp = self.imp();
        imp.message_page.set_title(title);
        imp.message_page.set_description(Some(description));
        imp.stack.set_visible_child_name("message");
    }

    fn show_entries(&self, name: &str, entries: Vec<SimilarEntry>) {
        let imp = self.imp();
        imp.caption_label
            .set_label(&gettext("Closest to “{name}”, most alike first").replace("{name}", name));
        imp.results_list.remove_all();
        for entry in entries {
            imp.results_list.append(&self.row_of(&entry));
        }
        imp.stack.set_visible_child_name("results");
    }

    fn row_of(&self, entry: &SimilarEntry) -> adw::ActionRow {
        let picture = gtk::Picture::builder()
            .width_request(THUMBNAIL_SIDE)
            .height_request(THUMBNAIL_SIDE)
            .content_fit(gtk::ContentFit::Contain)
            .can_shrink(true)
            .build();
        glib::spawn_future_local(glib::clone!(
            #[weak]
            picture,
            #[strong(rename_to = file)]
            entry.file,
            #[strong(rename_to = thumbnail_file)]
            entry.thumbnail_file,
            async move {
                if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file).await {
                    picture.set_paintable(Some(&texture));
                }
            }
        ));
        let closeness = gtk::Label::builder()
            .label(closeness_text(entry.distance))
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(&entry.name)
            .subtitle(&entry.summary)
            .use_markup(false)
            .activatable(true)
            .build();
        row.add_prefix(&picture);
        row.add_suffix(&closeness);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let id = entry.id;
        row.connect_activated(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| dialog.show_in_all(id)
        ));
        row
    }

    fn show_in_all(&self, id: AssetId) {
        let callback = self.imp().on_show_in_all.borrow().clone();
        self.close();
        if let Some(callback) = callback {
            callback(id);
        }
    }
}

fn closeness_text(distance: u32) -> String {
    if distance <= ALMOST_IDENTICAL_DISTANCE {
        gettext("Almost identical")
    } else if distance <= VERY_CLOSE_DISTANCE {
        gettext("Very close")
    } else {
        gettext("Similar")
    }
}
