use std::path::{Path, PathBuf};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gdk, gio, glib};
use pigoune_core::{
    AssetCommand, AssetFilter, AssetId, AssetView, ChangeStamp, CollectionCommand, CollectionId,
    CollectionPath, CollectionRemoval, ImportError, ImportSummary, Library, LibraryError,
    TRASH_RETENTION, Tag, TagCommand, TagError, TagId, TextField, UndoError, library_display_name,
};

use crate::asset_object::{AssetEntry, PigouneAssetObject};
use crate::background_import::{self, FinishedImport};
use crate::clipboard_content;
use crate::collection_chooser;
use crate::collection_drop::{self, CollectionDrop};
use crate::collection_name_dialog::PigouneCollectionNameDialog;
use crate::collection_places::SharedCollection;
use crate::collection_sort::{CollectionCriterion, CollectionOrder, CollectionTree};
use crate::drop_message;
use crate::error_messages;
use crate::import_report::{self, Destination};
use crate::new_library_dialog::PigouneNewLibraryDialog;
use crate::preferences_dialog;
use crate::settings;
use crate::sidebar::{HoveredDrop, SidebarContent};
use crate::tag_editor::SharedTag;
use crate::thumbnails::THUMBNAIL_PIXELS;
use crate::undo_message;
use crate::view_setting;

const WELCOME_PAGE: &str = "welcome";
const LIBRARY_PAGE: &str = "library";
const EMPTY_PAGE: &str = "empty";
const ASSETS_PAGE: &str = "assets";
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
const TOGGLE_FAVORITE_ACTION: &str = "win.toggle-favorite";
const RENAME_TAG_ACTION: &str = "win.rename-tag";
const OPEN_PREVIEW_ACTION: &str = "win.open-preview";
const OPEN_WITH_ACTION: &str = "win.open-with";
const RENAME_ASSET_ACTION: &str = "win.rename-asset";
const ADD_TAG_ACTION: &str = "win.add-tag";
const ADD_TO_COLLECTION_ACTION: &str = "win.add-to-collection";
const REMOVE_FROM_COLLECTION_ACTION: &str = "win.remove-from-collection";
const TRASH_SELECTED_ACTION: &str = "win.trash-selected";
const RESTORE_SELECTED_ACTION: &str = "win.restore-selected";
const EMPTY_TRASH_ACTION: &str = "win.empty-trash";
const EMPTY_TRASH_RESPONSE: &str = "empty";
const DELETE_TAG_ACTION: &str = "win.delete-tag";
const DELETE_COLLECTION_ACTION: &str = "win.delete-collection";
const UNDO_ACTION: &str = "win.undo";
const PREFERENCES_ACTION: &str = "win.preferences";
const SEARCH_ACTION: &str = "win.search";
const COPY_SELECTED_ACTION: &str = "win.copy-selected";
const EXPORT_SELECTED_ACTION: &str = "win.export-selected";
const SELECT_ALL_ACTION: &str = "win.select-all";
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
const OPEN_LIBRARY_ACTIONS: [&str; 14] = [
    SEARCH_ACTION,
    UNDO_ACTION,
    CLOSE_LIBRARY_ACTION,
    IMPORT_FILES_ACTION,
    IMPORT_FOLDER_ACTION,
    ENLARGE_THUMBNAILS_ACTION,
    SHRINK_THUMBNAILS_ACTION,
    NEW_COLLECTION_ACTION,
    NEW_SUBCOLLECTION_ACTION,
    RENAME_COLLECTION_ACTION,
    DELETE_COLLECTION_ACTION,
    TOGGLE_FAVORITE_ACTION,
    RENAME_TAG_ACTION,
    DELETE_TAG_ACTION,
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
const MERGE_RESPONSE: &str = "merge";
const DELETE_RESPONSE: &str = "delete";

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
    use pigoune_core::{AssetFilter, AssetView, ChangeStamp, Library};

    use crate::asset_details::PigouneAssetDetails;
    use crate::asset_grid::PigouneAssetGrid;
    use crate::asset_preview::PigouneAssetPreview;
    use crate::grid_header::PigouneGridHeader;
    use crate::sidebar::PigouneSidebar;

    use super::{
        ADD_TAG_ACTION, ADD_TO_COLLECTION_ACTION, CLOSE_LIBRARY_ACTION, COPY_SELECTED_ACTION,
        CREATE_LIBRARY_ACTION, DELETE_COLLECTION_ACTION, DELETE_TAG_ACTION, EMPTY_TRASH_ACTION,
        ENLARGE_THUMBNAILS_ACTION, EXPORT_SELECTED_ACTION, IMPORT_FILES_ACTION,
        IMPORT_FOLDER_ACTION, NEW_COLLECTION_ACTION, NEW_SUBCOLLECTION_ACTION, OPEN_LIBRARY_ACTION,
        OPEN_PREVIEW_ACTION, OPEN_WITH_ACTION, PREFERENCES_ACTION, REMOVE_FROM_COLLECTION_ACTION,
        RENAME_ASSET_ACTION, RENAME_COLLECTION_ACTION, RENAME_TAG_ACTION, RESTORE_SELECTED_ACTION,
        SEARCH_ACTION, SELECT_ALL_ACTION, SHRINK_THUMBNAILS_ACTION, TOGGLE_FAVORITE_ACTION,
        TRASH_SELECTED_ACTION, UNDO_ACTION, collection_parameter, tag_parameter,
    };
    use pigoune_core::CollectionCommand;

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
        pub trash_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub asset_grid: TemplateChild<PigouneAssetGrid>,
        #[template_child]
        pub asset_details: TemplateChild<PigouneAssetDetails>,
        #[template_child]
        pub window_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub asset_preview: TemplateChild<PigouneAssetPreview>,
        #[template_child]
        pub sidebar: TemplateChild<PigouneSidebar>,
        #[template_child]
        pub sidebar_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub details_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub grid_header: TemplateChild<PigouneGridHeader>,
        pub current_view: Cell<AssetView>,
        pub browsing_selection: Cell<bool>,
        pub settings: OnceCell<gio::Settings>,
        pub library: RefCell<Option<Library>>,
        pub fresh_change: Cell<Option<ChangeStamp>>,
        pub search_query: RefCell<String>,
        pub filters: RefCell<AssetFilter>,
        pub hovered_drop: Cell<Option<(AssetView, bool)>>,
        pub undo_toast: RefCell<Option<(adw::Toast, ChangeStamp)>>,
    }

    fn install_asset_actions(class: &mut <PigouneWindow as ObjectSubclass>::Class) {
        class.install_action(TOGGLE_FAVORITE_ACTION, None, |window, _, _| {
            window.toggle_favorite();
        });
        class.install_action(TRASH_SELECTED_ACTION, None, |window, _, _| {
            window.trash_selected();
        });
        class.install_action(RESTORE_SELECTED_ACTION, None, |window, _, _| {
            window.restore_selected();
        });
        class.install_action(EMPTY_TRASH_ACTION, None, |window, _, _| {
            window.ask_to_empty_trash();
        });
        class.install_action(UNDO_ACTION, None, |window, _, _| {
            window.undo();
        });
        class.install_action(COPY_SELECTED_ACTION, None, |window, _, _| {
            window.copy_selected();
        });
        class.install_action(SELECT_ALL_ACTION, None, |window, _, _| {
            window.select_all();
        });
        class.install_action_async(EXPORT_SELECTED_ACTION, None, |window, _, _| async move {
            window.export_selected().await;
        });
        class.install_action_async(OPEN_WITH_ACTION, None, |window, _, _| async move {
            window.open_selected_with().await;
        });
        class.install_action(OPEN_PREVIEW_ACTION, None, |window, _, _| {
            window.after_menu_closes(|window| window.open_preview(None));
        });
        class.install_action(RENAME_ASSET_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::start_renaming_selected);
        });
        class.install_action(ADD_TAG_ACTION, None, |window, _, _| {
            window.after_menu_closes(|window| {
                window.imp().grid_header.details_button().set_active(true);
                window.imp().asset_details.focus_tag_entry();
            });
        });
        class.install_action(ADD_TO_COLLECTION_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::ask_collection_for_selected);
        });
        class.install_action(
            REMOVE_FROM_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(collection) = collection_parameter(parameter) {
                    window.change_selected_collections(|assets| CollectionCommand::RemoveAssets {
                        collection,
                        assets,
                    });
                }
            },
        );
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
            PigouneGridHeader::ensure_type();
            class.bind_template();
            class.install_action(CREATE_LIBRARY_ACTION, None, |window, _, _| {
                window.show_new_library_dialog();
            });
            class.install_action_async(OPEN_LIBRARY_ACTION, None, |window, _, _| async move {
                window.choose_library_to_open().await;
            });
            class.install_action(SEARCH_ACTION, None, |window, _, _| {
                window.imp().grid_header.search_entry().grab_focus();
            });
            class.install_action(PREFERENCES_ACTION, None, |window, _, _| {
                window.show_preferences();
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
            class.install_action(
                RENAME_TAG_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(tag) = tag_parameter(parameter) {
                        window.ask_tag_name(tag);
                    }
                },
            );
            class.install_action(
                DELETE_TAG_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(tag) = tag_parameter(parameter) {
                        window.ask_tag_deletion(tag);
                    }
                },
            );
            install_asset_actions(class);
            class.install_action(
                DELETE_COLLECTION_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(id) = collection_parameter(parameter) {
                        window.ask_collection_deletion(id);
                    }
                },
            );
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
                settings::SHOW_NAMES,
                &*window.imp().asset_grid,
                "show-names",
            )
            .build();
        window
            .imp()
            .asset_grid
            .link_size_adjustment(&window.imp().grid_header.size_adjustment());
        window.follow_sort_label(&settings);
        settings
            .bind(
                settings::SHOW_DETAILS,
                &window.imp().grid_header.details_button(),
                "active",
            )
            .get()
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
        window.follow_trash_confirmation(&settings);
        window.follow_search();
        window.follow_filters();
        window.offer_export_copies();
        window.follow_details_panel();
        window.follow_sidebar_panel();
        window.type_to_search();
        window
            .imp()
            .settings
            .set(settings)
            .expect("settings are set only once, at construction");
        window
    }

    fn follow_sort_label(&self, settings: &gio::Settings) {
        self.label_sort_button(settings);
        settings.connect_changed(
            Some(settings::SORT_CRITERION),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |settings, _| window.label_sort_button(settings)
            ),
        );
    }

    fn label_sort_button(&self, settings: &gio::Settings) {
        let label = match settings.string(settings::SORT_CRITERION).as_str() {
            "name" => gettext("Name"),
            "type" => gettext("Type"),
            "dimensions" => gettext("Dimensions"),
            "size" => gettext("Size"),
            _ => gettext("Date Added"),
        };
        self.imp().grid_header.show_sort_criterion(&label);
    }

    fn follow_details_panel(&self) {
        let imp = self.imp();
        imp.grid_header
            .details_button()
            .connect_active_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |button| {
                    window.remember_details_choice(button.is_active());
                    window.update_details_panel();
                }
            ));
        imp.details_split.connect_collapsed_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |split| window.fold_details_panel(split.is_collapsed())
        ));
        imp.details_split.connect_show_sidebar_notify(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |split| {
                let button = &window.imp().grid_header.details_button();
                if button.is_visible() && button.is_active() != split.shows_sidebar() {
                    button.set_active(split.shows_sidebar());
                }
            }
        ));
    }

    fn remember_details_choice(&self, shown: bool) {
        let imp = self.imp();
        let user_toggle = imp.grid_header.details_button().is_visible();
        if user_toggle
            && !imp.details_split.is_collapsed()
            && let Some(settings) = imp.settings.get()
        {
            settings::store_bool(settings, settings::SHOW_DETAILS, shown);
        }
    }

    fn fold_details_panel(&self, collapsed: bool) {
        let shown = !collapsed
            && self
                .imp()
                .settings
                .get()
                .is_some_and(|settings| settings.boolean(settings::SHOW_DETAILS));
        self.imp().grid_header.details_button().set_active(shown);
        self.update_details_panel();
    }

    fn follow_sidebar_panel(&self) {
        let imp = self.imp();
        imp.sidebar_split
            .bind_property("show-sidebar", &imp.grid_header.sidebar_button(), "active")
            .bidirectional()
            .sync_create()
            .build();
        imp.sidebar_split
            .connect_collapsed_notify(|split| split.set_show_sidebar(!split.is_collapsed()));
    }

    fn close_folded_sidebar(&self) {
        let split = &self.imp().sidebar_split;
        if split.is_collapsed() {
            split.set_show_sidebar(false);
        }
    }

    fn update_details_panel(&self) {
        let imp = self.imp();
        let button = &imp.grid_header.details_button();
        imp.details_split
            .set_show_sidebar(button.is_visible() && button.is_active());
    }

    fn type_to_search(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| window.start_search_with(key, modifiers)
        ));
        self.add_controller(keys);
    }

    fn start_search_with(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let shortcut_modifiers = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;
        let typed = key
            .to_unicode()
            .filter(|character| !character.is_control() && !character.is_whitespace());
        let Some(typed) = typed else {
            return glib::Propagation::Proceed;
        };
        if modifiers.intersects(shortcut_modifiers) || !self.can_start_search() {
            return glib::Propagation::Proceed;
        }
        let entry = self.imp().grid_header.search_entry();
        let text = format!("{}{typed}", entry.text());
        entry.grab_focus();
        entry.set_text(&text);
        entry.set_position(-1);
        glib::Propagation::Stop
    }

    fn can_start_search(&self) -> bool {
        let imp = self.imp();
        let library_shown = imp.library.borrow().is_some()
            && imp.stack.visible_child_name().as_deref() == Some(LIBRARY_PAGE)
            && imp.window_stack.visible_child_name().as_deref() == Some(MAIN_PAGE)
            && imp.library_stack.visible_child_name().as_deref() == Some(ASSETS_PAGE);
        let typing_elsewhere = GtkWindowExt::focus(self).is_some_and(|focus| {
            focus.is::<gtk::Text>()
                || focus.is::<gtk::TextView>()
                || focus.ancestor(adw::Dialog::static_type()).is_some()
                || focus.ancestor(gtk::Popover::static_type()).is_some()
        });
        library_shown && !typing_elsewhere
    }

    fn follow_search(&self) {
        let entry = self.imp().grid_header.search_entry();
        entry.connect_search_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| window.search(&entry.text())
        ));
        entry.connect_stop_search(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| {
                entry.set_text("");
                window.search("");
                window.imp().asset_grid.grab_focus();
            }
        ));
    }

    fn offer_export_copies(&self) {
        self.imp().asset_grid.connect_export_copies(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or_default]
            move |assets| {
                window
                    .imp()
                    .library
                    .borrow()
                    .as_ref()
                    .and_then(|library| library.export_copies(assets).ok())
                    .unwrap_or_default()
            }
        ));
    }

    fn follow_filters(&self) {
        self.imp()
            .grid_header
            .filter_popover()
            .connect_changed(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.apply_filters()
            ));
    }

    fn apply_filters(&self) {
        let imp = self.imp();
        let choice = imp.grid_header.filter_popover().choice();
        let filters = AssetFilter {
            text: String::new(),
            formats: choice.formats,
            favorites_only: choice.favorites_only,
        };
        imp.grid_header.show_filter_count(filters.chosen_filters());
        imp.filters.replace(filters);
        self.refresh_grid();
    }

    fn search(&self, query: &str) {
        let imp = self.imp();
        if *imp.search_query.borrow() == query {
            return;
        }
        imp.search_query.replace(query.to_owned());
        self.refresh_grid();
    }

    fn reset_search(&self) {
        let imp = self.imp();
        imp.search_query.replace(String::new());
        imp.filters.replace(AssetFilter::default());
        imp.grid_header.filter_popover().clear();
        imp.grid_header.show_filter_count(0);
        imp.grid_header.search_entry().set_text("");
    }

    fn follow_trash_confirmation(&self, settings: &gio::Settings) {
        self.label_empty_trash_button(settings);
        settings.connect_changed(
            Some(settings::AUTO_EMPTY_TRASH),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| window.empty_expired_trash()
            ),
        );
        settings.connect_changed(
            Some(settings::CONFIRM_EMPTY_TRASH),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |settings, _| window.label_empty_trash_button(settings)
            ),
        );
    }

    fn label_empty_trash_button(&self, settings: &gio::Settings) {
        let label = if settings.boolean(settings::CONFIRM_EMPTY_TRASH) {
            gettext("_Empty Trash…")
        } else {
            gettext("_Empty Trash")
        };
        self.imp().trash_banner.set_button_label(Some(&label));
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
            move |view| {
                window.show_view(view);
                window.close_folded_sidebar();
            }
        ));
        self.imp().sidebar.connect_collection_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dragged, drop| window.drop_collection(dragged, drop)
        ));
        self.imp().sidebar.connect_assets_hovered(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |hovered| window.describe_hovered_drop(hovered.as_ref())
        ));
        self.imp().sidebar.connect_assets_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view, assets, keep_source| window.drop_assets_on(view, &assets, keep_source)
        ));
        self.imp().sidebar.connect_files_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view, paths| {
                let target = match view {
                    AssetView::Collection(id) => ImportTarget::Collection(id),
                    AssetView::Tag(id) => ImportTarget::Tag(id),
                    AssetView::All
                    | AssetView::Favorites
                    | AssetView::Unclassified
                    | AssetView::Trash => ImportTarget::Nowhere,
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
            AssetView::All
            | AssetView::Favorites
            | AssetView::Unclassified
            | AssetView::Tag(_)
            | AssetView::Trash => None,
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

    fn show_preferences(&self) {
        preferences_dialog::present(self, self.settings());
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
        self.forget_undo_toast();
        self.reset_search();
        imp.library.replace(Some(library));
        imp.current_view.set(self.view_on_opening());
        self.refresh_assets();
        self.empty_expired_trash();
        imp.stack.set_visible_child_name(LIBRARY_PAGE);
        imp.import_button.set_visible(true);
        self.set_library_actions_enabled(true);
    }

    fn close_library(&self) {
        let imp = self.imp();
        self.forget_undo_toast();
        self.reset_search();
        imp.library.replace(None);
        imp.asset_grid.show_assets(&[]);
        imp.asset_grid.forget_thumbnails();
        settings::store_string(self.settings(), settings::LAST_LIBRARY_PATH, "");
        imp.window_title.set_title("Pigoune");
        imp.stack.set_visible_child_name(WELCOME_PAGE);
        imp.import_button.set_visible(false);
        imp.grid_header.details_button().set_visible(false);
        self.set_library_actions_enabled(false);
    }

    fn describe_selected_asset(&self) {
        let imp = self.imp();
        imp.asset_details.show(None, &imp.asset_grid.thumbnails());
        imp.asset_grid.connect_selection_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |selected| window.show_selection(&selected)
        ));
        imp.asset_details.connect_renamed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |asset, name| window.rename_asset(asset, &name)
        ));
        imp.asset_details.connect_text_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |asset, field, value| window.change_asset_text(asset, field, &value)
        ));
        imp.asset_grid.connect_context_menu_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or_else]
            || gio::Menu::new().upcast(),
            move || window.asset_menu()
        ));
        imp.asset_grid.connect_trash_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.trash_selected()
        ));
        imp.asset_grid.connect_rename_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.start_renaming_selected()
        ));
        for editor in imp.asset_details.tag_editors() {
            editor.connect_added(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |names| window.add_tags_to_selected(names)
            ));
            editor.connect_removed(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |tag| window.remove_tag_from_selected(tag)
            ));
            editor.connect_opened(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |tag| window.go_to_view(AssetView::Tag(tag), Vec::new())
            ));
            editor.connect_applied(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |name| window.add_tags_to_selected(vec![name])
            ));
        }
        imp.asset_details.connect_collection_opened(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |collection| window.go_to_collection(collection)
        ));
    }

    fn start_renaming_selected(&self) {
        let imp = self.imp();
        if !self.is_showing_trash() && imp.asset_grid.selected_asset().is_some() {
            imp.grid_header.details_button().set_active(true);
            imp.asset_details.start_renaming();
        }
    }

    fn after_menu_closes(&self, action: impl FnOnce(&Self) + 'static) {
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || action(&window)
        ));
    }

    fn asset_menu(&self) -> gio::MenuModel {
        let imp = self.imp();
        let selected = imp.asset_grid.selected_assets();
        let viewing = gio::Menu::new();
        viewing.append_item(&menu_item(
            &gettext("Open Preview"),
            OPEN_PREVIEW_ACTION,
            Some("space"),
        ));
        if selected.len() == 1 && !self.is_showing_trash() {
            viewing.append(Some(&gettext("Open With…")), Some(OPEN_WITH_ACTION));
        }
        if selected.len() == 1 {
            viewing.append_item(&menu_item(
                &gettext("Rename…"),
                RENAME_ASSET_ACTION,
                Some("F2"),
            ));
        }
        if self.is_showing_trash() {
            viewing.append(Some(&gettext("_Restore")), Some(RESTORE_SELECTED_ACTION));
            return viewing.upcast();
        }
        let sharing = gio::Menu::new();
        sharing.append_item(&menu_item(
            &gettext("Copy"),
            COPY_SELECTED_ACTION,
            Some("<Control>c"),
        ));
        sharing.append(Some(&gettext("Export To…")), Some(EXPORT_SELECTED_ACTION));
        let organizing = gio::Menu::new();
        let favorite_label = if selected.iter().all(PigouneAssetObject::favorite) {
            gettext("Remove from Favorites")
        } else {
            gettext("Add to Favorites")
        };
        organizing.append(Some(&favorite_label), Some(TOGGLE_FAVORITE_ACTION));
        organizing.append(Some(&gettext("Add a Tag…")), Some(ADD_TAG_ACTION));
        organizing.append(
            Some(&gettext("Add to a Collection…")),
            Some(ADD_TO_COLLECTION_ACTION),
        );
        if let AssetView::Collection(id) = imp.current_view.get() {
            let name = self.collection_name(id).unwrap_or_default();
            let item = gio::MenuItem::new(
                Some(&gettext("Remove from the Collection “{name}”").replace("{name}", &name)),
                None,
            );
            item.set_action_and_target_value(
                Some(REMOVE_FROM_COLLECTION_ACTION),
                Some(&id.to_string().to_variant()),
            );
            organizing.append_item(&item);
        }
        let discarding = gio::Menu::new();
        discarding.append_item(&menu_item(
            &gettext("Move to Trash"),
            TRASH_SELECTED_ACTION,
            Some("Delete"),
        ));
        let menu = gio::Menu::new();
        menu.append_section(None, &viewing);
        menu.append_section(None, &sharing);
        menu.append_section(None, &organizing);
        menu.append_section(None, &discarding);
        menu.upcast()
    }

    fn show_selection(&self, selected: &[PigouneAssetObject]) {
        let imp = self.imp();
        let thumbnails = imp.asset_grid.thumbnails();
        match selected {
            [] => imp.asset_details.show(None, &thumbnails),
            trashed if self.is_showing_trash() => imp.asset_details.show_trashed(trashed),
            [single] => {
                imp.asset_details.show(Some(single), &thumbnails);
                let timing = imp
                    .library
                    .borrow()
                    .as_ref()
                    .and_then(|library| library.animation_timing(single.asset()));
                imp.asset_details.show_animation_timing(timing);
            }
            several => imp.asset_details.show_group(several, &thumbnails),
        }
        self.refresh_selected_tags();
        self.refresh_selected_collections();
    }

    fn is_showing_trash(&self) -> bool {
        self.imp().current_view.get() == AssetView::Trash
    }

    fn trash_selected(&self) {
        if !self.is_showing_trash() {
            self.set_trashed(&self.selected_ids(), true);
        }
    }

    fn restore_selected(&self) {
        if self.is_showing_trash() {
            self.set_trashed(&self.selected_ids(), false);
        }
    }

    fn set_trashed(&self, assets: &[AssetId], trashed: bool) {
        if assets.is_empty()
            || !self.apply_asset_command(&AssetCommand::SetTrashed {
                assets: assets.to_vec(),
                trashed,
            })
        {
            return;
        }
        self.refresh_sidebar();
        self.drop_assets_leaving_view(assets);
        let count = assets.len();
        let message = if trashed {
            ngettext(
                "{count} resource moved to the trash",
                "{count} resources moved to the trash",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
        } else {
            ngettext(
                "{count} resource restored",
                "{count} resources restored",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
        };
        self.show_undoable_toast(&message.replace("{count}", &count.to_string()));
    }

    fn ask_to_empty_trash(&self) {
        let count = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.view_counts().ok())
            .map_or(0, |counts| counts.of(AssetView::Trash));
        if count == 0 {
            return;
        }
        if !self.settings().boolean(settings::CONFIRM_EMPTY_TRASH) {
            self.empty_trash();
            return;
        }
        let alert = adw::AlertDialog::new(
            Some(&gettext("Empty the Trash?")),
            Some(
                &ngettext(
                    "{count} resource will be deleted for good. This cannot be undone.",
                    "{count} resources will be deleted for good. This cannot be undone.",
                    u32::try_from(count).unwrap_or(u32::MAX),
                )
                .replace("{count}", &count.to_string()),
            ),
        );
        alert.add_responses(&[
            (CLOSE_RESPONSE, &gettext("_Cancel")),
            (EMPTY_TRASH_RESPONSE, &gettext("_Empty Trash")),
        ]);
        alert.set_response_appearance(EMPTY_TRASH_RESPONSE, adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some(CLOSE_RESPONSE));
        alert.set_close_response(CLOSE_RESPONSE);
        alert.connect_response(
            Some(EMPTY_TRASH_RESPONSE),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| window.empty_trash()
            ),
        );
        alert.present(Some(self));
    }

    fn empty_trash(&self) {
        let emptied = self.change_library(Library::empty_trash);
        if let Some(Err(error)) = emptied {
            self.show_library_error(&error);
        }
        self.refresh_assets();
    }

    fn change_library<R>(&self, change: impl FnOnce(&mut Library) -> R) -> Option<R> {
        let imp = self.imp();
        let (result, before, after) = {
            let mut library = imp.library.borrow_mut();
            let library = library.as_mut()?;
            let before = library.latest_change();
            let result = change(library);
            (result, before, library.latest_change())
        };
        imp.fresh_change.set(after.filter(|_| after != before));
        self.dismiss_stale_undo_toast(after);
        Some(result)
    }

    fn dismiss_stale_undo_toast(&self, latest: Option<ChangeStamp>) {
        let imp = self.imp();
        let stale = imp
            .undo_toast
            .borrow()
            .as_ref()
            .is_some_and(|(_, stamp)| latest != Some(*stamp));
        if stale && let Some((toast, _)) = imp.undo_toast.take() {
            toast.dismiss();
        }
    }

    fn show_undoable_toast(&self, text: &str) {
        let imp = self.imp();
        let Some(stamp) = imp.fresh_change.get() else {
            self.show_toast(text);
            return;
        };
        let toast = adw::Toast::new(text);
        toast.set_button_label(Some(&gettext("_Undo")));
        toast.connect_button_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.undo_change(stamp)
        ));
        toast.connect_dismissed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dismissed| {
                let imp = window.imp();
                let is_current = imp
                    .undo_toast
                    .borrow()
                    .as_ref()
                    .is_some_and(|(current, _)| current == dismissed);
                if is_current {
                    imp.undo_toast.take();
                }
            }
        ));
        if let Some((previous, _)) = imp.undo_toast.replace(Some((toast.clone(), stamp))) {
            previous.dismiss();
        }
        imp.toast_overlay.add_toast(toast);
    }

    fn forget_undo_toast(&self) {
        if let Some((toast, _)) = self.imp().undo_toast.take() {
            toast.dismiss();
        }
    }

    fn empty_expired_trash(&self) {
        if !self.settings().boolean(settings::AUTO_EMPTY_TRASH) {
            return;
        }
        match self.change_library(Library::empty_expired_trash) {
            Some(Ok(0)) | None => {}
            Some(Ok(count)) => {
                self.refresh_assets();
                let days = TRASH_RETENTION.as_secs() / SECONDS_PER_DAY;
                self.show_toast(
                    &ngettext(
                        "{count} resource older than {days} days deleted from the trash for good",
                        "{count} resources older than {days} days deleted from the trash for good",
                        u32::try_from(count).unwrap_or(u32::MAX),
                    )
                    .replace("{count}", &count.to_string())
                    .replace("{days}", &days.to_string()),
                );
            }
            Some(Err(error)) => self.show_library_error(&error),
        }
    }

    fn undo(&self) {
        if self.undo_typing() {
            return;
        }
        let undone = self.change_library(|library| {
            library
                .undo()
                .map(|change| change.map(|change| undo_message::describe(&change, library)))
        });
        self.show_undo_outcome(undone, true);
    }

    fn undo_change(&self, stamp: ChangeStamp) {
        let undone = self.change_library(|library| {
            library
                .undo_change(stamp)
                .map(|change| change.map(|change| undo_message::describe(&change, library)))
        });
        self.show_undo_outcome(undone, false);
    }

    fn show_undo_outcome(
        &self,
        undone: Option<Result<Option<String>, UndoError>>,
        tell_when_nothing: bool,
    ) {
        match undone {
            Some(Ok(Some(message))) => {
                self.refresh_sidebar();
                self.refresh_grid();
                self.show_toast(&message);
            }
            Some(Ok(None)) if tell_when_nothing => self.show_toast(&gettext("Nothing to undo")),
            Some(Ok(None)) | None => {}
            Some(Err(error)) => {
                self.refresh_sidebar();
                self.refresh_grid();
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Undo")),
                    Some(&error_messages::describe_undo(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
        }
    }

    fn copy_selected(&self) {
        if self.copy_typed_text() {
            return;
        }
        let selected = self.targeted_ids();
        if selected.is_empty() || self.is_showing_trash() {
            return;
        }
        let prepared = self.imp().library.borrow().as_ref().map(|library| {
            let copies = library.clipboard_copies(&selected)?;
            let first_name = library.asset(selected[0])?.map(|asset| asset.display_name);
            Ok::<_, LibraryError>((copies, first_name))
        });
        match prepared {
            Some(Ok((copies, first_name))) if !copies.is_empty() => {
                self.clipboard()
                    .set_content(Some(&clipboard_content::provider(&copies)))
                    .ok();
                let message = match (copies.len(), first_name) {
                    (1, Some(name)) => gettext("“{name}” copied").replace("{name}", &name),
                    (count, _) => ngettext(
                        "{count} resource copied",
                        "{count} resources copied",
                        u32::try_from(count).unwrap_or(u32::MAX),
                    )
                    .replace("{count}", &count.to_string()),
                };
                self.show_toast(&message);
            }
            Some(Err(error)) => self.show_library_error(&error),
            Some(Ok(_)) | None => {}
        }
    }

    fn copy_typed_text(&self) -> bool {
        self.activate_on_focused_text("clipboard.copy")
    }

    fn activate_on_focused_text(&self, action: &str) -> bool {
        GtkWindowExt::focus(self).is_some_and(|focus| {
            (focus.is::<gtk::Text>() || focus.is::<gtk::TextView>() || focus.is::<gtk::Label>())
                && focus.activate_action(action, None).is_ok()
        })
    }

    fn select_all(&self) {
        if self.select_all_typed_text() {
            return;
        }
        if let Some(selection) = self.imp().asset_grid.selection() {
            selection.select_all();
        }
    }

    fn select_all_typed_text(&self) -> bool {
        self.activate_on_focused_text("selection.select-all")
    }

    async fn export_selected(&self) {
        let selected = self.selected_ids();
        if selected.is_empty() || self.is_showing_trash() {
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Export To"))
            .accept_label(gettext("_Export"))
            .modal(true)
            .build();
        let Ok(folder) = dialog.select_folder_future(Some(self)).await else {
            return;
        };
        let Some(path) = folder.path() else {
            return;
        };
        let exported = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.export_to(&selected, &path));
        match exported {
            Some(Ok(copies)) => self.show_export_toast(copies.len(), &folder),
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Export")),
                    Some(&error_messages::describe(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
            None => {}
        }
    }

    async fn open_selected_with(&self) {
        let selected = self.selected_ids();
        let [asset] = selected[..] else {
            return;
        };
        if self.is_showing_trash() {
            return;
        }
        let prepared = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.opening_copy(asset));
        let copy = match prepared {
            Some(Ok(Some(copy))) => copy,
            Some(Err(error)) => {
                self.show_open_with_error(&error_messages::describe(&error));
                return;
            }
            Some(Ok(None)) | None => return,
        };
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(copy)));
        launcher.set_always_ask(true);
        if let Err(error) = launcher.launch_future(Some(self)).await
            && !is_dismissed(&error)
        {
            self.show_open_with_error(error.message());
        }
    }

    fn show_open_with_error(&self, details: &str) {
        let alert =
            adw::AlertDialog::new(Some(&gettext("Unable to Open the Resource")), Some(details));
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_export_toast(&self, count: usize, folder: &gio::File) {
        let name = folder
            .basename()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let toast = adw::Toast::new(
            &ngettext(
                "{count} resource exported to “{name}”",
                "{count} resources exported to “{name}”",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
            .replace("{count}", &count.to_string())
            .replace("{name}", &name),
        );
        toast.set_button_label(Some(&gettext("_Open Folder")));
        toast.connect_button_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[strong]
            folder,
            move |_| {
                gtk::FileLauncher::new(Some(&folder)).launch(
                    Some(&window),
                    gio::Cancellable::NONE,
                    |_| {},
                );
            }
        ));
        self.imp().toast_overlay.add_toast(toast);
    }

    fn undo_typing(&self) -> bool {
        GtkWindowExt::focus(self).is_some_and(|focus| {
            (focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
                && focus.activate_action("text.undo", None).is_ok()
        })
    }

    fn targeted_ids(&self) -> Vec<AssetId> {
        let imp = self.imp();
        if imp.window_stack.visible_child_name().as_deref() == Some(PREVIEW_PAGE) {
            return imp
                .asset_preview
                .shown_asset()
                .map(|asset| vec![asset.id()])
                .unwrap_or_default();
        }
        self.selected_ids()
    }

    fn selected_ids(&self) -> Vec<AssetId> {
        self.imp()
            .asset_grid
            .selected_assets()
            .iter()
            .map(PigouneAssetObject::id)
            .collect()
    }

    fn rename_asset(&self, asset: &PigouneAssetObject, name: &str) {
        let command = AssetCommand::Rename {
            asset: asset.id(),
            name: name.to_owned(),
        };
        if self.apply_asset_command(&command) {
            asset.set_display_name(name.trim());
            self.imp().asset_grid.resort();
        } else {
            asset.notify_display_name();
        }
    }

    fn change_asset_text(&self, asset: &PigouneAssetObject, field: TextField, value: &str) {
        let command = AssetCommand::SetText {
            asset: asset.id(),
            field,
            value: value.to_owned(),
        };
        if self.apply_asset_command(&command) {
            asset.set_text(field, value.trim());
        }
    }

    fn apply_asset_command(&self, command: &AssetCommand) -> bool {
        let applied = self.change_library(|library| library.apply_asset_command(command));
        match applied {
            Some(Ok(_)) => true,
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Change the Resource")),
                    Some(&error_messages::describe_asset(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
                false
            }
            None => false,
        }
    }

    fn refresh_selected_tags(&self) {
        let imp = self.imp();
        let selected = self.selected_ids();
        if selected.is_empty() {
            return;
        }
        let read = imp.library.borrow().as_ref().map(|library| {
            let mut carried: Vec<(Tag, usize)> = Vec::new();
            for asset in &selected {
                for tag in library.tags_of(*asset)? {
                    match carried.iter_mut().find(|(known, _)| known.id == tag.id) {
                        Some((_, count)) => *count += 1,
                        None => carried.push((tag, 1)),
                    }
                }
            }
            Ok::<_, LibraryError>((carried, library.tags()?))
        });
        if let Some(Ok((mut carried, all))) = read {
            carried.sort_by_key(|(tag, _)| tag.name.to_lowercase());
            let shared = carried
                .into_iter()
                .map(|(tag, carried_by)| SharedTag {
                    tag,
                    carried_by,
                    out_of: selected.len(),
                })
                .collect();
            imp.asset_details.show_tags(shared, all);
        }
    }

    fn add_tags_to_selected(&self, names: Vec<String>) {
        let selected = self.selected_ids();
        if selected.is_empty() {
            return;
        }
        let commands = names
            .into_iter()
            .map(|name| TagCommand::Add {
                assets: selected.clone(),
                name,
            })
            .collect();
        if self.apply_tag_command(&TagCommand::Batch(commands)) {
            self.refresh_selected_tags();
            self.refresh_sidebar();
        }
    }

    fn remove_tag_from_selected(&self, tag: TagId) {
        let imp = self.imp();
        let selected = self.selected_ids();
        if selected.is_empty() {
            return;
        }
        let command = TagCommand::Unlink {
            tag,
            assets: selected.clone(),
        };
        if !self.apply_tag_command(&command) {
            return;
        }
        self.refresh_selected_tags();
        self.refresh_sidebar();
        if imp.current_view.get() == AssetView::Tag(tag) {
            for asset in selected {
                imp.asset_grid.remove_asset(asset);
            }
            if imp.asset_grid.is_empty() {
                self.refresh_grid();
            }
        }
    }

    fn refresh_selected_collections(&self) {
        let selected = self.selected_ids().len();
        if let Some((shared, _)) = self.selected_collections() {
            self.imp().asset_details.show_collections(&shared, selected);
        }
    }

    fn ask_collection_for_selected(&self) {
        let Some((shared, all)) = self.selected_collections() else {
            return;
        };
        let held_by_all = shared
            .iter()
            .filter(|shared| shared.held_by == shared.out_of)
            .map(|shared| shared.path.id)
            .collect();
        collection_chooser::present(
            self,
            all,
            held_by_all,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |collection| window.change_selected_collections(|assets| {
                    CollectionCommand::AddAssets { collection, assets }
                })
            ),
        );
    }

    fn go_to_collection(&self, id: CollectionId) {
        let imp = self.imp();
        let reveal = imp.library.borrow().as_ref().map(|library| {
            let parent = library.collection(id).ok().flatten().and_then(|c| c.parent);
            ancestors(library, parent)
        });
        if let Some(reveal) = reveal {
            self.go_to_view(AssetView::Collection(id), reveal);
        }
    }

    fn go_to_view(&self, view: AssetView, reveal: Vec<CollectionId>) {
        let imp = self.imp();
        imp.current_view.set(view);
        self.remember_view(view);
        imp.asset_preview.close();
        self.refresh_sidebar_revealing(reveal);
        self.refresh_grid();
        imp.sidebar.point_out(view);
        imp.asset_grid.point_out_selected_next();
    }

    fn selected_collections(&self) -> Option<(Vec<SharedCollection>, Vec<CollectionPath>)> {
        let imp = self.imp();
        let selected = self.selected_ids();
        if selected.is_empty() {
            return None;
        }
        let read = imp.library.borrow().as_ref().map(|library| {
            let mut held: Vec<(CollectionId, usize)> = Vec::new();
            for asset in &selected {
                for collection in library.collections_of(*asset)? {
                    match held.iter_mut().find(|(known, _)| *known == collection) {
                        Some((_, count)) => *count += 1,
                        None => held.push((collection, 1)),
                    }
                }
            }
            Ok::<_, LibraryError>((held, library.collection_paths()?))
        });
        if let Some(Ok((held, mut all))) = read {
            all.sort_by_cached_key(|path| {
                path.names
                    .iter()
                    .map(|name| glib::FilenameCollationKey::from(name.as_str()))
                    .collect::<Vec<_>>()
            });
            let shared: Vec<SharedCollection> = all
                .iter()
                .filter_map(|path| {
                    let (_, held_by) = held.iter().find(|(id, _)| *id == path.id)?;
                    Some(SharedCollection {
                        path: path.clone(),
                        held_by: *held_by,
                        out_of: selected.len(),
                    })
                })
                .collect();
            Some((shared, all))
        } else {
            None
        }
    }

    fn change_selected_collections(&self, command: impl FnOnce(Vec<AssetId>) -> CollectionCommand) {
        let selected = self.selected_ids();
        if !selected.is_empty() {
            self.apply_collection_change(&command(selected.clone()), &selected);
        }
    }

    fn apply_collection_change(&self, command: &CollectionCommand, assets: &[AssetId]) -> bool {
        let applied = self.change_library(|library| library.apply_collection_command(command));
        match applied {
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Change the Collections")),
                    Some(&error_messages::describe_collection(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
                return false;
            }
            None => return false,
        }
        self.refresh_sidebar();
        self.drop_assets_leaving_view(assets);
        true
    }

    fn describe_hovered_drop(&self, hovered: Option<&HoveredDrop>) {
        let imp = self.imp();
        let key = hovered.map(|hovered| (hovered.view, hovered.keep_source));
        if imp.hovered_drop.replace(key) == key {
            return;
        }
        let text = hovered.and_then(|hovered| {
            let library = imp.library.borrow();
            drop_message::describe(hovered, imp.current_view.get(), library.as_ref()?)
        });
        imp.asset_grid.show_drag_caption(text.as_deref());
    }

    fn drop_assets_on(&self, target: AssetView, assets: &[AssetId], keep_source: bool) {
        if self.is_showing_trash() {
            return;
        }
        match target {
            AssetView::Trash => self.set_trashed(assets, true),
            AssetView::Collection(to) => self.drop_assets_on_collection(to, assets, keep_source),
            AssetView::Tag(tag) => self.drop_assets_on_tag(tag, assets),
            AssetView::All | AssetView::Favorites | AssetView::Unclassified => {}
        }
    }

    fn drop_assets_on_collection(&self, to: CollectionId, assets: &[AssetId], keep_source: bool) {
        let count = assets.len();
        let (command, message) = match self.imp().current_view.get() {
            AssetView::Collection(from) if from == to => return,
            AssetView::Collection(from) if !keep_source => (
                CollectionCommand::MoveAssets {
                    from,
                    to,
                    assets: assets.to_vec(),
                },
                ngettext(
                    "{count} resource moved to “{name}”",
                    "{count} resources moved to “{name}”",
                    u32::try_from(count).unwrap_or(u32::MAX),
                ),
            ),
            _ => (
                CollectionCommand::AddAssets {
                    collection: to,
                    assets: assets.to_vec(),
                },
                ngettext(
                    "{count} resource added to “{name}”",
                    "{count} resources added to “{name}”",
                    u32::try_from(count).unwrap_or(u32::MAX),
                ),
            ),
        };
        let name = self.collection_name(to).unwrap_or_default();
        if self.apply_collection_change(&command, assets) {
            self.show_undoable_toast(
                &message
                    .replace("{count}", &count.to_string())
                    .replace("{name}", &name),
            );
        }
    }

    fn drop_assets_on_tag(&self, tag: TagId, assets: &[AssetId]) {
        let count = assets.len();
        if !self.apply_tag_command(&TagCommand::Link {
            tag,
            assets: assets.to_vec(),
        }) {
            return;
        }
        self.refresh_selected_tags();
        self.refresh_sidebar();
        self.drop_assets_leaving_view(assets);
        self.show_undoable_toast(
            &ngettext(
                "Tag “{name}” added to {count} resource",
                "Tag “{name}” added to {count} resources",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
            .replace("{count}", &count.to_string())
            .replace("{name}", &self.tag_name(tag).unwrap_or_default()),
        );
    }

    fn drop_collection(&self, dragged: CollectionId, drop: CollectionDrop) {
        let imp = self.imp();
        let order = self.collection_order();
        let custom_order_shown = order.criterion == CollectionCriterion::Custom && !order.reversed;
        let collections = match imp
            .library
            .borrow()
            .as_ref()
            .map(Library::visible_collections)
        {
            Some(Ok(collections)) => collections,
            Some(Err(error)) => {
                self.show_library_error(&error);
                return;
            }
            None => return,
        };
        let tree = CollectionTree::new(collections, order, |name: &str| {
            glib::FilenameCollationKey::from(name)
        });
        let Some(plan) = collection_drop::plan(&tree, dragged, drop, custom_order_shown) else {
            return;
        };
        let applied = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Batch(plan.commands))?;
            let parent = library.collection(dragged)?.and_then(|moved| moved.parent);
            Ok::<_, pigoune_core::CollectionError>(ancestors(library, parent))
        });
        let reveal = match applied {
            Some(Ok(reveal)) => reveal,
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Move the Collection")),
                    Some(&error_messages::describe_collection(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
                return;
            }
            None => return,
        };
        if plan.switches_to_custom_order {
            settings::store_bool(self.settings(), settings::COLLECTION_SORT_REVERSED, false);
            settings::store_string(self.settings(), settings::COLLECTION_SORT, "custom");
        }
        self.refresh_sidebar_revealing(reveal);
        self.refresh_grid();
    }

    fn collection_name(&self, id: CollectionId) -> Option<String> {
        self.imp()
            .library
            .borrow()
            .as_ref()?
            .collection(id)
            .ok()?
            .map(|collection| collection.name)
    }

    fn show_toast(&self, text: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(text));
    }

    fn drop_assets_leaving_view(&self, assets: &[AssetId]) {
        let imp = self.imp();
        let view = imp.current_view.get();
        let leaving: Vec<AssetId> = imp
            .library
            .borrow()
            .as_ref()
            .map(|library| {
                assets
                    .iter()
                    .copied()
                    .filter(|asset| !library.view_contains(view, *asset).unwrap_or(true))
                    .collect()
            })
            .unwrap_or_default();
        if leaving.is_empty() {
            return;
        }
        for asset in leaving {
            imp.asset_grid.remove_asset(asset);
        }
        if imp.asset_grid.is_empty() {
            self.refresh_grid();
        }
    }

    fn tag_imported(&self, tag: TagId, summary: &ImportSummary) {
        let assets: Vec<_> = summary
            .imported
            .iter()
            .chain(&summary.already_known)
            .copied()
            .collect();
        if assets.is_empty() {
            return;
        }
        let tagged = self.change_library(|library| library.tag_imported(tag, &assets));
        if let Some(Err(error)) = tagged {
            self.show_tag_error(&error);
        }
    }

    fn apply_tag_command(&self, command: &TagCommand) -> bool {
        let applied = self.change_library(|library| library.apply_tag_command(command));
        match applied {
            Some(Ok(_)) => true,
            Some(Err(error)) => {
                self.show_tag_error(&error);
                false
            }
            None => false,
        }
    }

    fn show_tag_error(&self, error: &TagError) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Change the Tags")),
            Some(&error_messages::describe_tag(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn tag_name(&self, tag: TagId) -> Option<String> {
        self.imp()
            .library
            .borrow()
            .as_ref()?
            .tags()
            .ok()?
            .into_iter()
            .find(|candidate| candidate.id == tag)
            .map(|candidate| candidate.name)
    }

    fn ask_tag_name(&self, tag: TagId) {
        let Some(current_name) = self.tag_name(tag) else {
            return;
        };
        let dialog = PigouneCollectionNameDialog::new(
            &gettext("Rename Tag"),
            &gettext("_Rename"),
            &current_name,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name| window.rename_tag(tag, name)
            ),
        );
        dialog.present(Some(self));
    }

    fn rename_tag(&self, tag: TagId, name: &str) -> Result<(), String> {
        let renamed = self.change_library(|library| {
            library.apply_tag_command(&TagCommand::Rename {
                tag,
                name: name.to_owned(),
            })
        });
        match renamed {
            Some(Ok(_)) => {
                self.refresh_assets();
                self.refresh_selected_tags();
                Ok(())
            }
            Some(Err(TagError::NameTaken(existing))) => {
                self.offer_merge(tag, existing);
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_tag(&error)),
            None => Ok(()),
        }
    }

    fn offer_merge(&self, from: TagId, into: TagId) {
        let (Some(from_name), Some(into_name)) = (self.tag_name(from), self.tag_name(into)) else {
            return;
        };
        let alert = adw::AlertDialog::new(
            Some(&gettext("Merge the Tags?")),
            Some(
                &gettext("The tag “{into}” already exists. Merge “{from}” into “{into}”?")
                    .replace("{into}", &into_name)
                    .replace("{from}", &from_name),
            ),
        );
        alert.add_responses(&[
            (CLOSE_RESPONSE, &gettext("_Cancel")),
            (MERGE_RESPONSE, &gettext("_Merge")),
        ]);
        alert.set_response_appearance(MERGE_RESPONSE, adw::ResponseAppearance::Suggested);
        alert.set_default_response(Some(MERGE_RESPONSE));
        alert.set_close_response(CLOSE_RESPONSE);
        alert.connect_response(
            Some(MERGE_RESPONSE),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| {
                    if window.apply_tag_command(&TagCommand::Merge { from, into }) {
                        if window.imp().current_view.get() == AssetView::Tag(from) {
                            window.imp().current_view.set(AssetView::Tag(into));
                        }
                        window.refresh_assets();
                        window.refresh_selected_tags();
                    }
                }
            ),
        );
        alert.present(Some(self));
    }

    fn ask_tag_deletion(&self, tag: TagId) {
        let Some(name) = self.tag_name(tag) else {
            return;
        };
        let used_by = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.view_counts().ok())
            .map_or(0, |counts| counts.of(AssetView::Tag(tag)));
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete the Tag “{name}”?").replace("{name}", &name)),
            Some(
                &ngettext(
                    "It will be removed from {count} resource. The resources stay in the library.",
                    "It will be removed from {count} resources. The resources stay in the library.",
                    u32::try_from(used_by).unwrap_or(u32::MAX),
                )
                .replace("{count}", &used_by.to_string()),
            ),
        );
        alert.add_responses(&[
            (CLOSE_RESPONSE, &gettext("_Cancel")),
            (DELETE_RESPONSE, &gettext("_Delete")),
        ]);
        alert.set_response_appearance(DELETE_RESPONSE, adw::ResponseAppearance::Destructive);
        alert.set_close_response(CLOSE_RESPONSE);
        alert.connect_response(
            Some(DELETE_RESPONSE),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| {
                    if window.apply_tag_command(&TagCommand::Delete { tag }) {
                        window.refresh_assets();
                        window.refresh_selected_tags();
                    }
                }
            ),
        );
        alert.present(Some(self));
    }

    fn connect_preview(&self) {
        let imp = self.imp();
        imp.asset_grid.connect_preview_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |position| window.open_preview(position)
        ));
        imp.asset_preview.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |last| window.leave_preview(last.as_ref())
        ));
    }

    fn open_preview(&self, activated: Option<u32>) {
        let imp = self.imp();
        let selected = imp.asset_grid.selected_assets();
        let visible = imp.asset_grid.visible_assets();
        let activated = activated
            .and_then(|position| usize::try_from(position).ok())
            .and_then(|index| visible.get(index))
            .cloned();
        let browsing_selection = selected.len() > 1;
        let items = if browsing_selection {
            selected
        } else {
            visible
        };
        let start = activated.or_else(|| imp.asset_grid.selected_assets().first().cloned());
        let Some(start) = start else {
            return;
        };
        let position = items
            .iter()
            .position(|asset| asset.id() == start.id())
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0);
        imp.browsing_selection.set(browsing_selection);
        imp.window_stack.set_visible_child_name(PREVIEW_PAGE);
        imp.asset_preview
            .open(items, position, imp.asset_grid.thumbnails());
    }

    fn leave_preview(&self, last: Option<&PigouneAssetObject>) {
        let imp = self.imp();
        if imp.window_stack.visible_child_name().as_deref() != Some(PREVIEW_PAGE) {
            return;
        }
        imp.window_stack.set_visible_child_name(MAIN_PAGE);
        match last {
            Some(last) if imp.browsing_selection.get() => imp.asset_grid.reveal_asset(last.id()),
            Some(last) => imp.asset_grid.select_asset(last.id()),
            None => imp.asset_grid.reveal_selected(),
        }
        imp.asset_grid.focus_selected_later();
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
            Ok::<_, LibraryError>((
                library.visible_collections()?,
                library.view_counts()?,
                library.tags()?,
            ))
        });
        let (collections, counts, tags) = match read {
            Some(Ok(read)) => read,
            Some(Err(error)) => {
                self.show_library_error(&error);
                return;
            }
            None => return,
        };
        let still_exists = match imp.current_view.get() {
            AssetView::Collection(id) => collections.iter().any(|collection| collection.id == id),
            AssetView::Tag(id) => tags.iter().any(|tag| tag.id == id),
            AssetView::All | AssetView::Favorites | AssetView::Unclassified | AssetView::Trash => {
                true
            }
        };
        if !still_exists {
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
            tags,
        });
        self.refresh_selected_collections();
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
        let created = self.change_library(|library| {
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

    fn ask_collection_deletion(&self, id: CollectionId) {
        let read = self.imp().library.borrow().as_ref().map(|library| {
            Ok::<_, pigoune_core::CollectionError>((
                library.removal_of(id)?,
                library.view_counts()?.of(AssetView::Collection(id)),
            ))
        });
        let (removal, contained) = match read {
            Some(Ok(read)) => read,
            Some(Err(error)) => {
                self.show_collection_error(&error);
                return;
            }
            None => return,
        };
        let name = self.collection_name(id).unwrap_or_default();
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete the Collection “{name}”?").replace("{name}", &name)),
            Some(&deletion_consequences(removal, contained)),
        );
        alert.add_responses(&[
            (CLOSE_RESPONSE, &gettext("_Cancel")),
            (DELETE_RESPONSE, &gettext("_Delete")),
        ]);
        alert.set_response_appearance(DELETE_RESPONSE, adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some(CLOSE_RESPONSE));
        alert.set_close_response(CLOSE_RESPONSE);
        alert.connect_response(
            Some(DELETE_RESPONSE),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| window.delete_collection(id, &name)
            ),
        );
        alert.present(Some(self));
    }

    fn delete_collection(&self, id: CollectionId, name: &str) {
        let deleted = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Trash { id })
        });
        match deleted {
            Some(Ok(_)) => {
                self.refresh_assets();
                self.show_undoable_toast(
                    &gettext("Collection “{name}” deleted").replace("{name}", name),
                );
            }
            Some(Err(error)) => self.show_collection_error(&error),
            None => {}
        }
    }

    fn show_collection_error(&self, error: &pigoune_core::CollectionError) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Change the Collections")),
            Some(&error_messages::describe_collection(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
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
        let renamed = self.change_library(|library| {
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
        let previously_selected = self.selected_ids();
        let filter = AssetFilter {
            text: imp.search_query.borrow().clone(),
            ..imp.filters.borrow().clone()
        };
        let read = imp.library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((
                asset_objects(library, view, &filter)?,
                library.view_counts()?.all,
                view_name(library, view),
            ))
        });
        match read {
            Some(Ok((assets, library_total, view_name))) => {
                let page = if library_total == 0 && view != AssetView::Trash {
                    EMPTY_PAGE
                } else {
                    ASSETS_PAGE
                };
                let searching = filter.narrows();
                if assets.is_empty() && searching {
                    self.describe_missing_results(view, &view_name, &filter);
                } else if assets.is_empty() {
                    self.describe_empty_view(view, &view_name);
                }
                imp.asset_grid.show_nothing(assets.is_empty());
                imp.grid_header
                    .show_result_count(searching.then_some(assets.len()));
                imp.asset_grid.show_assets(&assets);
                if !previously_selected.is_empty() {
                    imp.asset_grid.select_assets(&previously_selected);
                }
                imp.library_stack.set_visible_child_name(page);
                imp.grid_header
                    .details_button()
                    .set_visible(!assets.is_empty());
                self.update_details_panel();
                imp.trash_banner.set_revealed(view == AssetView::Trash);
            }
            Some(Err(error)) => {
                imp.asset_grid.show_assets(&[]);
                imp.library_stack.set_visible_child_name(EMPTY_PAGE);
                imp.grid_header.details_button().set_visible(false);
                self.show_library_error(&error);
            }
            None => {}
        }
    }

    fn toggle_favorite(&self) {
        let imp = self.imp();
        let selected = imp.asset_grid.selected_assets();
        if selected.is_empty() || self.is_showing_trash() {
            return;
        }
        let favorite = !selected.iter().all(PigouneAssetObject::favorite);
        let applied = self.change_library(|library| {
            library.apply_asset_command(&AssetCommand::SetFavorite {
                assets: selected.iter().map(PigouneAssetObject::id).collect(),
                favorite,
            })
        });
        match applied {
            Some(Ok(_)) => {
                for asset in &selected {
                    asset.set_favorite(favorite);
                }
                self.refresh_sidebar();
                if imp.current_view.get() == AssetView::Favorites && !favorite {
                    for asset in &selected {
                        imp.asset_grid.remove_asset(asset.id());
                    }
                    if imp.asset_grid.is_empty() {
                        self.refresh_grid();
                    }
                } else if selected.len() > 1 {
                    imp.asset_details
                        .show_group(&selected, &imp.asset_grid.thumbnails());
                }
            }
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Change the Favorites")),
                    Some(&error_messages::describe_asset(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
            None => {}
        }
    }

    fn describe_missing_results(&self, view: AssetView, view_name: &str, filter: &AssetFilter) {
        let scope = match view {
            AssetView::All => None,
            AssetView::Favorites => Some(gettext("Favorites")),
            AssetView::Unclassified => Some(gettext("Unclassified")),
            AssetView::Trash => Some(gettext("Trash")),
            AssetView::Collection(_) | AssetView::Tag(_) => Some(view_name.to_owned()),
        };
        let description = match scope {
            _ if filter.chosen_filters() > 0 => {
                gettext("No resource matches the chosen filters. Clear them to see more resources.")
            }
            Some(scope) => gettext(
                "No resource in “{name}” matches this search. Select “All” to search everywhere.",
            )
            .replace("{name}", &scope),
            None => gettext("No resource in the library matches this search."),
        };
        let page = self.imp().asset_grid.nothing_page();
        page.set_icon_name(Some("system-search-symbolic"));
        let query = filter.text.trim();
        if query.is_empty() {
            page.set_title(&gettext("No Results"));
        } else {
            page.set_title(&gettext("No Results for “{query}”").replace("{query}", query));
        }
        page.set_description(Some(&description));
    }

    fn describe_empty_view(&self, view: AssetView, view_name: &str) {
        let page = self.imp().asset_grid.nothing_page();
        page.set_icon_name(Some(if view == AssetView::Trash {
            "user-trash-symbolic"
        } else {
            "folder-symbolic"
        }));
        if view == AssetView::Trash {
            page.set_title(&gettext("The Trash Is Empty"));
            page.set_description(Some(&gettext(
                "Resources moved to the trash can be restored from here.",
            )));
        } else if let AssetView::Tag(_) = view {
            page.set_title(&gettext("No Resource Tagged “{name}”").replace("{name}", view_name));
            page.set_description(Some(&gettext(
                "Add this tag to resources from the details panel, or drop files on it.",
            )));
        } else if view == AssetView::Favorites {
            page.set_title(&gettext("No Favorites"));
            page.set_description(Some(&gettext(
                "Mark a resource as a favorite with the star in the details panel or with Ctrl+D.",
            )));
        } else if view == AssetView::Unclassified {
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
            move |_, drop| {
                window.imp().library.borrow().is_some()
                    && drop.drag().is_none()
                    && drop.formats().contains_type(gdk::FileList::static_type())
            }
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
        let target = match self.target_collection() {
            Some(id) => ImportTarget::Collection(id),
            None => ImportTarget::Nowhere,
        };
        self.import_paths_into(paths, target).await;
    }

    async fn import_paths_into(&self, paths: Vec<PathBuf>, target: ImportTarget) {
        if paths.is_empty() {
            return;
        }
        let destination_name =
            self.imp()
                .library
                .borrow()
                .as_ref()
                .and_then(|library| match target {
                    ImportTarget::Collection(id) => {
                        library.collection(id).ok().flatten().map(|c| c.name)
                    }
                    ImportTarget::Tag(id) => library
                        .tags()
                        .ok()?
                        .into_iter()
                        .find(|tag| tag.id == id)
                        .map(|tag| tag.name),
                    ImportTarget::Nowhere => None,
                });
        let collection = match target {
            ImportTarget::Collection(id) => Some(id),
            ImportTarget::Tag(_) | ImportTarget::Nowhere => None,
        };
        let Some(library) = self.imp().library.take() else {
            return;
        };

        self.set_importing(true);
        let finished = background_import::run(self, library, paths.clone(), collection).await;
        self.set_importing(false);

        if let Some(FinishedImport { library, result }) = finished {
            self.imp().library.replace(Some(library));
            if let (ImportTarget::Tag(tag), Ok(summary)) = (target, &result) {
                self.tag_imported(tag, summary);
            }
            self.refresh_assets();
            let destination = destination_name.as_deref().map(|name| match target {
                ImportTarget::Tag(_) => Destination::Tag(name),
                ImportTarget::Collection(_) | ImportTarget::Nowhere => {
                    Destination::Collection(name)
                }
            });
            self.report_import(result, &paths, destination);
        } else {
            self.close_library();
            import_report::unexpected_stop_dialog().present(Some(self));
        }
    }

    fn report_import(
        &self,
        result: Result<ImportSummary, ImportError>,
        chosen: &[PathBuf],
        destination: Option<Destination>,
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

fn tag_parameter(parameter: Option<&glib::Variant>) -> Option<TagId> {
    TagId::parse(&parameter?.get::<String>()?)
}

#[derive(Debug, Clone, Copy)]
enum ImportTarget {
    Nowhere,
    Collection(CollectionId),
    Tag(TagId),
}

fn collection_parameter(parameter: Option<&glib::Variant>) -> Option<CollectionId> {
    CollectionId::parse(&parameter?.get::<String>()?)
}

fn deletion_consequences(removal: CollectionRemoval, contained: usize) -> String {
    let count = |number: usize| u32::try_from(number).unwrap_or(u32::MAX);
    let mut sentences = Vec::new();
    if removal.sub_collections > 0 {
        sentences.push(
            ngettext(
                "Its {count} sub-collection will be deleted too.",
                "Its {count} sub-collections will be deleted too.",
                count(removal.sub_collections),
            )
            .replace("{count}", &removal.sub_collections.to_string()),
        );
    }
    if removal.trashed_assets > 0 {
        sentences.push(
            ngettext(
                "{count} resource found only here will go to the trash.",
                "{count} resources found only here will go to the trash.",
                count(removal.trashed_assets),
            )
            .replace("{count}", &removal.trashed_assets.to_string()),
        );
    }
    if contained > removal.trashed_assets {
        sentences.push(gettext(
            "Resources also kept in other collections stay there.",
        ));
    }
    if sentences.is_empty() {
        sentences.push(gettext("This collection is empty."));
    }
    sentences.join(" ")
}

fn is_dismissed(error: &glib::Error) -> bool {
    matches!(
        error.kind::<gtk::DialogError>(),
        Some(gtk::DialogError::Dismissed | gtk::DialogError::Cancelled)
    ) || error.matches(gio::IOErrorEnum::Cancelled)
}

fn menu_item(label: &str, action: &str, accel: Option<&str>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    if let Some(accel) = accel {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    item
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
        AssetView::Tag(id) => library
            .tags()
            .ok()
            .and_then(|tags| tags.into_iter().find(|tag| tag.id == id))
            .map_or_else(|| library.name(), |tag| tag.name),
        AssetView::All | AssetView::Favorites | AssetView::Unclassified | AssetView::Trash => {
            library.name()
        }
    }
}

fn asset_objects(
    library: &Library,
    view: AssetView,
    filter: &AssetFilter,
) -> Result<Vec<PigouneAssetObject>, LibraryError> {
    Ok(library
        .find_assets_in(view, filter)?
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
