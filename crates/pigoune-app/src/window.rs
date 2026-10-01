use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use pigoune_core::{Library, LibraryError, library_display_name};

use crate::error_messages;
use crate::new_library_dialog::PigouneNewLibraryDialog;
use crate::settings;

const WELCOME_PAGE: &str = "welcome";
const LIBRARY_PAGE: &str = "library";
const CLOSE_LIBRARY_ACTION: &str = "win.close-library";

const CLOSE_RESPONSE: &str = "close";
const OPEN_ANOTHER_RESPONSE: &str = "open-another";
const RETRY_RESPONSE: &str = "retry";

enum ReopeningChoice {
    Retry,
    OpenAnother,
    Dismiss,
}

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gio, glib};
    use pigoune_core::Library;

    use super::CLOSE_LIBRARY_ACTION;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/window.ui")]
    pub struct PigouneWindow {
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        pub settings: OnceCell<gio::Settings>,
        pub library: RefCell<Option<Library>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWindow {
        const NAME: &'static str = "PigouneWindow";
        type Type = super::PigouneWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.install_action("win.create-library", None, |window, _, _| {
                window.show_new_library_dialog();
            });
            class.install_action_async("win.open-library", None, |window, _, _| async move {
                window.choose_library_to_open().await;
            });
            class.install_action(CLOSE_LIBRARY_ACTION, None, |window, _, _| {
                window.close_library();
            });
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().action_set_enabled(CLOSE_LIBRARY_ACTION, false);
        }
    }

    impl WidgetImpl for PigouneWindow {}

    impl WindowImpl for PigouneWindow {
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_window_state();
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for PigouneWindow {}
    impl AdwApplicationWindowImpl for PigouneWindow {}
}

glib::wrapper! {
    pub struct PigouneWindow(ObjectSubclass<imp::PigouneWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PigouneWindow {
    pub fn new(application: &adw::Application, settings: gio::Settings) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", application)
            .build();
        window.restore_window_state(&settings);
        window
            .imp()
            .settings
            .set(settings)
            .expect("settings are set only once, at construction");
        window
    }

    pub fn reopen_last_library(&self) {
        let last_library_path = self.settings().string(settings::LAST_LIBRARY_PATH);
        if last_library_path.is_empty() {
            return;
        }
        let window = self.clone();
        glib::spawn_future_local(async move {
            window
                .reopen_library_at(PathBuf::from(last_library_path.as_str()))
                .await;
        });
    }

    async fn reopen_library_at(&self, root: PathBuf) {
        loop {
            let error = match Library::open(&root) {
                Ok(library) => {
                    self.show_library(library);
                    return;
                }
                Err(error) => error,
            };
            match self.ask_after_reopening_failure(&root, &error).await {
                ReopeningChoice::Retry => {}
                ReopeningChoice::OpenAnother => {
                    self.choose_library_to_open().await;
                    return;
                }
                ReopeningChoice::Dismiss => return,
            }
        }
    }

    async fn ask_after_reopening_failure(
        &self,
        root: &Path,
        error: &LibraryError,
    ) -> ReopeningChoice {
        let alert = opening_error_alert(root, error);
        alert.add_responses(&[
            (OPEN_ANOTHER_RESPONSE, &gettext("_Open Another Library…")),
            (RETRY_RESPONSE, &gettext("_Try Again")),
        ]);
        alert.set_response_appearance(RETRY_RESPONSE, adw::ResponseAppearance::Suggested);
        alert.set_default_response(Some(RETRY_RESPONSE));
        alert.set_close_response(CLOSE_RESPONSE);

        match alert.choose_future(Some(self)).await.as_str() {
            RETRY_RESPONSE => ReopeningChoice::Retry,
            OPEN_ANOTHER_RESPONSE => ReopeningChoice::OpenAnother,
            _ => ReopeningChoice::Dismiss,
        }
    }

    fn settings(&self) -> &gio::Settings {
        self.imp()
            .settings
            .get()
            .expect("settings are set at construction")
    }

    fn show_new_library_dialog(&self) {
        let dialog = PigouneNewLibraryDialog::new();
        dialog.connect_library_created(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |library| window.show_library(library)
        ));
        dialog.present(Some(self));
    }

    async fn choose_library_to_open(&self) {
        let file_dialog = gtk::FileDialog::builder()
            .title(gettext("Open a Pigoune Library"))
            .modal(true)
            .build();
        let Ok(folder) = file_dialog.select_folder_future(Some(self)).await else {
            return;
        };
        if let Some(root) = folder.path() {
            self.open_library_at(&root);
        }
    }

    fn open_library_at(&self, root: &Path) {
        if self.is_showing_library_at(root) {
            return;
        }
        match Library::open(root) {
            Ok(library) => self.show_library(library),
            Err(error) => self.show_opening_error(root, &error),
        }
    }

    fn is_showing_library_at(&self, root: &Path) -> bool {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .is_some_and(|library| library.root() == root)
    }

    fn show_library(&self, library: Library) {
        let imp = self.imp();
        imp.window_title.set_title(&library.name());
        settings::store_string(
            self.settings(),
            settings::LAST_LIBRARY_PATH,
            &library.root().to_string_lossy(),
        );
        imp.library.replace(Some(library));
        imp.stack.set_visible_child_name(LIBRARY_PAGE);
        self.action_set_enabled(CLOSE_LIBRARY_ACTION, true);
    }

    fn close_library(&self) {
        let imp = self.imp();
        imp.library.replace(None);
        settings::store_string(self.settings(), settings::LAST_LIBRARY_PATH, "");
        imp.window_title.set_title("Pigoune");
        imp.stack.set_visible_child_name(WELCOME_PAGE);
        self.action_set_enabled(CLOSE_LIBRARY_ACTION, false);
    }

    fn show_opening_error(&self, root: &Path, error: &LibraryError) {
        opening_error_alert(root, error).present(Some(self));
    }

    fn restore_window_state(&self, settings: &gio::Settings) {
        self.set_default_size(
            settings.int(settings::WINDOW_WIDTH),
            settings.int(settings::WINDOW_HEIGHT),
        );
        if settings.boolean(settings::WINDOW_MAXIMIZED) {
            self.maximize();
        }
    }

    fn save_window_state(&self) {
        let settings = self.settings();
        let (width, height) = self.default_size();
        settings::store_int(settings, settings::WINDOW_WIDTH, width);
        settings::store_int(settings, settings::WINDOW_HEIGHT, height);
        settings::store_bool(settings, settings::WINDOW_MAXIMIZED, self.is_maximized());
    }
}

fn opening_error_alert(root: &Path, error: &LibraryError) -> adw::AlertDialog {
    let heading = gettext("Unable to Open “{name}”").replace("{name}", &library_display_name(root));
    let alert = adw::AlertDialog::new(Some(&heading), Some(&error_messages::describe(error)));
    alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
    alert
}
