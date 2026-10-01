use std::path::Path;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use pigoune_core::Library;

use crate::error_messages;

type LibraryCreatedCallback = Box<dyn Fn(Library)>;

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::LibraryCreatedCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/new-library-dialog.ui")]
    pub struct PigouneNewLibraryDialog {
        #[template_child]
        pub name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub location_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub create_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        pub location: RefCell<Option<PathBuf>>,
        pub on_library_created: RefCell<Option<LibraryCreatedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneNewLibraryDialog {
        const NAME: &'static str = "PigouneNewLibraryDialog";
        type Type = super::PigouneNewLibraryDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneNewLibraryDialog {}
    impl WidgetImpl for PigouneNewLibraryDialog {}
    impl AdwDialogImpl for PigouneNewLibraryDialog {}
}

glib::wrapper! {
    pub struct PigouneNewLibraryDialog(ObjectSubclass<imp::PigouneNewLibraryDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneNewLibraryDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_library_created(&self, callback: impl Fn(Library) + 'static) {
        self.imp()
            .on_library_created
            .replace(Some(Box::new(callback)));
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_inputs_changed(&self) {
        self.refresh_create_button();
    }

    #[template_callback]
    fn on_choose_location_clicked(&self) {
        let dialog = self.clone();
        glib::spawn_future_local(async move {
            dialog.choose_location().await;
        });
    }

    #[template_callback]
    fn on_create_clicked(&self) {
        self.create_library();
    }

    async fn choose_location(&self) {
        let file_dialog = gtk::FileDialog::builder()
            .title(gettext("Choose a Location"))
            .modal(true)
            .build();
        let parent_window = self.root().and_downcast::<gtk::Window>();
        let Ok(folder) = file_dialog
            .select_folder_future(parent_window.as_ref())
            .await
        else {
            return;
        };
        let Some(location) = folder.path() else {
            return;
        };

        let imp = self.imp();
        imp.location_row
            .set_subtitle(&folder_display_name(&location));
        imp.location.replace(Some(location));
        self.refresh_create_button();
    }

    fn refresh_create_button(&self) {
        let imp = self.imp();
        let has_name = !imp.name_row.text().trim().is_empty();
        let has_location = imp.location.borrow().is_some();
        imp.create_button.set_sensitive(has_name && has_location);
        imp.error_label.set_visible(false);
    }

    fn create_library(&self) {
        let imp = self.imp();
        let Some(location) = imp.location.borrow().clone() else {
            return;
        };

        match Library::create(&location, &imp.name_row.text()) {
            Ok(library) => {
                if let Some(on_library_created) = imp.on_library_created.borrow().as_ref() {
                    on_library_created(library);
                }
                self.close();
            }
            Err(error) => {
                imp.error_label.set_label(&error_messages::describe(&error));
                imp.error_label.set_visible(true);
            }
        }
    }
}

impl Default for PigouneNewLibraryDialog {
    fn default() -> Self {
        Self::new()
    }
}

fn folder_display_name(folder: &Path) -> String {
    folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
