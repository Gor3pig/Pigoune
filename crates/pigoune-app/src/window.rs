use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};
use pigoune_core::{
    AssetView, CollectionCommand, CollectionId, ImportError, ImportSummary, Library, LibraryError,
    library_display_name,
};

use crate::asset_object::{AssetEntry, PigouneAssetObject};
use crate::background_import::{self, FinishedImport};
use crate::collection_name_dialog::PigouneCollectionNameDialog;
use crate::collection_sort::{CollectionCriterion, CollectionOrder, CollectionTree};
use crate::error_messages;
use crate::import_report;
use crate::new_library_dialog::PigouneNewLibraryDialog;
use crate::settings;
use crate::sidebar::SidebarContent;
use crate::thumbnails::THUMBNAIL_PIXELS;
use crate::view_setting;

const WELCOME_PAGE: &str = "welcome";
const LIBRARY_PAGE: &str = "library";
const EMPTY_PAGE: &str = "empty";
const ASSETS_PAGE: &str = "assets";
const NOTHING_PAGE: &str = "nothing";
const MAIN_PAGE: &str = "main";
const PREVIEW_PAGE: &str = "preview";
const CREATE_LIBRARY_ACTION: &str = "win.create-library";
const OPEN_LIBRARY_ACTION: &str = "win.open-library";
const CLOSE_LIBRARY_ACTION: &str = "win.close-library";
const IMPORT_FILES_ACTION: &str = "win.import-files";
const IMPORT_FOLDER_ACTION: &str = "win.import-folder";
const ENLARGE_THUMBNAILS_ACTION: &str = "win.enlarge-thumbnails";
const SHRINK_THUMBNAILS_ACTION: &str = "win.shrink-thumbnails";
const NEW_COLLECTION_ACTION: &str = "win.new-collection";
const NEW_SUBCOLLECTION_ACTION: &str = "win.new-subcollection";
const RENAME_COLLECTION_ACTION: &str = "win.rename-collection";
const OPEN_LIBRARY_ACTIONS: [&str; 8] = [
    CLOSE_LIBRARY_ACTION,
    IMPORT_FILES_ACTION,
    IMPORT_FOLDER_ACTION,
    ENLARGE_THUMBNAILS_ACTION,
    SHRINK_THUMBNAILS_ACTION,
    NEW_COLLECTION_ACTION,
    NEW_SUBCOLLECTION_ACTION,
    RENAME_COLLECTION_ACTION,
];

const IMAGE_MIME_TYPES: [&str; 7] = [
    "image/svg+xml",
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/gif",
    "image/vnd.microsoft.icon",
    "image/x-icon",
];

const CLOSE_RESPONSE: &str = "close";
const OPEN_ANOTHER_RESPONSE: &str = "open-another";
const RETRY_RESPONSE: &str = "retry";

enum ReopeningChoice {
    Retry,
    OpenAnother,
    Dismiss,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gio, glib};
    use pigoune_core::{AssetView, Library};

    use crate::asset_details::PigouneAssetDetails;
    use crate::asset_grid::PigouneAssetGrid;
    use crate::asset_preview::PigouneAssetPreview;
    use crate::sidebar::PigouneSidebar;

    use super::{
        CLOSE_LIBRARY_ACTION, CREATE_LIBRARY_ACTION, ENLARGE_THUMBNAILS_ACTION,
        IMPORT_FILES_ACTION, IMPORT_FOLDER_ACTION, NEW_COLLECTION_ACTION, NEW_SUBCOLLECTION_ACTION,
        OPEN_LIBRARY_ACTION, RENAME_COLLECTION_ACTION, SHRINK_THUMBNAILS_ACTION,
        collection_parameter,
    };

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/window.ui")]
    pub struct PigouneWindow {
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub import_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub drop_hint: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub drop_area: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub library_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub asset_grid: TemplateChild<PigouneAssetGrid>,
        #[template_child]
        pub asset_details: TemplateChild<PigouneAssetDetails>,
        #[template_child]
        pub details_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub window_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub asset_preview: TemplateChild<PigouneAssetPreview>,
        #[template_child]
        pub sidebar: TemplateChild<PigouneSidebar>,
        #[template_child]
        pub nothing_page: TemplateChild<adw::StatusPage>,
        pub current_view: Cell<AssetView>,
        pub settings: OnceCell<gio::Settings>,
        pub library: RefCell<Option<Library>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWindow {
        const NAME: &'static str = "PigouneWindow";
        type Type = super::PigouneWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(class: &mut Self::Class) {
            PigouneAssetGrid::ensure_type();
            PigouneAssetDetails::ensure_type();
            PigouneAssetPreview::ensure_type();
            PigouneSidebar::ensure_type();
            class.bind_template();
            class.install_action(CREATE_LIBRARY_ACTION, None, |window, _, _| {
                window.show_new_library_dialog();
            });
            class.install_action_async(OPEN_LIBRARY_ACTION, None, |window, _, _| async move {
                window.choose_library_to_open().await;
            });
            class.install_action(CLOSE_LIBRARY_ACTION, None, |window, _, _| {
                window.close_library();
            });
            class.install_action_async(IMPORT_FILES_ACTION, None, |window, _, _| async move {
                window.choose_files_to_import().await;
            });
            class.install_action_async(IMPORT_FOLDER_ACTION, None, |window, _, _| async move {
                window.choose_folders_to_import().await;
            });
            class.install_action(ENLARGE_THUMBNAILS_ACTION, None, |window, _, _| {
                window.imp().asset_grid.enlarge_tiles();
            });
            class.install_action(SHRINK_THUMBNAILS_ACTION, None, |window, _, _| {
                window.imp().asset_grid.shrink_tiles();
            });
            class.install_action(NEW_COLLECTION_ACTION, None, |window, _, _| {
                window.ask_new_collection(None);
            });
            class.install_action(
                NEW_SUBCOLLECTION_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(parent) = collection_parameter(parameter) {
                        window.ask_new_collection(Some(parent));
                    }
                },
            );
            class.install_action(
                RENAME_COLLECTION_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(id) = collection_parameter(parameter) {
                        window.ask_collection_name(id);
                    }
                },
            );
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_library_actions_enabled(false);
            self.obj().accept_dropped_files();
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
        settings
            .bind(
                settings::THUMBNAIL_SIZE,
                &*window.imp().asset_grid,
                "tile-size",
            )
            .build();
        settings
            .bind(
                settings::SHOW_DETAILS,
                &*window.imp().details_button,
                "active",
            )
            .build();
        for (key, property) in [
            (settings::SORT_CRITERION, "sort-criterion"),
            (settings::SORT_REVERSED, "sort-reversed"),
        ] {
            settings
                .bind(key, &*window.imp().asset_grid, property)
                .build();
            window.add_action(&settings.create_action(key));
        }
        settings
            .bind(
                settings::PREVIEW_BACKGROUND,
                &*window.imp().asset_preview,
                "background",
            )
            .build();
        window.add_action(&settings.create_action(settings::PREVIEW_BACKGROUND));
        window.describe_selected_asset();
        window.connect_preview();
        window.follow_sidebar(&settings);
        window
            .imp()
            .settings
            .set(settings)
            .expect("settings are set only once, at construction");
        window
    }

    fn follow_sidebar(&self, settings: &gio::Settings) {
        for key in [
            settings::COLLECTION_SORT,
            settings::COLLECTION_SORT_REVERSED,
        ] {
            self.add_action(&settings.create_action(key));
        }
        for key in [
            settings::COLLECTION_SORT,
            settings::COLLECTION_SORT_REVERSED,
            settings::SHOW_COUNTS,
        ] {
            settings.connect_changed(
                Some(key),
                glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    move |_, _| window.refresh_sidebar()
                ),
            );
        }
        self.imp().sidebar.connect_view_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view| window.show_view(view)
        ));
        self.imp().sidebar.connect_files_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view, paths| {
                let target = match view {
                    AssetView::Collection(id) => Some(id),
                    AssetView::All | AssetView::Unclassified => None,
                };
                glib::spawn_future_local(async move {
                    window.import_paths_into(paths, target).await;
                });
            }
        ));
    }

    fn show_view(&self, view: AssetView) {
        let imp = self.imp();
        if imp.current_view.replace(view) == view {
            return;
        }
        self.remember_view(view);
        imp.asset_preview.close();
        self.refresh_grid();
    }

    fn view_on_opening(&self) -> AssetView {
        let settings = self.settings();
        if settings.boolean(settings::RESTORE_LAST_VIEW) {
            view_setting::from_setting(&settings.string(settings::LAST_VIEW))
        } else {
            AssetView::All
        }
    }

    fn collection_order(&self) -> CollectionOrder {
        let settings = self.settings();
        CollectionOrder {
            criterion: CollectionCriterion::from_setting(
                &settings.string(settings::COLLECTION_SORT),
            ),
            reversed: settings.boolean(settings::COLLECTION_SORT_REVERSED),
        }
    }

    fn target_collection(&self) -> Option<CollectionId> {
        match self.imp().current_view.get() {
            AssetView::Collection(id) => Some(id),
            AssetView::All | AssetView::Unclassified => None,
        }
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
        imp.asset_grid.forget_thumbnails();
        imp.library.replace(Some(library));
        imp.current_view.set(self.view_on_opening());
        self.refresh_assets();
        imp.stack.set_visible_child_name(LIBRARY_PAGE);
        imp.import_button.set_visible(true);
        self.set_library_actions_enabled(true);
    }

    fn close_library(&self) {
        let imp = self.imp();
        imp.library.replace(None);
        imp.asset_grid.show_assets(&[]);
        imp.asset_grid.forget_thumbnails();
        settings::store_string(self.settings(), settings::LAST_LIBRARY_PATH, "");
        imp.window_title.set_title("Pigoune");
        imp.stack.set_visible_child_name(WELCOME_PAGE);
        imp.import_button.set_visible(false);
        imp.details_button.set_visible(false);
        self.set_library_actions_enabled(false);
    }

    fn describe_selected_asset(&self) {
        let imp = self.imp();
        imp.asset_details.show(None, &imp.asset_grid.thumbnails());
        imp.asset_grid.connect_selected_asset_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |selected| {
                let imp = window.imp();
                imp.asset_details
                    .show(selected.as_ref(), &imp.asset_grid.thumbnails());
            }
        ));
    }

    fn connect_preview(&self) {
        let imp = self.imp();
        imp.asset_grid.connect_preview_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.open_preview()
        ));
        imp.asset_preview.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.leave_preview()
        ));
    }

    fn open_preview(&self) {
        let imp = self.imp();
        let Some(selection) = imp.asset_grid.selection() else {
            return;
        };
        if selection.selected_item().is_none() {
            return;
        }
        imp.window_stack.set_visible_child_name(PREVIEW_PAGE);
        imp.asset_preview
            .open(&selection, imp.asset_grid.thumbnails());
    }

    fn leave_preview(&self) {
        let imp = self.imp();
        if imp.window_stack.visible_child_name().as_deref() != Some(PREVIEW_PAGE) {
            return;
        }
        imp.window_stack.set_visible_child_name(MAIN_PAGE);
        imp.asset_grid.reveal_selected();
    }

    fn refresh_assets(&self) {
        self.imp().asset_preview.close();
        self.refresh_sidebar();
        self.refresh_grid();
    }

    fn refresh_sidebar(&self) {
        self.refresh_sidebar_revealing(Vec::new());
    }

    fn refresh_sidebar_revealing(&self, reveal: Vec<CollectionId>) {
        let imp = self.imp();
        let order = self.collection_order();
        let show_counts = self.settings().boolean(settings::SHOW_COUNTS);
        let read = imp.library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((library.visible_collections()?, library.view_counts()?))
        });
        let (collections, counts) = match read {
            Some(Ok(read)) => read,
            Some(Err(error)) => {
                self.show_library_error(&error);
                return;
            }
            None => return,
        };
        if let AssetView::Collection(id) = imp.current_view.get()
            && !collections.iter().any(|collection| collection.id == id)
        {
            imp.current_view.set(AssetView::All);
        }
        let tree = CollectionTree::new(collections, order, |name: &str| {
            glib::FilenameCollationKey::from(name)
        });
        imp.sidebar.show_content(&SidebarContent {
            tree,
            counts,
            show_counts,
            selected: imp.current_view.get(),
            reveal,
        });
    }

    fn ask_new_collection(&self, parent: Option<CollectionId>) {
        let title = if parent.is_some() {
            gettext("New Sub-collection")
        } else {
            gettext("New Collection")
        };
        let dialog = PigouneCollectionNameDialog::new(
            &title,
            &gettext("C_reate"),
            &gettext("New Collection"),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name| window.create_collection(name, parent)
            ),
        );
        dialog.present(Some(self));
    }

    fn create_collection(&self, name: &str, parent: Option<CollectionId>) -> Result<(), String> {
        let created = self.imp().library.borrow_mut().as_mut().map(|library| {
            let id = library.create_collection(name, parent)?;
            Ok((id, ancestors(library, parent)))
        });
        match created {
            Some(Ok((id, ancestors))) => {
                self.imp().current_view.set(AssetView::Collection(id));
                self.remember_view(AssetView::Collection(id));
                self.imp().asset_preview.close();
                self.refresh_sidebar_revealing(ancestors);
                self.refresh_grid();
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_collection(&error)),
            None => Ok(()),
        }
    }

    fn ask_collection_name(&self, id: CollectionId) {
        let current_name = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.collection(id).ok().flatten())
            .map(|collection| collection.name);
        let Some(current_name) = current_name else {
            return;
        };
        let dialog = PigouneCollectionNameDialog::new(
            &gettext("Rename Collection"),
            &gettext("_Rename"),
            &current_name,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name| window.rename_collection(id, name)
            ),
        );
        dialog.present(Some(self));
    }

    fn rename_collection(&self, id: CollectionId, name: &str) -> Result<(), String> {
        let renamed = self.imp().library.borrow_mut().as_mut().map(|library| {
            library.apply_collection_command(&CollectionCommand::Rename {
                id,
                name: name.to_owned(),
            })
        });
        match renamed {
            Some(Ok(_)) => {
                self.refresh_assets();
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_collection(&error)),
            None => Ok(()),
        }
    }

    fn remember_view(&self, view: AssetView) {
        settings::store_string(
            self.settings(),
            settings::LAST_VIEW,
            &view_setting::to_setting(view),
        );
    }

    fn refresh_grid(&self) {
        let imp = self.imp();
        let view = imp.current_view.get();
        let read = imp.library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((
                asset_objects(library, view)?,
                library.view_counts()?.all,
                view_name(library, view),
            ))
        });
        match read {
            Some(Ok((assets, library_total, view_name))) => {
                let page = if library_total == 0 {
                    EMPTY_PAGE
                } else if assets.is_empty() {
                    self.describe_empty_view(view, &view_name);
                    NOTHING_PAGE
                } else {
                    ASSETS_PAGE
                };
                imp.asset_grid.show_assets(&assets);
                imp.library_stack.set_visible_child_name(page);
                imp.details_button.set_visible(!assets.is_empty());
            }
            Some(Err(error)) => {
                imp.asset_grid.show_assets(&[]);
                imp.library_stack.set_visible_child_name(EMPTY_PAGE);
                imp.details_button.set_visible(false);
                self.show_library_error(&error);
            }
            None => {}
        }
    }

    fn describe_empty_view(&self, view: AssetView, view_name: &str) {
        let page = &self.imp().nothing_page;
        if view == AssetView::Unclassified {
            page.set_title(&gettext("No Unclassified Resources"));
            page.set_description(Some(&gettext(
                "Every resource is in at least one collection.",
            )));
        } else {
            page.set_title(&gettext("“{name}” Is Empty").replace("{name}", view_name));
            page.set_description(Some(&gettext(
                "Resources imported while this collection is selected are placed in it.",
            )));
        }
    }

    fn show_library_error(&self, error: &LibraryError) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Show the Library")),
            Some(&error_messages::describe(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn set_library_actions_enabled(&self, enabled: bool) {
        for action in OPEN_LIBRARY_ACTIONS {
            self.action_set_enabled(action, enabled);
        }
    }

    fn set_importing(&self, importing: bool) {
        for action in [CREATE_LIBRARY_ACTION, OPEN_LIBRARY_ACTION] {
            self.action_set_enabled(action, !importing);
        }
        self.set_library_actions_enabled(!importing);
    }

    fn accept_dropped_files(&self) {
        let drop_target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop_target.connect_accept(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            false,
            move |_, _| window.imp().library.borrow().is_some()
        ));
        drop_target.connect_enter(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, _| {
                window.show_drop_hint();
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_leave(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.imp().drop_hint.set_visible(false)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                window.imp().drop_hint.set_visible(false);
                let Ok(files) = value.get::<gdk::FileList>() else {
                    return false;
                };
                let paths: Vec<PathBuf> =
                    files.files().iter().filter_map(gio::File::path).collect();
                glib::spawn_future_local(async move { window.import_paths(paths).await });
                true
            }
        ));
        self.imp().drop_area.add_controller(drop_target);
    }

    fn show_drop_hint(&self) {
        let imp = self.imp();
        let view = imp.current_view.get();
        let library_name = imp
            .library
            .borrow()
            .as_ref()
            .map(|library| view_name(library, view))
            .unwrap_or_default();
        imp.drop_hint
            .set_title(&gettext("Drop to Import Into “{name}”").replace("{name}", &library_name));
        imp.drop_hint.set_visible(true);
    }

    async fn choose_files_to_import(&self) {
        let file_dialog = gtk::FileDialog::builder()
            .title(gettext("Import Files"))
            .accept_label(gettext("_Import"))
            .modal(true)
            .filters(&image_filters())
            .build();
        if let Ok(files) = file_dialog.open_multiple_future(Some(self)).await {
            self.import_paths(paths_of(&files)).await;
        }
    }

    async fn choose_folders_to_import(&self) {
        let file_dialog = gtk::FileDialog::builder()
            .title(gettext("Import Folders"))
            .accept_label(gettext("_Import"))
            .modal(true)
            .build();
        if let Ok(folders) = file_dialog.select_multiple_folders_future(Some(self)).await {
            self.import_paths(paths_of(&folders)).await;
        }
    }

    async fn import_paths(&self, paths: Vec<PathBuf>) {
        self.import_paths_into(paths, self.target_collection())
            .await;
    }

    async fn import_paths_into(&self, paths: Vec<PathBuf>, target: Option<CollectionId>) {
        if paths.is_empty() {
            return;
        }
        let destination = target.and_then(|id| {
            self.imp()
                .library
                .borrow()
                .as_ref()?
                .collection(id)
                .ok()
                .flatten()
                .map(|collection| collection.name)
        });
        let Some(library) = self.imp().library.take() else {
            return;
        };

        self.set_importing(true);
        let finished = background_import::run(self, library, paths.clone(), target).await;
        self.set_importing(false);

        if let Some(FinishedImport { library, result }) = finished {
            self.imp().library.replace(Some(library));
            self.refresh_assets();
            self.report_import(result, &paths, destination.as_deref());
        } else {
            self.close_library();
            import_report::unexpected_stop_dialog().present(Some(self));
        }
    }

    fn report_import(
        &self,
        result: Result<ImportSummary, ImportError>,
        chosen: &[PathBuf],
        destination: Option<&str>,
    ) {
        match result {
            Ok(summary) if import_report::needs_attention(&summary) => {
                import_report::summary_dialog(&summary, chosen, destination).present(Some(self));
            }
            Ok(summary) => {
                self.imp()
                    .toast_overlay
                    .add_toast(adw::Toast::new(&import_report::toast_text(
                        &summary,
                        destination,
                    )));
            }
            Err(error) => import_report::failure_dialog(&error).present(Some(self)),
        }
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

fn image_filters() -> gio::ListStore {
    let images = gtk::FileFilter::new();
    images.set_name(Some(&gettext("Images")));
    for mime_type in IMAGE_MIME_TYPES {
        images.add_mime_type(mime_type);
    }
    let everything = gtk::FileFilter::new();
    everything.set_name(Some(&gettext("All Files")));
    everything.add_pattern("*");

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&images);
    filters.append(&everything);
    filters
}

fn paths_of(files: &gio::ListModel) -> Vec<PathBuf> {
    (0..files.n_items())
        .filter_map(|position| files.item(position).and_downcast::<gio::File>())
        .filter_map(|file| file.path())
        .collect()
}

fn collection_parameter(parameter: Option<&glib::Variant>) -> Option<CollectionId> {
    CollectionId::parse(&parameter?.get::<String>()?)
}

fn ancestors(library: &Library, parent: Option<CollectionId>) -> Vec<CollectionId> {
    std::iter::successors(parent, |id| library.collection(*id).ok().flatten()?.parent).collect()
}

fn view_name(library: &Library, view: AssetView) -> String {
    match view {
        AssetView::Collection(id) => library
            .collection(id)
            .ok()
            .flatten()
            .map_or_else(|| library.name(), |collection| collection.name),
        AssetView::All | AssetView::Unclassified => library.name(),
    }
}

fn asset_objects(
    library: &Library,
    view: AssetView,
) -> Result<Vec<PigouneAssetObject>, LibraryError> {
    Ok(library
        .visible_assets_in(view)?
        .iter()
        .map(|asset| {
            PigouneAssetObject::new(AssetEntry {
                file: library.file_of(asset),
                thumbnail_file: library.thumbnail_file(asset.id, THUMBNAIL_PIXELS),
                asset: asset.clone(),
            })
        })
        .collect())
}
