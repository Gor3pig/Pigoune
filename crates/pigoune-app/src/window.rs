use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gdk, gio, glib, graphene};
use pigoune_core::{
    AdoptError, AnnotatedError, AnnotationRender, AssetCommand, AssetError, AssetFilter,
    AssetFormat, AssetId, AssetView, ChangeStamp, CollectionCommand, CollectionId, CollectionLook,
    CollectionPath, CollectionRemoval, ConfirmedRemoval, Dimensions, DuplicateError, DuplicateKind,
    FlatFiles, HealthPlan, IgnoreId, IgnoredEntry, ImportError, ImportSummary, Library,
    LibraryError, LibraryExportReport, RemovalError, RgbaImage, SIMILAR_DISTANCE, SimilarSearch,
    SmartCollection, SmartCollectionCommand, SmartCollectionId, TRASH_RETENTION, Tag, TagCommand,
    TagError, TagId, TextField, UndoError, VisualAnalysis, VisualDetail, dominant_colors,
    library_display_name, visual_analysis,
};

use crate::analysis_pace;
use crate::annotated_files;
use crate::asset_colors;
use crate::asset_details::AnnotationAccess;
use crate::asset_facts;
use crate::asset_object::{AssetEntry, PigouneAssetObject};
use crate::background_import::{self, FinishedImport};
use crate::capture_draft;
use crate::clipboard_content;
use crate::collection_choice;
use crate::collection_chooser;
use crate::collection_drop::{self, CollectionDrop};
use crate::collection_look_dialog::PigouneCollectionLookDialog;
use crate::collection_name_dialog::PigouneCollectionNameDialog;
use crate::collection_places::SharedCollection;
use crate::collection_sort::{CollectionCriterion, CollectionOrder, CollectionTree};
use crate::conversion_memory;
use crate::conversion_report;
use crate::displayed_view;
use crate::drag_export;
use crate::drop_action;
use crate::drop_message;
use crate::drop_places;
use crate::dropped_content::{self, Dropped, DroppedImage};
use crate::duplicates_page::{
    Attachments, DuplicateActions, DuplicateMember, IgnoringActions, PlannedSearch, RemovalFailure,
    Removed,
};
use crate::editor::EditedCapture;
use crate::error_messages;
use crate::export_as_dialog::PigouneExportAsDialog;
use crate::export_outcome::{self, Outcome as ExportOutcome};
use crate::flatpak_updates::FlatpakUpdates;
use crate::grid_header::SearchScope;
use crate::health_page::{Adopted, HealthActions, Refusal, Replaced, Trashed};
use crate::host_path;
use crate::image_conversion::{self, ConversionSettings};
use crate::import_report;
use crate::library_info_dialog::{LibraryReport, PigouneLibraryInfoDialog};
use crate::mask_alert::{self, MaskChoice, Operation, Scope};
use crate::new_library_dialog::PigouneNewLibraryDialog;
use crate::pasted_content::{self, Pasted};
use crate::preferences_dialog;
use crate::preview_flight::Flight;
use crate::recent_libraries;
use crate::screen_size::{self, ScreenChoice};
use crate::screenshot::{self, ScreenshotOutcome};
use crate::settings;
use crate::shortcuts;
use crate::sidebar::{FoldedSections, HoveredDrop, SidebarContent};
use crate::sidebar_item::SidebarEntry;
use crate::similar_dialog::{PigouneSimilarDialog, SimilarEntry, SimilarOutcome};
use crate::smart_collection_dialog::{
    PigouneSmartCollectionDialog, SmartCollectionDraft, free_name,
};
use crate::smart_collection_sort::{self, SmartCollectionCriterion, SmartCollectionOrder};
use crate::tag_chooser;
use crate::tag_editor::SharedTag;
use crate::tag_tree;
use crate::thumbnails::{self, THUMBNAIL_PIXELS};
use crate::toasts;
use crate::undo_message::{self, Outcome};
use crate::update_banner;
use crate::update_news::UpdateNews;
use crate::view_setting;
use crate::wallpaper::{self, WallpaperResponse};
use crate::wallpaper_dialog::{PigouneWallpaperDialog, WallpaperChoice, WallpaperSource};
use crate::wallpaper_progress_dialog::PigouneWallpaperProgressDialog;
use crate::zoom_math::Size;

const WELCOME_PAGE: &str = "welcome";
const LIBRARY_PAGE: &str = "library";
const EMPTY_PAGE: &str = "empty";
const ASSETS_PAGE: &str = "assets";
const MAIN_PAGE: &str = "main";
const PREVIEW_PAGE: &str = "preview";
const EDITOR_PAGE: &str = "editor";
const OPENING_FLIGHT_MILLISECONDS: u32 = 250;
const CLOSING_FLIGHT_MILLISECONDS: u32 = 200;
const COUNTS_REFRESH_DELAY: std::time::Duration = std::time::Duration::from_millis(350);
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
const CUSTOMIZE_COLLECTION_ACTION: &str = "win.customize-collection";
const MOVE_COLLECTION_ACTION: &str = "win.move-collection";
const TOGGLE_FAVORITE_ACTION: &str = "win.toggle-favorite";
const RENAME_TAG_ACTION: &str = "win.rename-tag";
const OPEN_PREVIEW_ACTION: &str = "win.open-preview";
const OPEN_WITH_ACTION: &str = "win.open-with";
const SHOW_IN_FILES_ACTION: &str = "win.show-in-files";
const SET_WALLPAPER_ACTION: &str = "win.set-wallpaper";
const FIND_SIMILAR_ACTION: &str = "win.find-similar";
const FALLBACK_SCREEN: (u32, u32) = (1920, 1080);
const RENAME_ASSET_ACTION: &str = "win.rename-asset";
const ADD_TAG_ACTION: &str = "win.add-tag";
const ADD_TO_COLLECTION_ACTION: &str = "win.add-to-collection";
const MOVE_TO_COLLECTION_ACTION: &str = "win.move-to-collection";
const REMOVE_FROM_COLLECTION_ACTION: &str = "win.remove-from-collection";
const REMOVE_FROM_ALL_COLLECTIONS_ACTION: &str = "win.remove-from-all-collections";
const TRASH_SELECTED_ACTION: &str = "win.trash-selected";
const RESTORE_SELECTED_ACTION: &str = "win.restore-selected";
const EMPTY_TRASH_ACTION: &str = "win.empty-trash";
const EMPTY_TRASH_RESPONSE: &str = "empty";
const DELETE_TAG_ACTION: &str = "win.delete-tag";
const NEW_SUB_TAG_ACTION: &str = "win.new-sub-tag";
const MOVE_TAG_ACTION: &str = "win.move-tag";
const MERGE_TAG_ACTION: &str = "win.merge-tag";
const NEW_SMART_COLLECTION_ACTION: &str = "win.new-smart-collection";
const EDIT_SMART_COLLECTION_ACTION: &str = "win.edit-smart-collection";
const DELETE_SMART_COLLECTION_ACTION: &str = "win.delete-smart-collection";
const DELETE_COLLECTION_ACTION: &str = "win.delete-collection";
const UNDO_ACTION: &str = "win.undo";
const REDO_ACTION: &str = "win.redo";
const CLOSE_WINDOW_ACTION: &str = "win.close-window";
const PREFERENCES_ACTION: &str = "win.preferences";
const LIBRARY_INFO_ACTION: &str = "win.library-info";
const OPEN_RECENT_LIBRARY_ACTION: &str = "win.open-recent-library";
const CLEAR_RECENT_LIBRARIES_ACTION: &str = "win.clear-recent-libraries";
const LIBRARY_ENTRIES: i32 = 3;
const RECENT_LIBRARIES_POSITION: i32 = 2;
const WELCOME_MINIMUM_HEIGHT: i32 = 480;
const LIBRARY_MINIMUM_HEIGHT: i32 = 294;
const SEARCH_ACTION: &str = "win.search";
const COPY_SELECTED_ACTION: &str = "win.copy-selected";
const PASTE_ACTION: &str = "win.paste";
const PASTE_AND_ANNOTATE_ACTION: &str = "win.paste-and-annotate";
const CAPTURE_ACTION: &str = "win.capture";
const CAPTURE_SETTLE: std::time::Duration = std::time::Duration::from_millis(350);
const LARGEST_CAPTURE_SIDE: i32 = 8000;
const WARMED_RENDERINGS: usize = 8;
const EXPORT_SELECTED_ACTION: &str = "win.export-selected";
const EXPORT_SELECTED_AS_ACTION: &str = "win.export-selected-as";
const SELECT_ALL_ACTION: &str = "win.select-all";
const ANNOTATE_ACTION: &str = "win.annotate";
const VIEW_ORIGINAL_ACTION: &str = "win.view-original";
const IMPORT_ACTIONS: [&str; 3] = [IMPORT_FILES_ACTION, IMPORT_FOLDER_ACTION, PASTE_ACTION];
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
const COLOR_BATCH: usize = 50;
const MOST_SIMILAR_SHOWN: usize = 50;
const OPEN_LIBRARY_ACTIONS: [&str; 30] = [
    PASTE_AND_ANNOTATE_ACTION,
    ANNOTATE_ACTION,
    VIEW_ORIGINAL_ACTION,
    SET_WALLPAPER_ACTION,
    FIND_SIMILAR_ACTION,
    SEARCH_ACTION,
    LIBRARY_INFO_ACTION,
    UNDO_ACTION,
    REDO_ACTION,
    CLOSE_LIBRARY_ACTION,
    IMPORT_FILES_ACTION,
    IMPORT_FOLDER_ACTION,
    PASTE_ACTION,
    ENLARGE_THUMBNAILS_ACTION,
    SHRINK_THUMBNAILS_ACTION,
    NEW_COLLECTION_ACTION,
    NEW_SUBCOLLECTION_ACTION,
    RENAME_COLLECTION_ACTION,
    CUSTOMIZE_COLLECTION_ACTION,
    MOVE_COLLECTION_ACTION,
    DELETE_COLLECTION_ACTION,
    TOGGLE_FAVORITE_ACTION,
    RENAME_TAG_ACTION,
    DELETE_TAG_ACTION,
    NEW_SUB_TAG_ACTION,
    MOVE_TAG_ACTION,
    MERGE_TAG_ACTION,
    NEW_SMART_COLLECTION_ACTION,
    EDIT_SMART_COLLECTION_ACTION,
    DELETE_SMART_COLLECTION_ACTION,
];

const IMAGE_MIME_TYPES: [&str; 13] = [
    "image/svg+xml",
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/avif",
    "image/heic",
    "image/heif",
    "image/jxl",
    "image/gif",
    "image/tiff",
    "image/bmp",
    "image/vnd.microsoft.icon",
    "image/x-icon",
];

const CLOSE_RESPONSE: &str = "close";
const CAPTURE_TAG: &str = "capture";
const DEFAULT_VECTOR_EXPORT_PIXELS: u32 = 512;
const OPEN_ANOTHER_RESPONSE: &str = "open-another";
const RETRY_RESPONSE: &str = "retry";
const MERGE_RESPONSE: &str = "merge";
const DELETE_RESPONSE: &str = "delete";
const KEEP_SUB_TAGS_RESPONSE: &str = "keep-sub-tags";

#[derive(Debug, Clone, Copy)]
pub enum ToastOffer {
    Undo(ChangeStamp),
    Redo(ChangeStamp),
}

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
    use pigoune_core::{AssetFilter, AssetView, ChangeStamp, Dimensions, Library};

    use crate::asset_details::PigouneAssetDetails;
    use crate::asset_grid::PigouneAssetGrid;
    use std::rc::Rc;

    use crate::asset_preview::PigouneAssetPreview;
    use crate::editor::PigouneEditor;
    use crate::flatpak_updates::FlatpakUpdates;
    use crate::grid_header::PigouneGridHeader;
    use crate::preview_flight::PigounePreviewFlight;
    use crate::query_pills::PigouneQueryPills;
    use crate::sidebar::PigouneSidebar;
    use crate::update_news::UpdateNews;

    use super::{
        ADD_TAG_ACTION, ADD_TO_COLLECTION_ACTION, ANNOTATE_ACTION, CAPTURE_ACTION,
        CLEAR_RECENT_LIBRARIES_ACTION, CLOSE_LIBRARY_ACTION, CLOSE_WINDOW_ACTION,
        COPY_SELECTED_ACTION, CREATE_LIBRARY_ACTION, CUSTOMIZE_COLLECTION_ACTION,
        DELETE_COLLECTION_ACTION, DELETE_SMART_COLLECTION_ACTION, DELETE_TAG_ACTION,
        EDIT_SMART_COLLECTION_ACTION, EMPTY_TRASH_ACTION, ENLARGE_THUMBNAILS_ACTION,
        EXPORT_SELECTED_ACTION, EXPORT_SELECTED_AS_ACTION, FIND_SIMILAR_ACTION,
        IMPORT_FILES_ACTION, IMPORT_FOLDER_ACTION, LIBRARY_INFO_ACTION, MERGE_TAG_ACTION,
        MOVE_COLLECTION_ACTION, MOVE_TAG_ACTION, MOVE_TO_COLLECTION_ACTION, NEW_COLLECTION_ACTION,
        NEW_SMART_COLLECTION_ACTION, NEW_SUB_TAG_ACTION, NEW_SUBCOLLECTION_ACTION,
        OPEN_LIBRARY_ACTION, OPEN_PREVIEW_ACTION, OPEN_RECENT_LIBRARY_ACTION, OPEN_WITH_ACTION,
        PASTE_ACTION, PASTE_AND_ANNOTATE_ACTION, PREFERENCES_ACTION, REDO_ACTION,
        REMOVE_FROM_ALL_COLLECTIONS_ACTION, REMOVE_FROM_COLLECTION_ACTION, RENAME_ASSET_ACTION,
        RENAME_COLLECTION_ACTION, RENAME_TAG_ACTION, RESTORE_SELECTED_ACTION, SEARCH_ACTION,
        SELECT_ALL_ACTION, SET_WALLPAPER_ACTION, SHOW_IN_FILES_ACTION, SHRINK_THUMBNAILS_ACTION,
        TOGGLE_FAVORITE_ACTION, TRASH_SELECTED_ACTION, ToastOffer, UNDO_ACTION,
        VIEW_ORIGINAL_ACTION, collection_parameter, smart_collection_parameter, tag_parameter,
    };

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/window.ui")]
    pub struct PigouneWindow {
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub library_libraries_section: TemplateChild<gio::Menu>,
        pub recent_menu: gio::Menu,
        #[template_child]
        pub primary_menu: TemplateChild<gio::Menu>,
        #[template_child]
        pub library_menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub welcome_recent_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub welcome_recent_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub welcome_recent_count: TemplateChild<gtk::Label>,
        pub welcome_recent_paths: RefCell<Vec<String>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub import_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub history_buttons: TemplateChild<gtk::Box>,
        #[template_child]
        pub undo_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub redo_button: TemplateChild<gtk::Button>,
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
        pub update_banner: TemplateChild<adw::Banner>,
        pub flatpak_updates: RefCell<Option<Rc<FlatpakUpdates>>>,
        pub analysing_colors: Cell<bool>,
        pub activity: crate::analysis_pace::Activity,
        pub library_generation: Cell<u32>,
        pub library_actions_enabled: Cell<bool>,
        pub update_news: RefCell<Option<UpdateNews>>,
        #[template_child]
        pub asset_grid: TemplateChild<PigouneAssetGrid>,
        #[template_child]
        pub asset_details: TemplateChild<PigouneAssetDetails>,
        #[template_child]
        pub details_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub window_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub asset_preview: TemplateChild<PigouneAssetPreview>,
        #[template_child]
        pub editor: TemplateChild<PigouneEditor>,
        #[template_child]
        pub preview_flight: TemplateChild<PigounePreviewFlight>,
        #[template_child]
        pub sidebar: TemplateChild<PigouneSidebar>,
        #[template_child]
        pub sidebar_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub details_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub grid_header: TemplateChild<PigouneGridHeader>,
        #[template_child]
        pub query_pills: TemplateChild<PigouneQueryPills>,
        pub current_view: Cell<AssetView>,
        pub view_chosen_by_click: Cell<bool>,
        pub browsing_selection: Cell<bool>,
        pub settings: OnceCell<gio::Settings>,
        pub library: RefCell<Option<Library>>,
        pub screen: Cell<Option<Dimensions>>,
        pub fresh_change: Cell<Option<ChangeStamp>>,
        pub view_to_restore: Cell<Option<(ChangeStamp, AssetView)>>,
        pub counts_refresh: RefCell<Option<glib::SourceId>>,
        pub search_query: RefCell<String>,
        pub filters: RefCell<AssetFilter>,
        pub hovered_drop: Cell<Option<(AssetView, bool)>>,
        pub undo_toast: RefCell<Option<(adw::Toast, ToastOffer)>>,
    }

    fn install_collection_actions(class: &mut <PigouneWindow as ObjectSubclass>::Class) {
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
        class.install_action(
            CUSTOMIZE_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(id) = collection_parameter(parameter) {
                    window.ask_collection_look(id);
                }
            },
        );
        class.install_action(
            MOVE_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(id) = collection_parameter(parameter) {
                    window.ask_collection_parent(id);
                }
            },
        );
    }

    fn install_smart_collection_actions(class: &mut <PigouneWindow as ObjectSubclass>::Class) {
        class.install_action(NEW_SMART_COLLECTION_ACTION, None, |window, _, _| {
            window.ask_new_smart_collection();
        });
        class.install_action(
            EDIT_SMART_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(collection) = smart_collection_parameter(parameter) {
                    window.ask_smart_collection_changes(collection);
                }
            },
        );
        class.install_action(
            DELETE_SMART_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(collection) = smart_collection_parameter(parameter) {
                    window.ask_smart_collection_deletion(collection);
                }
            },
        );
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
        class.install_action(REDO_ACTION, None, |window, _, _| {
            window.redo();
        });
        class.install_action(CLOSE_WINDOW_ACTION, None, |window, _, _| {
            window.close();
        });
        class.install_action_async(COPY_SELECTED_ACTION, None, |window, _, _| async move {
            window.copy_selected().await;
        });
        class.install_action_async(PASTE_ACTION, None, |window, _, _| async move {
            window.paste().await;
        });
        class.install_action_async(CAPTURE_ACTION, None, |window, _, _| async move {
            window.capture_screenshot().await;
        });
        class.install_action_async(PASTE_AND_ANNOTATE_ACTION, None, |window, _, _| async move {
            window.paste_and_annotate().await;
        });
        class.install_action(SELECT_ALL_ACTION, None, |window, _, _| {
            window.select_all();
        });
        class.install_action_async(EXPORT_SELECTED_ACTION, None, |window, _, _| async move {
            window.export_selected().await;
        });
        class.install_action(EXPORT_SELECTED_AS_ACTION, None, |window, _, _| {
            window.export_selected_as();
        });
        class.install_action_async(OPEN_WITH_ACTION, None, |window, _, _| async move {
            window.open_selected_with().await;
        });
        class.install_action_async(ANNOTATE_ACTION, None, |window, _, _| async move {
            window.annotate_selected().await;
        });
        class.install_action_async(VIEW_ORIGINAL_ACTION, None, |window, _, _| async move {
            window.view_selected_original().await;
        });
        class.install_action_async(SHOW_IN_FILES_ACTION, None, |window, _, _| async move {
            window.show_selected_in_files().await;
        });
        class.install_action_async(SET_WALLPAPER_ACTION, None, |window, _, _| async move {
            window.prepare_selected_as_wallpaper().await;
        });
        class.install_action(FIND_SIMILAR_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::find_similar_to_selected);
        });
        class.install_action(OPEN_PREVIEW_ACTION, None, |window, _, _| {
            window.after_menu_closes(|window| window.open_preview(None));
        });
        class.install_action(RENAME_ASSET_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::start_renaming_selected);
        });
        class.install_action(ADD_TAG_ACTION, None, |window, _, _| {
            if !window.can_organize_selection() {
                return;
            }
            window.after_menu_closes(|window| {
                window.imp().grid_header.details_button().set_active(true);
                window.imp().asset_details.focus_tag_entry();
            });
        });
        class.install_action(ADD_TO_COLLECTION_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::ask_collection_for_selected);
        });
        class.install_action(MOVE_TO_COLLECTION_ACTION, None, |window, _, _| {
            window.after_menu_closes(super::PigouneWindow::ask_collection_to_move_selected);
        });
        class.install_action(REMOVE_FROM_ALL_COLLECTIONS_ACTION, None, |window, _, _| {
            window.unclassify_selected();
        });
        class.install_action(
            REMOVE_FROM_COLLECTION_ACTION,
            Some(glib::VariantTy::STRING),
            |window, _, parameter| {
                if let Some(collection) = collection_parameter(parameter) {
                    window.remove_selected_from_collection(collection);
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
            PigouneEditor::ensure_type();
            PigounePreviewFlight::ensure_type();
            PigouneSidebar::ensure_type();
            PigouneGridHeader::ensure_type();
            PigouneQueryPills::ensure_type();
            class.bind_template();
            class.install_action(CREATE_LIBRARY_ACTION, None, |window, _, _| {
                window.show_new_library_dialog();
            });
            class.install_action_async(OPEN_LIBRARY_ACTION, None, |window, _, _| async move {
                window.choose_library_to_open().await;
            });
            class.install_action(SEARCH_ACTION, None, |window, _, _| {
                window.imp().grid_header.focus_search();
            });
            class.install_action(PREFERENCES_ACTION, None, |window, _, _| {
                window.show_preferences();
            });
            class.install_action(LIBRARY_INFO_ACTION, None, |window, _, _| {
                window.show_library_info();
            });
            class.install_action(CLEAR_RECENT_LIBRARIES_ACTION, None, |window, _, _| {
                window.forget_recent_libraries(&[]);
            });
            class.install_action(
                OPEN_RECENT_LIBRARY_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(path) = parameter.and_then(glib::Variant::get::<String>) {
                        window.open_recent_library(&path);
                    }
                },
            );
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
                NEW_SUB_TAG_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(tag) = tag_parameter(parameter) {
                        window.ask_sub_tag_name(tag);
                    }
                },
            );
            class.install_action(
                MOVE_TAG_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(tag) = tag_parameter(parameter) {
                        window.ask_tag_parent(tag);
                    }
                },
            );
            class.install_action(
                MERGE_TAG_ACTION,
                Some(glib::VariantTy::STRING),
                |window, _, parameter| {
                    if let Some(tag) = tag_parameter(parameter) {
                        window.ask_tag_to_merge_into(tag);
                    }
                },
            );
            install_smart_collection_actions(class);
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
            install_collection_actions(class);
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
            self.obj().explain_refused_import_shortcuts();
            self.obj().follow_welcome_recent_list();
            self.obj().follow_page_height();
            self.obj().watch_for_updates();
            self.obj().follow_screen();
            self.obj().connect_history_buttons();
        }
    }

    impl WidgetImpl for PigouneWindow {}

    impl WindowImpl for PigouneWindow {
        fn close_request(&self) -> glib::Propagation {
            if self.obj().is_editing() {
                self.editor.flush_draft();
            }
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
        window.bind_grid_settings(&settings);
        window.imp().asset_details.bind_sections(&settings);
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
        for (key, property) in [
            (settings::PREVIEW_BOUNDS, "show-bounds"),
            (settings::PREVIEW_PIXEL_GRID, "show-pixel-grid"),
            (settings::PREVIEW_DETAILS, "show-details"),
            (settings::PREVIEW_STRIP, "show-strip"),
        ] {
            settings
                .bind(key, &*window.imp().asset_preview, property)
                .build();
            window.add_action(&settings.create_action(key));
        }
        window.describe_selected_asset();
        window.connect_preview();
        window.connect_editor();
        window.follow_sidebar(&settings);
        window.follow_trash_confirmation(&settings);
        window.follow_search();
        window.follow_filters();
        window.offer_export_copies();
        window.follow_details_panel();
        window.follow_sidebar_panel();
        window.type_to_search();
        window.watch_activity();
        window
            .imp()
            .settings
            .set(settings)
            .expect("settings are set only once, at construction");
        window.follow_recent_libraries_limit(window.settings());
        window.refresh_recent_libraries();
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

    fn watch_activity(&self) {
        let activity = self.imp().activity.clone();
        let events = gtk::EventControllerLegacy::new();
        events.set_propagation_phase(gtk::PropagationPhase::Capture);
        events.connect_event(move |_, _| {
            activity.noticed();
            glib::Propagation::Proceed
        });
        self.add_controller(events);
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
        self.imp().grid_header.focus_search();
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
        self.imp().query_pills.connect_query_changed(glib::clone!(
            #[weak]
            entry,
            move |query| {
                entry.set_text(&query);
                entry.set_position(-1);
            }
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

    fn ready_flat_files(&self, ids: &[AssetId]) -> (FlatFiles, usize) {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return (FlatFiles::new(), 0);
        };
        let mut flats = FlatFiles::new();
        let mut not_ready = 0;
        for id in ids {
            match library.annotation_render(*id) {
                Ok(Some(render)) if render.file.is_file() => {
                    flats.insert(*id, render.file);
                }
                Ok(Some(_)) | Err(_) => not_ready += 1,
                Ok(None) => {}
            }
        }
        (flats, not_ready)
    }

    fn warm_annotated_renderings(&self, shown: &[PigouneAssetObject]) {
        let ids: Vec<AssetId> = shown
            .iter()
            .filter(|asset| asset.annotation().is_some())
            .map(PigouneAssetObject::id)
            .take(WARMED_RENDERINGS)
            .collect();
        if ids.is_empty() {
            return;
        }
        let window = self.clone();
        glib::spawn_future_local(async move {
            window.flat_files_for(&ids).await;
        });
    }

    async fn flat_files_for(&self, ids: &[AssetId]) -> FlatFiles {
        let jobs: Vec<(AssetId, AnnotationRender, PathBuf)> = {
            let library = self.imp().library.borrow();
            let Some(library) = library.as_ref() else {
                return FlatFiles::new();
            };
            ids.iter()
                .filter_map(|id| {
                    let render = library.annotation_render(*id).ok().flatten()?;
                    let asset = library.asset(*id).ok().flatten()?;
                    Some((*id, render, library.file_of(&asset)))
                })
                .collect()
        };
        let mut flats = FlatFiles::new();
        for (id, render, source) in jobs {
            if let Some(file) = annotated_files::ensure(&render, &source, self).await {
                flats.insert(id, file);
            }
        }
        flats
    }

    async fn output_source(
        &self,
        asset: &PigouneAssetObject,
    ) -> Result<(PathBuf, bool, (u32, u32)), String> {
        let Some(render) = asset.annotation() else {
            return Ok((
                asset.source_file().to_path_buf(),
                asset.asset().format == AssetFormat::Svg,
                natural_size(asset),
            ));
        };
        let unavailable = || gettext("The annotated image could not be created");
        let file = annotated_files::ensure(render, asset.source_file(), self)
            .await
            .ok_or_else(unavailable)?;
        let natural = annotated_files::png_size(&file).ok_or_else(unavailable)?;
        Ok((file, false, natural))
    }

    fn exposed_mask_names(&self, ids: &[AssetId]) -> Vec<String> {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Vec::new();
        };
        ids.iter()
            .filter_map(|id| {
                let document = library.annotation_document(*id).ok().flatten()?;
                let hidden = document.exposed_masks();
                let asset = library.asset(*id).ok().flatten()?;
                (!hidden.is_empty()).then_some(asset.display_name)
            })
            .collect()
    }

    async fn output_is_allowed(&self, ids: &[AssetId], operation: Operation) -> bool {
        let names = self.exposed_mask_names(ids);
        names.is_empty()
            || mask_alert::ask(self, &Scope::Images(names), operation, false).await
                == MaskChoice::Continue
    }

    fn offer_export_copies(&self) {
        self.imp().asset_grid.connect_export_copies(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or_default]
            move |assets| {
                let (flats, not_ready) = window.ready_flat_files(assets);
                let plan = drag_export::plan(assets, window.exposed_mask_names(assets), not_ready);
                if let Some(refusal) = &plan.refusal {
                    window.show_toast(&drag_export::refusal_text(refusal));
                    return Vec::new();
                }
                window
                    .imp()
                    .library
                    .borrow()
                    .as_ref()
                    .and_then(|library| library.export_copies(&plan.files, &flats).ok())
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
        let filters = imp.grid_header.filter_popover().choice();
        imp.grid_header.show_filter_count(filters.chosen_filters());
        imp.filters.replace(filters);
        let before = self.selected_ids();
        self.refresh_grid();
        self.tell_lost_selection(&before);
        self.schedule_counts_refresh();
    }

    fn search(&self, query: &str) {
        let imp = self.imp();
        if *imp.search_query.borrow() == query {
            return;
        }
        imp.search_query.replace(query.to_owned());
        imp.view_chosen_by_click.set(false);
        imp.query_pills.show_query(query);
        let before = self.selected_ids();
        self.refresh_grid();
        self.tell_lost_selection(&before);
        self.schedule_counts_refresh();
    }

    fn schedule_counts_refresh(&self) {
        let imp = self.imp();
        if let Some(pending) = imp.counts_refresh.take() {
            pending.remove();
        }
        let window = self.downgrade();
        let pending = glib::timeout_add_local_once(COUNTS_REFRESH_DELAY, move || {
            if let Some(window) = window.upgrade() {
                window.imp().counts_refresh.take();
                window.refresh_sidebar();
            }
        });
        imp.counts_refresh.replace(Some(pending));
    }

    fn reset_search(&self) {
        let imp = self.imp();
        imp.search_query.replace(String::new());
        imp.view_chosen_by_click.set(false);
        imp.filters.replace(AssetFilter::default());
        imp.grid_header.filter_popover().clear();
        imp.grid_header.show_filter_count(0);
        imp.grid_header.search_entry().set_text("");
        imp.query_pills.show_query("");
        self.schedule_counts_refresh();
    }

    fn follow_trash_confirmation(&self, settings: &gio::Settings) {
        self.label_empty_trash_button(settings);
        self.describe_trash_retention(settings);
        self.explain_empty_trash_button();
        settings.connect_changed(
            Some(settings::AUTO_EMPTY_TRASH),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |settings, _| {
                    window.describe_trash_retention(settings);
                    window.empty_expired_trash();
                }
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

    fn describe_trash_retention(&self, settings: &gio::Settings) {
        let title = if settings.boolean(settings::AUTO_EMPTY_TRASH) {
            gettext("Assets in the trash are deleted for good after 30 days.")
        } else {
            gettext("Assets in the trash are deleted for good only when it is emptied.")
        };
        self.imp().trash_banner.set_title(&title);
    }

    fn explain_empty_trash_button(&self) {
        if let Some(button) = descendant_button(self.imp().trash_banner.upcast_ref()) {
            button.set_tooltip_text(Some(&gettext("Delete every asset in the trash for good")));
        }
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
            settings::SMART_COLLECTION_SORT,
            settings::SMART_COLLECTION_SORT_REVERSED,
        ] {
            self.add_action(&settings.create_action(key));
        }
        for key in [
            settings::COLLECTION_SORT,
            settings::COLLECTION_SORT_REVERSED,
            settings::SMART_COLLECTION_SORT,
            settings::SMART_COLLECTION_SORT_REVERSED,
            settings::SHOW_SMART_COLLECTIONS,
            settings::SHOW_TAGS,
            settings::SHOW_COUNTS,
            settings::SIDEBAR_COLLECTIONS_EXPANDED,
            settings::SIDEBAR_SMART_COLLECTIONS_EXPANDED,
            settings::SIDEBAR_TAGS_EXPANDED,
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
        self.imp().sidebar.connect_tag_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dragged, target| window.move_tag(dragged, target)
        ));
        self.imp().sidebar.connect_collection_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dragged, drop| window.drop_collection(dragged, drop)
        ));
        self.imp()
            .sidebar
            .connect_smart_collection_dropped(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |dragged, target, after| window.drop_smart_collection(dragged, target, after)
            ));
        self.imp().sidebar.connect_section_toggled(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| window.toggle_sidebar_section(entry)
        ));
        self.imp().sidebar.connect_assets_hovered(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |hovered| window.describe_hovered_drop(hovered.as_ref())
        ));
        self.imp().sidebar.connect_drop_offer(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            drop_action::DropOffer::REFUSED,
            move |target| drop_action::offer_for_assets(target, window.dragged_from_view())
        ));
        self.imp().sidebar.connect_assets_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view, assets, keep_source| window.drop_assets_on(view, &assets, keep_source)
        ));
        self.imp().sidebar.connect_content_dropped(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |view, dropped| {
                let collection = match view {
                    AssetView::Collection(id) => Some(id),
                    AssetView::All | AssetView::Unclassified => None,
                    AssetView::Favorites
                    | AssetView::Tag(_)
                    | AssetView::Smart(_)
                    | AssetView::Trash => return,
                };
                glib::spawn_future_local(async move {
                    window.import_content(dropped, collection).await;
                });
            }
        ));
    }

    fn toggle_sidebar_section(&self, entry: SidebarEntry) {
        let key = match entry {
            SidebarEntry::CollectionsHeader => settings::SIDEBAR_COLLECTIONS_EXPANDED,
            SidebarEntry::SmartCollectionsHeader => settings::SIDEBAR_SMART_COLLECTIONS_EXPANDED,
            SidebarEntry::TagsHeader => settings::SIDEBAR_TAGS_EXPANDED,
            SidebarEntry::View(_) | SidebarEntry::TagCloud => return,
        };
        let settings = self.settings();
        let _ = settings.set_boolean(key, !settings.boolean(key));
    }

    fn note_view_chosen_by_click(&self) -> bool {
        let imp = self.imp();
        let narrows = displayed_view::narrows_by_click(
            self.settings().boolean(settings::SEARCH_EVERYWHERE),
            self.current_filter().narrows(),
        );
        let changed = narrows && !imp.view_chosen_by_click.get();
        if changed {
            imp.view_chosen_by_click.set(true);
        }
        changed
    }

    fn show_view(&self, view: AssetView) {
        let imp = self.imp();
        let narrowed = self.note_view_chosen_by_click();
        if imp.current_view.replace(view) == view {
            if narrowed {
                self.refresh_grid();
            }
            return;
        }
        self.remember_view(view);
        imp.asset_preview.close();
        let before = self.selected_ids();
        self.refresh_grid();
        self.tell_lost_selection(&before);
    }

    fn tell_lost_selection(&self, before: &[AssetId]) {
        let lost = crate::selection_loss::lost_count(before, &self.selected_ids());
        if lost == 0 {
            return;
        }
        self.show_toast(
            &ngettext(
                "{count} selected asset is not in this view.",
                "{count} selected assets are not in this view.",
                u32::try_from(lost).unwrap_or(u32::MAX),
            )
            .replace("{count}", &lost.to_string()),
        );
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

    fn drop_smart_collection(
        &self,
        dragged: SmartCollectionId,
        target: SmartCollectionId,
        after: bool,
    ) {
        let order = self.smart_collection_order();
        let mut collections = match self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(Library::smart_collections)
        {
            Some(Ok(collections)) => collections,
            Some(Err(error)) => {
                self.show_library_error(&error);
                return;
            }
            None => return,
        };
        order.sort(&mut collections, |name: &str| {
            glib::FilenameCollationKey::from(name)
        });
        let shown: Vec<SmartCollectionId> =
            collections.iter().map(|collection| collection.id).collect();
        let arranged = smart_collection_sort::reordered(&shown, dragged, target, after);
        let custom_order_shown =
            order.criterion == SmartCollectionCriterion::Custom && !order.reversed;
        if arranged == shown && custom_order_shown {
            return;
        }
        if let Err(message) =
            self.change_smart_collection(&SmartCollectionCommand::Arrange { order: arranged })
        {
            self.show_smart_collection_error(&message);
            return;
        }
        if !custom_order_shown {
            let settings = self.settings();
            settings::store_bool(settings, settings::SMART_COLLECTION_SORT_REVERSED, false);
            settings::store_string(settings, settings::SMART_COLLECTION_SORT, "custom");
        }
        self.show_undoable_toast(&gettext("The order of the smart collections was changed."));
    }

    fn bind_grid_settings(&self, settings: &gio::Settings) {
        settings
            .bind(
                settings::THUMBNAIL_SIZE,
                &*self.imp().asset_grid,
                "tile-size",
            )
            .build();
        settings
            .bind(settings::SHOW_NAMES, &*self.imp().asset_grid, "show-names")
            .build();
        settings
            .bind(
                settings::NAMES_CLICKABLE,
                &*self.imp().asset_grid,
                "names-clickable",
            )
            .build();
        settings
            .bind(
                settings::ANIMATE_ON_HOVER,
                &*self.imp().asset_grid,
                "animate-on-hover",
            )
            .build();
        settings
            .bind(
                settings::SHOW_FORMATS,
                &*self.imp().asset_grid,
                "show-formats",
            )
            .build();
        settings
            .bind(
                settings::TILE_BACKGROUND,
                &*self.imp().asset_grid,
                "tile-background",
            )
            .build();
        let search_scope = gio::SimpleAction::new_stateful(
            "search-scope",
            Some(glib::VariantTy::STRING),
            &"everywhere".to_variant(),
        );
        search_scope.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, parameter| {
                if let Some(choice) = parameter.and_then(glib::Variant::str) {
                    window.imp().view_chosen_by_click.set(false);
                    settings::store_bool(
                        window.settings(),
                        settings::SEARCH_EVERYWHERE,
                        choice == "everywhere",
                    );
                }
            }
        ));
        self.add_action(&search_scope);
        settings.connect_changed(
            Some(settings::SEARCH_EVERYWHERE),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| window.refresh_grid()
            ),
        );
    }

    fn smart_collection_order(&self) -> SmartCollectionOrder {
        let settings = self.settings();
        SmartCollectionOrder {
            criterion: SmartCollectionCriterion::from_setting(
                &settings.string(settings::SMART_COLLECTION_SORT),
            ),
            reversed: settings.boolean(settings::SMART_COLLECTION_SORT_REVERSED),
        }
    }

    fn target_collection(&self) -> Option<CollectionId> {
        match self.imp().current_view.get() {
            AssetView::Collection(id) => Some(id),
            AssetView::All
            | AssetView::Favorites
            | AssetView::Unclassified
            | AssetView::Tag(_)
            | AssetView::Smart(_)
            | AssetView::Trash => None,
        }
    }

    pub fn reopen_last_library(&self) {
        let last_library_path = self.settings().string(settings::LAST_LIBRARY_PATH);
        if last_library_path.is_empty() || !self.settings().boolean(settings::REOPEN_LAST_LIBRARY) {
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

    fn clear_thumbnails(&self) -> Result<(), String> {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .map_or(Ok(()), |library| {
                library
                    .clear_thumbnail_cache()
                    .map_err(|error| error_messages::describe(&error))
            })
    }

    fn health_actions(&self) -> HealthActions {
        HealthActions {
            plan: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(String::new()),
                move || window.health_plan()
            )),
            adopt: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |file: &Path| window.adopt_unrecorded(file)
            )),
            replace: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |id: AssetId, copy: &Path| window.replace_stored_file(id, copy)
            )),
            remove: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |id: AssetId| window.remove_record(id)
            )),
            trash: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |id: AssetId| window.trash_damaged_resource(id)
            )),
            undo: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |stamp: ChangeStamp| window.undo_from_health(stamp)
            )),
            changed: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.refresh_assets()
            )),
            show: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |id| window.show_in_all(id)
            )),
        }
    }

    fn duplicate_actions(&self) -> DuplicateActions {
        DuplicateActions {
            plan: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(String::new()),
                move || window.duplicate_plan()
            )),
            members: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move |ids: &[AssetId]| window.duplicate_members(ids)
            )),
            thumbnails: self.imp().asset_grid.thumbnail_cache(),
            remove: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(RemovalFailure::Changed),
                move |removal: &ConfirmedRemoval| window.remove_duplicates(removal)
            )),
            attachments: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move |id: AssetId| window.duplicate_attachments(id)
            )),
            undo: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |stamp: ChangeStamp| window.undo_from_health(stamp)
            )),
            changed: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.refresh_assets()
            )),
            show: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |id| window.show_in_all(id)
            )),
            prepare: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || {
                    Box::pin(async { Ok(true) })
                        as Pin<Box<dyn Future<Output = Result<bool, String>>>>
                },
                move |canceled: Arc<AtomicBool>, progress: Rc<dyn Fn(u64, u64)>| {
                    window.prepare_duplicates(canceled, progress)
                }
            )),
            ignoring: self.ignoring_actions(),
            compare: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move |first: AssetId, second: AssetId| window.duplicate_kind_between(first, second)
            )),
            trash_expires: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move || window.settings().boolean(settings::AUTO_EMPTY_TRASH)
            )),
        }
    }

    fn ignoring_actions(&self) -> IgnoringActions {
        IgnoringActions {
            ignore_group: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |group: &[AssetId]| window
                    .change_ignoring(&ignoring_failure_title(), |library| library
                        .ignore_duplicate_group(group))
            )),
            ignore_member: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |member: AssetId, others: &[AssetId]| window
                    .change_ignoring(&ignoring_failure_title(), |library| library
                        .ignore_duplicate_member(member, others))
            )),
            unignore: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move |action: IgnoreId| window
                    .change_ignoring(&showing_again_failure_title(), |library| library
                        .unignore_duplicate_action(action))
            )),
            unignore_all: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_else]
                || Err(Refusal::default()),
                move || window.change_ignoring(
                    &showing_again_failure_title(),
                    Library::unignore_all_duplicates
                )
            )),
            ignored_entries: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move || window.ignored_duplicate_entries()
            )),
            ignored_count: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move || window.ignored_duplicate_count()
            )),
            ignored_asset_count: Box::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_default]
                move || window.ignored_asset_count()
            )),
        }
    }

    fn change_ignoring<T>(
        &self,
        title: &str,
        change: impl FnOnce(&mut Library) -> Result<T, DuplicateError>,
    ) -> Result<T, Refusal> {
        match self.change_library(change) {
            Some(Ok(done)) => Ok(done),
            Some(Err(error)) => Err(Refusal {
                title: title.to_string(),
                body: error_messages::describe_duplicate(&error),
                try_again: false,
            }),
            None => Err(Refusal::default()),
        }
    }

    fn ignored_duplicate_entries(&self) -> Vec<IgnoredEntry> {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.ignored_duplicate_entries().ok())
            .unwrap_or_default()
    }

    fn ignored_duplicate_count(&self) -> usize {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.ignored_duplicate_count().ok())
            .unwrap_or_default()
    }

    fn ignored_asset_count(&self) -> usize {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.ignored_asset_count().ok())
            .unwrap_or_default()
    }

    fn refresh_open_library_info(&self) {
        if let Some(dialog) = self
            .visible_dialog()
            .and_downcast::<PigouneLibraryInfoDialog>()
        {
            dialog.history_changed();
        }
    }

    fn duplicate_kind_between(&self, first: AssetId, second: AssetId) -> Option<DuplicateKind> {
        let library = self.imp().library.borrow();
        library
            .as_ref()?
            .duplicate_kind_between(first, second)
            .ok()
            .flatten()
    }

    fn remove_duplicates(&self, removal: &ConfirmedRemoval) -> Removed {
        let outcome = self.change_library(|library| library.trash_confirmed_duplicates(removal));
        match outcome {
            Some(Ok(stamp)) => Ok(stamp),
            Some(Err(RemovalError::Asset(error))) => Err(RemovalFailure::Refused(Refusal {
                title: gettext("Unable to Change the Asset"),
                body: error_messages::describe_asset(&error),
                try_again: false,
            })),
            Some(Err(RemovalError::Library(error))) => Err(RemovalFailure::Refused(Refusal {
                title: gettext("Unable to Change the Asset"),
                body: error_messages::describe(&error),
                try_again: false,
            })),
            Some(Err(_)) | None => Err(RemovalFailure::Changed),
        }
    }

    fn duplicate_attachments(&self, id: AssetId) -> Attachments {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Attachments::default();
        };
        let asset = library.asset(id).ok().flatten();
        Attachments {
            collections: library.collections_of(id).map_or(0, |found| found.len()),
            tags: library.tags_of(id).map_or(0, |found| found.len()),
            favorite: asset.as_ref().is_some_and(|asset| asset.is_favorite),
            note: asset
                .as_ref()
                .is_some_and(|asset| !asset.note.trim().is_empty()),
        }
    }

    fn duplicate_plan(&self) -> Result<PlannedSearch, String> {
        let library = self.imp().library.borrow();
        let library = library.as_ref().ok_or_else(String::new)?;
        let plan = library
            .duplicate_plan()
            .map_err(|error| error_messages::describe(&error))?;
        let pending = library
            .count_awaiting_visual_hash()
            .map_err(|error| error_messages::describe(&error))?;
        Ok(PlannedSearch { plan, pending })
    }

    fn duplicate_members(&self, ids: &[AssetId]) -> Vec<DuplicateMember> {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Vec::new();
        };
        ids.iter()
            .filter_map(|id| library.asset(*id).ok().flatten())
            .map(|asset| DuplicateMember {
                id: asset.id,
                summary: asset_facts::summary_text(&asset, None),
                file: library.file_of(&asset),
                thumbnail_file: library.thumbnail_file(asset.id, THUMBNAIL_PIXELS),
                name: asset.display_name,
            })
            .collect()
    }

    fn replace_stored_file(&self, id: AssetId, copy: &Path) -> Replaced {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Err(Refusal::default());
        };
        let file_name = copy
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let asset_name = library
            .asset(id)
            .ok()
            .flatten()
            .map(|asset| asset.display_name)
            .unwrap_or_default();
        library
            .replace_stored_file(id, copy)
            .map_err(|error| error_messages::describe_replacement(&error, &file_name, &asset_name))
    }

    fn trash_damaged_resource(&self, id: AssetId) -> Trashed {
        self.trash_assets(vec![id])
    }

    fn trash_assets(&self, assets: Vec<AssetId>) -> Trashed {
        let trashed = self.change_library(|library| {
            library.apply_asset_command(&AssetCommand::SetTrashed {
                assets,
                trashed: true,
            })?;
            Ok::<_, AssetError>(library.latest_change())
        });
        match trashed {
            Some(Ok(Some(stamp))) => Ok(stamp),
            Some(Err(error)) => Err(Refusal {
                title: gettext("Unable to Change the Asset"),
                body: error_messages::describe_asset(&error),
                try_again: false,
            }),
            Some(Ok(None)) | None => Err(Refusal::default()),
        }
    }

    fn undo_from_health(&self, stamp: ChangeStamp) -> Replaced {
        match self.change_library(|library| library.undo_change(stamp)) {
            Some(Err(error)) => Err(Refusal {
                title: gettext("Unable to Undo"),
                body: error_messages::describe_undo(&error),
                try_again: false,
            }),
            Some(Ok(Some(_))) | None => Ok(()),
            Some(Ok(None)) => Err(Refusal {
                title: gettext("Unable to Undo"),
                body: gettext(
                    "Something else was changed since, so this change can no longer be undone here.",
                ),
                try_again: false,
            }),
        }
    }

    fn remove_record(&self, id: AssetId) -> Replaced {
        let mut library = self.imp().library.borrow_mut();
        let Some(library) = library.as_mut() else {
            return Err(Refusal::default());
        };
        let asset_name = library
            .asset(id)
            .ok()
            .flatten()
            .map(|asset| asset.display_name)
            .unwrap_or_default();
        library
            .remove_record_of_missing_file(id)
            .map_err(|error| error_messages::describe_removal(&error, &asset_name))
    }

    fn adopt_unrecorded(&self, file: &Path) -> Adopted {
        let mut library = self.imp().library.borrow_mut();
        let Some(library) = library.as_mut() else {
            return Err(Refusal::default());
        };
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        match library.adopt_unrecorded(file) {
            Ok(id) => {
                let shown = library
                    .asset(id)
                    .ok()
                    .flatten()
                    .map_or(name, |asset| asset.display_name);
                Ok((id, shown))
            }
            Err(error) => {
                let known_name = match &error {
                    AdoptError::AlreadyKnown(_, known) => library
                        .asset(*known)
                        .ok()
                        .flatten()
                        .map(|asset| asset.display_name),
                    _ => None,
                };
                Err(error_messages::describe_adoption(
                    &error,
                    &name,
                    known_name.as_deref(),
                ))
            }
        }
    }

    fn health_plan(&self) -> Result<HealthPlan, String> {
        self.imp().library.borrow().as_ref().map_or_else(
            || Err(String::new()),
            |library| {
                library
                    .health_plan()
                    .map_err(|error| error_messages::describe(&error))
            },
        )
    }

    fn show_library_info(&self) {
        let shown = self.imp().library.borrow().as_ref().map(|library| {
            let records = library.records()?;
            let record_thumbnails = [
                &records.heaviest,
                &records.largest,
                &records.newest,
                &records.oldest,
            ]
            .into_iter()
            .flatten()
            .map(|asset| (asset.id, library.thumbnail_file(asset.id, THUMBNAIL_PIXELS)))
            .collect();
            Ok::<_, LibraryError>(LibraryReport {
                name: library.name(),
                root: library.root().to_path_buf(),
                overview: library.overview()?,
                shares: library.format_shares()?,
                storage: library.storage_use_without_thumbnails()?,
                records,
                record_thumbnails,
                colors: library.color_shares()?,
                export_outline: library.export_outline(&gettext("Unclassified"))?,
            })
        });
        let shown = shown.map(|report| {
            report.map(|report| {
                PigouneLibraryInfoDialog::new(
                    report,
                    glib::clone!(
                        #[weak(rename_to = window)]
                        self,
                        move |id| window.show_in_all(id)
                    ),
                    glib::clone!(
                        #[weak(rename_to = window)]
                        self,
                        #[upgrade_or]
                        Ok(()),
                        move || window.clear_thumbnails()
                    ),
                    glib::clone!(
                        #[weak(rename_to = window)]
                        self,
                        move || {
                            let window = window.clone();
                            glib::spawn_future_local(async move {
                                window.export_library().await;
                            });
                        }
                    ),
                    glib::clone!(
                        #[weak(rename_to = window)]
                        self,
                        #[upgrade_or_default]
                        move || window.analysis_backlog()
                    ),
                    self.health_actions(),
                    self.duplicate_actions(),
                )
            })
        });
        match shown {
            Some(Ok(dialog)) => dialog.present_fitting(self),
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Read the Library")),
                    Some(&error_messages::describe(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
            None => {}
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
        self.leave_editor();
        let imp = self.imp();
        imp.window_title.set_title(&library.name());
        let path = library.root().to_string_lossy().into_owned();
        settings::store_string(self.settings(), settings::LAST_LIBRARY_PATH, &path);
        imp.asset_grid.forget_thumbnails();
        self.forget_undo_toast();
        self.reset_search();
        let mut library = library;
        library.set_screen(imp.screen.get());
        imp.library.replace(Some(library));
        imp.library_generation
            .set(imp.library_generation.get().wrapping_add(1));
        self.store_recent_libraries(&recent_libraries::with_opened(
            &self.recent_library_paths(),
            &path,
            self.recent_libraries_limit(),
        ));
        imp.current_view.set(self.view_on_opening());
        self.refresh_assets();
        imp.stack.set_visible_child_name(LIBRARY_PAGE);
        imp.import_button.set_visible(true);
        imp.history_buttons.set_visible(true);
        self.refresh_history_buttons();
        self.set_library_actions_enabled(true);
        self.offer_draft();
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.empty_expired_trash()
        ));
        glib::timeout_add_local_once(
            analysis_pace::START_DELAY,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.analyse_colors()
            ),
        );
    }

    fn analyse_colors(&self) {
        if self.imp().analysing_colors.replace(true) {
            return;
        }
        let window = self.clone();
        glib::spawn_future_local(async move {
            loop {
                let colors_recorded = window.analyse_pending_colors().await;
                let hashes_recorded = window.analyse_pending_visual_hashes().await;
                if !(colors_recorded && hashes_recorded && window.analysis_is_pending()) {
                    break;
                }
            }
            window.imp().analysing_colors.set(false);
        });
    }

    fn analysis_is_pending(&self) -> bool {
        [
            self.files_awaiting_colors(),
            self.files_awaiting_visual_hashes(),
        ]
        .iter()
        .any(|pending| pending.as_ref().is_some_and(|files| !files.is_empty()))
    }

    async fn wait_for_calm_in(&self, generation: u32) -> bool {
        self.imp().activity.wait_for_calm().await;
        self.imp().library_generation.get() == generation
    }

    fn record_colors(&self, analysed: &[(AssetId, Vec<pigoune_core::DominantColor>)]) -> bool {
        let recorded = self
            .imp()
            .library
            .borrow_mut()
            .as_mut()
            .map(|library| library.record_colors_batch(analysed));
        let done = matches!(recorded, Some(Ok(())));
        if done {
            self.show_new_colors();
        }
        done
    }

    fn record_analyses(&self, analysed: &[(AssetId, Option<VisualAnalysis>)]) -> bool {
        let recorded = self
            .imp()
            .library
            .borrow_mut()
            .as_mut()
            .map(|library| library.record_visual_analysis_batch(analysed));
        matches!(recorded, Some(Ok(())))
    }

    async fn analyse_pending_colors(&self) -> bool {
        let generation = self.imp().library_generation.get();
        while let Some(pending) = self.files_awaiting_colors()
            && !pending.is_empty()
        {
            let mut analysed = Vec::with_capacity(pending.len());
            for (id, file, thumbnail_file) in pending {
                if !self.imp().activity.is_calm()
                    && !analysed.is_empty()
                    && !self.record_colors(&std::mem::take(&mut analysed))
                {
                    return false;
                }
                if !self.wait_for_calm_in(generation).await {
                    return false;
                }
                let colors = match thumbnails::thumbnail(&file, &thumbnail_file).await {
                    Some(texture) => {
                        let pixels = asset_colors::rgba_pixels(&texture);
                        gio::spawn_blocking(move || dominant_colors(&pixels))
                            .await
                            .unwrap_or_default()
                    }
                    None => Vec::new(),
                };
                analysed.push((id, colors));
            }
            if !self.record_colors(&analysed) {
                return false;
            }
        }
        true
    }

    async fn analyse_pending_visual_hashes(&self) -> bool {
        let generation = self.imp().library_generation.get();
        while let Some(pending) = self.files_awaiting_visual_hashes()
            && !pending.is_empty()
        {
            let mut analysed = Vec::with_capacity(pending.len());
            for (id, file, thumbnail_file) in pending {
                if !self.imp().activity.is_calm()
                    && !analysed.is_empty()
                    && !self.record_analyses(&std::mem::take(&mut analysed))
                {
                    return false;
                }
                if !self.wait_for_calm_in(generation).await {
                    return false;
                }
                let analysis = analyse_drawing(&file, &thumbnail_file).await.flatten();
                analysed.push((id, analysis));
            }
            if !self.record_analyses(&analysed) {
                return false;
            }
        }
        true
    }

    async fn analyse_for_search(
        &self,
        waiting: Vec<(AssetId, PathBuf, PathBuf)>,
        details_only: bool,
        canceled: &AtomicBool,
        progress: &dyn Fn(u64, u64),
    ) -> Result<bool, String> {
        let total = u64::try_from(waiting.len()).unwrap_or(u64::MAX);
        let mut done = 0_u64;
        progress(done, total);
        for chunk in waiting.chunks(COLOR_BATCH) {
            let mut analysed = Vec::with_capacity(chunk.len());
            for (id, file, thumbnail_file) in chunk {
                if canceled.load(Ordering::Relaxed) {
                    break;
                }
                if let Some(analysis) = analyse_drawing(file, thumbnail_file).await {
                    analysed.push((*id, analysis));
                }
            }
            self.record_for_search(&analysed, details_only)?;
            if canceled.load(Ordering::Relaxed) {
                return Ok(false);
            }
            done += u64::try_from(chunk.len()).unwrap_or(0);
            progress(done, total);
        }
        Ok(true)
    }

    fn record_for_search(
        &self,
        analysed: &[(AssetId, Option<VisualAnalysis>)],
        details_only: bool,
    ) -> Result<(), String> {
        let mut library = self.imp().library.borrow_mut();
        let library = library.as_mut().ok_or_else(String::new)?;
        let recorded = if details_only {
            let details: Vec<(AssetId, Option<VisualDetail>)> = analysed
                .iter()
                .map(|(id, analysis)| {
                    (
                        *id,
                        analysis.as_ref().map(|analysis| analysis.detail.clone()),
                    )
                })
                .collect();
            library.record_visual_details_batch(&details)
        } else {
            library.record_visual_analysis_batch(analysed)
        };
        recorded.map_err(|error| error_messages::describe(&error))
    }

    async fn analyse_duplicate_candidates(
        &self,
        canceled: &AtomicBool,
        progress: &dyn Fn(u64, u64),
    ) -> Result<bool, String> {
        let unhashed = self
            .files_awaiting(|library| library.assets_awaiting_visual_hash(usize::MAX))
            .unwrap_or_default();
        if !unhashed.is_empty()
            && !self
                .analyse_for_search(unhashed, false, canceled, progress)
                .await?
        {
            return Ok(false);
        }
        let candidates = self
            .files_awaiting(Library::duplicate_candidates_awaiting_detail)
            .unwrap_or_default();
        if candidates.is_empty() {
            return Ok(true);
        }
        self.analyse_for_search(candidates, true, canceled, progress)
            .await
    }

    fn prepare_duplicates(
        &self,
        canceled: Arc<AtomicBool>,
        progress: Rc<dyn Fn(u64, u64)>,
    ) -> Pin<Box<dyn Future<Output = Result<bool, String>>>> {
        let window = self.clone();
        Box::pin(async move {
            window
                .analyse_duplicate_candidates(&canceled, progress.as_ref())
                .await
        })
    }

    fn analysis_backlog(&self) -> (u64, u64) {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.analysis_backlog().ok())
            .unwrap_or_default()
    }

    fn files_awaiting_colors(&self) -> Option<Vec<(AssetId, PathBuf, PathBuf)>> {
        self.files_awaiting(|library| library.assets_awaiting_colors(COLOR_BATCH))
    }

    fn files_awaiting_visual_hashes(&self) -> Option<Vec<(AssetId, PathBuf, PathBuf)>> {
        self.files_awaiting(|library| library.assets_awaiting_visual_hash(COLOR_BATCH))
    }

    fn files_awaiting(
        &self,
        awaiting: impl Fn(&Library) -> Result<Vec<AssetId>, LibraryError>,
    ) -> Option<Vec<(AssetId, PathBuf, PathBuf)>> {
        let library = self.imp().library.borrow();
        let library = library.as_ref()?;
        let ids = awaiting(library).ok()?;
        Some(
            ids.into_iter()
                .filter_map(|id| library.asset(id).ok().flatten())
                .map(|asset| {
                    (
                        asset.id,
                        library.file_of(&asset),
                        library.thumbnail_file(asset.id, THUMBNAIL_PIXELS),
                    )
                })
                .collect(),
        )
    }

    fn show_new_colors(&self) {
        let imp = self.imp();
        let smart_collections_use_colors = imp
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.smart_collections().ok())
            .is_some_and(|collections| {
                collections.iter().any(|collection| {
                    !collection.filter.colors.is_empty() || collection.filter.custom_color.is_some()
                })
            });
        let colors_shown = !imp.filters.borrow().colors.is_empty()
            || imp.filters.borrow().custom_color.is_some()
            || matches!(imp.current_view.get(), AssetView::Smart(_));
        if smart_collections_use_colors {
            self.refresh_sidebar();
        }
        if colors_shown {
            self.refresh_assets();
        }
    }

    fn recent_libraries_limit(&self) -> usize {
        usize::try_from(self.settings().int(settings::RECENT_LIBRARIES_LIMIT)).unwrap_or(0)
    }

    fn follow_recent_libraries_limit(&self, settings: &gio::Settings) {
        settings.connect_changed(
            Some(settings::RECENT_LIBRARIES_LIMIT),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| {
                    window.store_recent_libraries(&recent_libraries::limited(
                        &window.recent_library_paths(),
                        window.recent_libraries_limit(),
                    ));
                }
            ),
        );
    }

    fn recent_library_paths(&self) -> Vec<String> {
        self.settings()
            .strv(settings::RECENT_LIBRARIES)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn store_recent_libraries(&self, paths: &[String]) {
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        if let Err(error) = self.settings().set_strv(settings::RECENT_LIBRARIES, paths) {
            glib::g_warning!("pigoune", "Unable to save the recent libraries: {error}");
        }
        self.refresh_recent_libraries();
    }

    fn refresh_recent_libraries(&self) {
        let imp = self.imp();
        let open = imp
            .library
            .borrow()
            .as_ref()
            .map(|library| library.root().to_string_lossy().into_owned());
        let paths: Vec<String> =
            recent_libraries::limited(&self.recent_library_paths(), self.recent_libraries_limit())
                .into_iter()
                .filter(|path| Some(path) != open.as_ref())
                .collect();
        imp.recent_menu.remove_all();
        let libraries = gio::Menu::new();
        for (path, label) in paths
            .iter()
            .zip(recent_libraries::labels(&host_path::shown_paths(&paths)))
        {
            let item = gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(
                Some(OPEN_RECENT_LIBRARY_ACTION),
                Some(&path.to_variant()),
            );
            libraries.append_item(&item);
        }
        let clearing = gio::Menu::new();
        clearing.append(
            Some(&gettext("Clear the List")),
            Some(CLEAR_RECENT_LIBRARIES_ACTION),
        );
        imp.recent_menu.append_section(None, &libraries);
        imp.recent_menu.append_section(None, &clearing);
        self.show_welcome_recent_libraries(&paths);
        let section = &imp.library_libraries_section;
        if section.n_items() > LIBRARY_ENTRIES {
            section.remove(RECENT_LIBRARIES_POSITION);
        }
        if !paths.is_empty() {
            section.insert_submenu(
                RECENT_LIBRARIES_POSITION,
                Some(&gettext("_Recent Libraries")),
                &imp.recent_menu,
            );
        }
    }

    fn follow_page_height(&self) {
        self.imp()
            .stack
            .connect_visible_child_name_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |stack| {
                    let height = if stack.visible_child_name().as_deref() == Some(WELCOME_PAGE) {
                        WELCOME_MINIMUM_HEIGHT
                    } else {
                        LIBRARY_MINIMUM_HEIGHT
                    };
                    window.set_height_request(height);
                }
            ));
        self.set_height_request(WELCOME_MINIMUM_HEIGHT);
    }

    fn forget_recent_libraries(&self, kept: &[String]) {
        let before = self.recent_library_paths();
        let open = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.root().to_string_lossy().into_owned());
        let after: Vec<String> = before
            .iter()
            .filter(|path| kept.contains(path) || Some(*path) == open.as_ref())
            .cloned()
            .collect();
        let removed = before.len() - after.len();
        if removed == 0 {
            return;
        }
        self.store_recent_libraries(&after);
        self.offer_to_restore_recent_libraries(&recent_removal_text(removed), before);
    }

    fn forget_recent_library(&self, path: &str) {
        let before = self.recent_library_paths();
        self.store_recent_libraries(&recent_libraries::without(&before, path));
        let name = library_display_name(Path::new(path));
        self.offer_to_restore_recent_libraries(
            &gettext("“{name}” removed from the recent libraries").replace("{name}", &name),
            before,
        );
    }

    fn offer_to_restore_recent_libraries(&self, message: &str, before: Vec<String>) {
        let toast = adw::Toast::new(message);
        toast.set_button_label(Some(&gettext("_Undo")));
        toast.connect_button_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.store_recent_libraries(&before)
        ));
        toasts::announce(&self.imp().toast_overlay, &toast);
    }

    fn follow_welcome_recent_list(&self) {
        self.imp()
            .welcome_recent_list
            .connect_row_activated(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, row| {
                    let path = usize::try_from(row.index()).ok().and_then(|index| {
                        window
                            .imp()
                            .welcome_recent_paths
                            .borrow()
                            .get(index)
                            .cloned()
                    });
                    if let Some(path) = path {
                        window.open_recent_library(&path);
                    }
                }
            ));
    }

    fn show_welcome_recent_libraries(&self, paths: &[String]) {
        let imp = self.imp();
        imp.welcome_recent_list.remove_all();
        for (path, label) in paths
            .iter()
            .zip(recent_libraries::labels(&host_path::shown_paths(paths)))
        {
            let row = welcome_recent_row(&label, Path::new(path));
            row.add_suffix(&self.forget_button(path));
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            imp.welcome_recent_list.append(&row);
        }
        imp.welcome_recent_paths.replace(paths.to_vec());
        imp.welcome_recent_count.set_label(&paths.len().to_string());
        imp.welcome_recent_box.set_visible(!paths.is_empty());
    }

    fn forget_button(&self, path: &str) -> gtk::Button {
        let button = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(gettext("Remove From the List, the Library Is Kept"))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        let path = path.to_owned();
        button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.forget_recent_library(&path)
        ));
        button
    }

    fn open_recent_library(&self, path: &str) {
        let root = PathBuf::from(path);
        if self.is_showing_library_at(&root) {
            return;
        }
        match Library::open(&root) {
            Ok(library) => self.show_library(library),
            Err(error) => {
                if matches!(
                    error,
                    LibraryError::NotFound(_) | LibraryError::NotALibrary(_)
                ) {
                    self.store_recent_libraries(&recent_libraries::without(
                        &self.recent_library_paths(),
                        path,
                    ));
                }
                self.show_opening_error(&root, &error);
            }
        }
    }

    fn close_library(&self) {
        let imp = self.imp();
        self.forget_undo_toast();
        self.reset_search();
        imp.library.replace(None);
        imp.asset_grid.show_assets(&[]);
        imp.asset_grid.forget_thumbnails();
        settings::store_string(self.settings(), settings::LAST_LIBRARY_PATH, "");
        self.refresh_recent_libraries();
        imp.window_title.set_title("Pigoune");
        imp.stack.set_visible_child_name(WELCOME_PAGE);
        imp.import_button.set_visible(false);
        imp.history_buttons.set_visible(false);
        imp.grid_header.details_button().set_visible(false);
        self.set_library_actions_enabled(false);
    }

    fn describe_selected_asset(&self) {
        let imp = self.imp();
        imp.asset_details.show(None, &imp.asset_grid.thumbnails());
        imp.asset_grid.connect_selection_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |selected| {
                if !window.is_previewing() {
                    window.show_selection(&selected);
                }
            }
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
                move |tag| window.open_tag_from_details(tag)
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

    fn follow_screen(&self) {
        self.connect_realize(|window| {
            let Some(surface) = window.surface() else {
                return;
            };
            surface.connect_enter_monitor(glib::clone!(
                #[weak]
                window,
                move |_, _| window.update_screen()
            ));
        });
    }

    fn update_screen(&self) {
        let imp = self.imp();
        let screen = screen_size::screen_of(self);
        if imp.screen.replace(screen) == screen {
            return;
        }
        let smart_collections_fit_screen = match imp.library.borrow_mut().as_mut() {
            Some(library) => {
                library.set_screen(screen);
                library.smart_collections().is_ok_and(|collections| {
                    collections
                        .iter()
                        .any(|collection| collection.filter.fits_screen)
                })
            }
            None => return,
        };
        let filters_fit_screen = imp.filters.borrow().fits_screen
            || matches!(imp.current_view.get(), AssetView::Smart(_));
        if smart_collections_fit_screen {
            self.refresh_sidebar();
        }
        if filters_fit_screen {
            self.refresh_grid();
        }
    }

    fn watch_for_updates(&self) {
        self.imp()
            .update_banner
            .connect_button_clicked(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_| window.follow_update_button()
            ));
        let watching = self.downgrade();
        glib::spawn_future_local(async move {
            let showing = watching.clone();
            let updates = FlatpakUpdates::watch(move |news| {
                if let Some(window) = showing.upgrade() {
                    window.show_update_news(news);
                }
            })
            .await;
            if let Some(window) = watching.upgrade() {
                window.imp().flatpak_updates.replace(updates.map(Rc::new));
            }
        });
    }

    fn show_update_news(&self, news: UpdateNews) {
        self.show_update_wording(&update_banner::wording(&news));
        self.imp().update_news.replace(Some(news));
    }

    fn show_update_wording(&self, wording: &update_banner::BannerWording) {
        let banner = &self.imp().update_banner;
        banner.set_title(&wording.title);
        banner.set_button_label(wording.button.as_deref());
        banner.set_revealed(true);
    }

    fn follow_update_button(&self) {
        let imp = self.imp();
        let Some(updates) = imp.flatpak_updates.borrow().clone() else {
            return;
        };
        let news = imp.update_news.borrow().clone();
        match news {
            Some(UpdateNews::Available) => {
                self.show_update_news(UpdateNews::Installing(0));
                glib::spawn_future_local(glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    async move {
                        if updates.install().await.is_err() {
                            window.show_update_news(UpdateNews::Failed);
                        }
                    }
                ));
            }
            Some(UpdateNews::Installed) => {
                let restart_by_hand = update_banner::restart_by_hand();
                glib::spawn_future_local(glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    async move {
                        if updates.launch_latest_version().await.is_ok() {
                            window.close();
                        } else {
                            window.show_update_wording(&restart_by_hand);
                        }
                    }
                ));
            }
            _ => {}
        }
    }

    fn start_renaming_selected(&self) {
        let imp = self.imp();
        let selected = self.selected_ids().len();
        if self.is_showing_trash() {
            if selected > 0 {
                self.show_toast(&gettext("Restore this asset to rename it."));
            }
        } else if selected > 1 {
            self.show_toast(&gettext("Select a single asset to rename it."));
        } else if imp.asset_grid.selected_asset().is_some() {
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
        let in_trash = self.is_showing_trash();
        if selected.len() == 1 {
            viewing.append_item(&shortcuts::menu_item(
                &gettext("Open With…"),
                OPEN_WITH_ACTION,
            ));
            viewing.append_item(&shortcuts::menu_item(
                &gettext("Show in Files"),
                SHOW_IN_FILES_ACTION,
            ));
            self.append_annotation_items(&viewing);
            if !in_trash {
                viewing.append_item(&menu_item(
                    &gettext("Find Similar…"),
                    FIND_SIMILAR_ACTION,
                    None,
                ));
            }
        }
        let sharing = gio::Menu::new();
        sharing.append_item(&menu_item(
            &gettext("Copy"),
            COPY_SELECTED_ACTION,
            shortcuts::current(COPY_SELECTED_ACTION).as_deref(),
        ));
        sharing.append_item(&shortcuts::menu_item(
            &gettext("Export To…"),
            EXPORT_SELECTED_ACTION,
        ));
        sharing.append_item(&shortcuts::menu_item(
            &gettext("Convert and Export…"),
            EXPORT_SELECTED_AS_ACTION,
        ));
        if selected.len() == 1 && !in_trash {
            sharing.append_item(&shortcuts::menu_item(
                &gettext("Frame and Set as Wallpaper…"),
                SET_WALLPAPER_ACTION,
            ));
        }
        if in_trash {
            let restoring = gio::Menu::new();
            restoring.append_item(&shortcuts::menu_item(
                &gettext("_Restore"),
                RESTORE_SELECTED_ACTION,
            ));
            let menu = gio::Menu::new();
            menu.append_section(None, &viewing);
            menu.append_section(None, &sharing);
            menu.append_section(None, &restoring);
            return menu.upcast();
        }
        let organizing = gio::Menu::new();
        if selected.len() == 1 {
            organizing.append_item(&menu_item(
                &gettext("Rename…"),
                RENAME_ASSET_ACTION,
                Some("F2"),
            ));
        }
        let favorite_label = if selected.iter().all(PigouneAssetObject::favorite) {
            gettext("Remove from Favorites")
        } else {
            gettext("Add to Favorites")
        };
        organizing.append_item(&menu_item(
            &favorite_label,
            TOGGLE_FAVORITE_ACTION,
            shortcuts::current(TOGGLE_FAVORITE_ACTION).as_deref(),
        ));
        organizing.append_item(&menu_item(
            &gettext("Add a Tag…"),
            ADD_TAG_ACTION,
            shortcuts::current(ADD_TAG_ACTION).as_deref(),
        ));
        organizing.append_item(&menu_item(
            &gettext("Add to a Collection…"),
            ADD_TO_COLLECTION_ACTION,
            shortcuts::current(ADD_TO_COLLECTION_ACTION).as_deref(),
        ));
        self.append_collection_actions(&organizing, &selected);
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

    fn append_collection_actions(&self, menu: &gio::Menu, selected: &[PigouneAssetObject]) {
        if matches!(self.displayed_view(), AssetView::Collection(_)) {
            menu.append_item(&menu_item(
                &gettext("Move to a Collection…"),
                MOVE_TO_COLLECTION_ACTION,
                shortcuts::current(MOVE_TO_COLLECTION_ACTION).as_deref(),
            ));
        }
        if let AssetView::Collection(id) = self.displayed_view()
            && self.directly_holds_any(id, selected)
        {
            let name = self.collection_name(id).unwrap_or_default();
            let item = gio::MenuItem::new(
                Some(&gettext("Remove from the Collection “{name}”").replace("{name}", &name)),
                None,
            );
            item.set_action_and_target_value(
                Some(REMOVE_FROM_COLLECTION_ACTION),
                Some(&id.to_string().to_variant()),
            );
            menu.append_item(&item);
        }
        if self.holds_any_collection(selected) {
            menu.append_item(&shortcuts::menu_item(
                &gettext("Remove from All Collections"),
                REMOVE_FROM_ALL_COLLECTIONS_ACTION,
            ));
        }
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
        self.warm_annotated_renderings(selected);
        let access = match selected {
            [single] => self.annotation_access(single),
            _ => AnnotationAccess::None,
        };
        imp.asset_details.show_annotations(access);
        self.refresh_selected_tags();
        self.refresh_selected_collections();
    }

    fn append_annotation_items(&self, menu: &gio::Menu) {
        let Some(asset) = self.annotation_target() else {
            return;
        };
        menu.append_item(&shortcuts::menu_item(
            &gettext("Annotate…"),
            ANNOTATE_ACTION,
        ));
        if self.is_annotated(asset.id()) {
            menu.append_item(&shortcuts::menu_item(
                &gettext("View Original"),
                VIEW_ORIGINAL_ACTION,
            ));
        }
    }

    fn is_annotated(&self, id: AssetId) -> bool {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .is_some_and(|library| library.annotations(id).ok().flatten().is_some())
    }

    fn annotation_target(&self) -> Option<PigouneAssetObject> {
        if self.is_showing_trash() {
            return None;
        }
        let [single] = &self.targeted_assets()[..] else {
            return None;
        };
        Some(single.clone())
    }

    fn annotation_access(&self, asset: &PigouneAssetObject) -> AnnotationAccess {
        if self.is_showing_trash() {
            AnnotationAccess::None
        } else if asset.asset().is_animated {
            AnnotationAccess::Unavailable(gettext("Animated images cannot be annotated yet"))
        } else {
            AnnotationAccess::Allowed {
                annotated: self.is_annotated(asset.id()),
            }
        }
    }

    async fn annotate_selected(&self) {
        let Some(asset) = self.annotation_target() else {
            return;
        };
        if self.is_editing() {
            return;
        }
        match self.annotation_access(&asset) {
            AnnotationAccess::Unavailable(reason) => {
                self.show_toast(&reason);
                return;
            }
            AnnotationAccess::None => return,
            AnnotationAccess::Allowed { .. } => {}
        }
        let id = asset.id();
        let stored = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.annotation_document(id));
        let Some(Ok(document)) = stored else {
            self.show_annotation_failure();
            return;
        };
        let document = document.unwrap_or_default();
        let Some(image) = annotated_files::load_source(asset.source_file()).await else {
            self.show_annotation_failure();
            return;
        };
        if image.width() > LARGEST_CAPTURE_SIDE || image.height() > LARGEST_CAPTURE_SIDE {
            self.show_image_too_large();
            return;
        }
        self.imp().asset_preview.close();
        self.imp().editor.open_item(id, &image, document);
        self.show_editor();
    }

    fn show_annotation_failure(&self) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Could Not Open the Annotations")),
            Some(&gettext(
                "The image or its annotations could not be read. They may come from a newer version of Pigoune.",
            )),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    async fn view_selected_original(&self) {
        let Some(asset) = self.annotation_target() else {
            return;
        };
        let source = asset.source_file().to_path_buf();
        let name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let copy = std::env::temp_dir().join(format!("pigoune-original-{}-{name}", asset.id()));
        if fs::copy(&source, &copy).is_err() {
            self.show_annotation_failure();
            return;
        }
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(copy)));
        if let Err(error) = launcher.launch_future(Some(self)).await
            && !is_dismissed(&error)
        {
            self.show_open_with_error(error.message());
        }
    }

    fn is_showing_trash(&self) -> bool {
        self.imp().current_view.get() == AssetView::Trash
    }

    fn trash_selected(&self) {
        if self.is_showing_trash() {
            if !self.targeted_ids().is_empty() {
                self.show_toast(&gettext(
                    "These assets are already in the trash. Restore them or empty the trash.",
                ));
            }
            return;
        }
        let imp = self.imp();
        if self.is_previewing() {
            if let Some(shown) = imp.asset_preview.shown_asset()
                && self.set_trashed(&[shown.id()], true)
            {
                imp.asset_preview.drop_shown();
            }
            return;
        }
        self.set_trashed(&self.selected_ids(), true);
    }

    fn restore_selected(&self) {
        if self.is_showing_trash() {
            self.set_trashed(&self.selected_ids(), false);
        }
    }

    fn set_trashed(&self, assets: &[AssetId], trashed: bool) -> bool {
        if assets.is_empty()
            || !self.apply_asset_command(&AssetCommand::SetTrashed {
                assets: assets.to_vec(),
                trashed,
            })
        {
            return false;
        }
        self.schedule_counts_refresh();
        self.drop_assets_leaving_view(assets);
        let count = assets.len();
        let message = if trashed {
            ngettext(
                "{count} asset moved to the trash",
                "{count} assets moved to the trash",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
        } else {
            ngettext(
                "{count} asset restored",
                "{count} assets restored",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
        };
        self.show_undoable_toast(&message.replace("{count}", &count.to_string()));
        true
    }

    fn ask_to_empty_trash(&self) {
        let count = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.view_count(AssetView::Trash).ok())
            .unwrap_or(0);
        if count == 0 {
            self.show_toast(&gettext("The trash is already empty."));
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
                    "{count} asset will be deleted for good. This cannot be undone.",
                    "{count} assets will be deleted for good. This cannot be undone.",
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
        let (result, before, after, redoable) = {
            let mut library = imp.library.borrow_mut();
            let library = library.as_mut()?;
            let before = library.latest_change();
            let result = change(library);
            (
                result,
                before,
                library.latest_change(),
                library.latest_redoable(),
            )
        };
        imp.fresh_change.set(after.filter(|_| after != before));
        self.dismiss_stale_undo_toast(after, redoable);
        self.refresh_history_buttons();
        Some(result)
    }

    fn dismiss_stale_undo_toast(&self, latest: Option<ChangeStamp>, redoable: Option<ChangeStamp>) {
        let imp = self.imp();
        let stale = imp
            .undo_toast
            .borrow()
            .as_ref()
            .is_some_and(|(_, offer)| match offer {
                ToastOffer::Undo(stamp) => latest != Some(*stamp),
                ToastOffer::Redo(stamp) => redoable != Some(*stamp),
            });
        if stale && let Some((toast, _)) = imp.undo_toast.take() {
            toast.dismiss();
        }
    }

    fn show_undoable_toast(&self, text: &str) {
        let Some(stamp) = self.imp().fresh_change.get() else {
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
        self.offer_toast(&toast, ToastOffer::Undo(stamp));
    }

    fn show_redoable_toast(&self, text: &str) {
        let redoable = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(Library::latest_redoable);
        let Some(stamp) = redoable else {
            self.show_toast(text);
            return;
        };
        let toast = adw::Toast::new(text);
        toast.set_button_label(Some(&gettext("_Redo")));
        toast.connect_button_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.redo_change(stamp)
        ));
        self.offer_toast(&toast, ToastOffer::Redo(stamp));
    }

    fn offer_toast(&self, toast: &adw::Toast, offer: ToastOffer) {
        let imp = self.imp();
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
        if let Some((previous, _)) = imp.undo_toast.replace(Some((toast.clone(), offer))) {
            previous.dismiss();
        }
        toasts::announce(&imp.toast_overlay, toast);
    }

    fn forget_undo_toast(&self) {
        if let Some((toast, _)) = self.imp().undo_toast.take() {
            toast.dismiss();
        }
    }

    fn connect_history_buttons(&self) {
        let imp = self.imp();
        imp.undo_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.undo()
        ));
        imp.redo_button.connect_clicked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.redo()
        ));
    }

    fn refresh_history_buttons(&self) {
        let imp = self.imp();
        let (can_undo, can_redo) = imp
            .library
            .borrow()
            .as_ref()
            .map_or((false, false), |library| {
                (library.can_undo(), library.can_redo())
            });
        imp.undo_button.set_sensitive(can_undo);
        imp.redo_button.set_sensitive(can_redo);
        imp.undo_button
            .set_tooltip_text(Some(&shortcut_tooltip(&gettext("Undo"), UNDO_ACTION)));
        imp.redo_button
            .set_tooltip_text(Some(&shortcut_tooltip(&gettext("Redo"), REDO_ACTION)));
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
                        "{count} asset older than {days} days deleted from the trash for good",
                        "{count} assets older than {days} days deleted from the trash for good",
                        u32::try_from(count).unwrap_or(u32::MAX),
                    )
                    .replace("{count}", &count.to_string())
                    .replace("{days}", &days.to_string()),
                );
            }
            Some(Err(error)) => self.show_library_error(&error),
        }
    }

    fn restore_view_after_undo(&self, undone: Option<ChangeStamp>) {
        let imp = self.imp();
        let Some((saved, view)) = imp.view_to_restore.get() else {
            return;
        };
        if undone != Some(saved) {
            return;
        }
        imp.view_to_restore.set(None);
        self.go_to_view(view, Vec::new());
    }

    fn latest_change_stamp(&self) -> Option<ChangeStamp> {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(Library::latest_change)
    }

    fn undo(&self) {
        if self.undo_typing() {
            return;
        }
        let stamp = self.latest_change_stamp();
        let undone = self.change_library(|library| {
            library.undo().map(|change| {
                change.map(|change| undo_message::describe(&change, library, Outcome::Undone))
            })
        });
        let succeeded = matches!(undone, Some(Ok(Some(_))));
        self.show_undo_outcome(undone, true);
        if succeeded {
            self.restore_view_after_undo(stamp);
        }
    }

    fn redo(&self) {
        if self.redo_typing() {
            return;
        }
        let redone = self.change_library(|library| {
            library.redo().map(|change| {
                change.map(|change| undo_message::describe(&change, library, Outcome::Redone))
            })
        });
        self.show_redo_outcome(redone, true);
    }

    fn redo_change(&self, stamp: ChangeStamp) {
        let redone = self.change_library(|library| {
            library.redo_change(stamp).map(|change| {
                change.map(|change| undo_message::describe(&change, library, Outcome::Redone))
            })
        });
        self.show_redo_outcome(redone, false);
    }

    fn show_redo_outcome(
        &self,
        redone: Option<Result<Option<String>, UndoError>>,
        tell_when_nothing: bool,
    ) {
        match redone {
            Some(Ok(Some(message))) => {
                self.refresh_sidebar();
                self.refresh_grid();
                self.refresh_open_library_info();
                self.show_undoable_toast(&message);
            }
            Some(Ok(None)) if tell_when_nothing => self.show_toast(&gettext("Nothing to redo")),
            Some(Ok(None)) | None => {}
            Some(Err(error)) => {
                self.refresh_sidebar();
                self.refresh_grid();
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Redo")),
                    Some(&error_messages::describe_undo(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
        }
    }

    fn undo_change(&self, stamp: ChangeStamp) {
        let undone = self.change_library(|library| {
            library.undo_change(stamp).map(|change| {
                change.map(|change| undo_message::describe(&change, library, Outcome::Undone))
            })
        });
        let succeeded = matches!(undone, Some(Ok(Some(_))));
        self.show_undo_outcome(undone, false);
        if succeeded {
            self.restore_view_after_undo(Some(stamp));
        }
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
                self.refresh_open_library_info();
                self.show_redoable_toast(&message);
            }
            Some(Ok(None)) if tell_when_nothing => self.show_toast(&gettext("Nothing to undo")),
            Some(Ok(None)) | None => {}
            Some(Err(error)) => {
                self.refresh_sidebar();
                self.refresh_grid();
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Undo")),
                    Some(&format!(
                        "{}\n\n{}",
                        error_messages::describe_undo(&error),
                        gettext(
                            "This change was left as it is. {shortcut} now undoes the change before it."
                        )
                        .replace("{shortcut}", &shortcuts::current_label(UNDO_ACTION))
                    )),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
        }
    }

    async fn copy_selected(&self) {
        if self.copy_typed_text() {
            return;
        }
        let selected = self.targeted_ids();
        if selected.is_empty() {
            return;
        }
        if !self.output_is_allowed(&selected, Operation::Copy).await {
            return;
        }
        let flats = self.flat_files_for(&selected).await;
        let prepared = self.imp().library.borrow().as_ref().map(|library| {
            let copies = library.clipboard_copies(&selected, &flats)?;
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
                        "{count} asset copied",
                        "{count} assets copied",
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

    async fn paste(&self) {
        if self.paste_typed_text() {
            return;
        }
        match pasted_content::read(&self.clipboard()).await {
            Pasted::Files(paths) => self.import_paths(paths).await,
            Pasted::Image(texture) => self.import_pasted_image(&texture).await,
            Pasted::Nothing => self.show_toast(&gettext("Nothing to paste")),
        }
    }

    pub async fn capture_screenshot(&self) {
        let imp = self.imp();
        if self.is_editing()
            && imp.editor.needs_saving()
            && !self.settle_editing_before_capture().await
        {
            return;
        }
        if imp.library.borrow().is_none() {
            let last = self.settings().string(settings::LAST_LIBRARY_PATH);
            if !last.is_empty() {
                self.reopen_library_at(PathBuf::from(last.as_str())).await;
            }
        }
        if imp.library.borrow().is_none() {
            let alert = adw::AlertDialog::new(
                Some(&gettext("Open a Library First")),
                Some(&gettext(
                    "A screenshot to annotate needs a library to be kept in. Open or create a library, then try again.",
                )),
            );
            alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
            alert.present(Some(self));
            return;
        }
        let hold = GtkWindowExt::application(self).map(|application| application.hold());
        self.set_visible(false);
        glib::timeout_future(CAPTURE_SETTLE).await;
        let outcome = screenshot::take().await;
        self.set_visible(true);
        self.present();
        drop(hold);
        self.annotate_screenshot(outcome);
    }

    async fn settle_editing_before_capture(&self) -> bool {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Take a Screenshot?")),
            Some(&gettext(
                "The image being annotated is not saved. Save it first, or let the screenshot replace it.",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("discard", &gettext("_Discard and Capture"));
        dialog.add_response("save", &gettext("_Save and Capture"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");
        match dialog.choose_future(Some(self)).await.as_str() {
            "save" => self.imp().editor.request_save().await,
            "discard" => true,
            _ => false,
        }
    }

    fn annotate_screenshot(&self, outcome: ScreenshotOutcome) {
        match outcome {
            ScreenshotOutcome::Cancelled => {}
            ScreenshotOutcome::Taken(path) => {
                let texture = gdk::Texture::from_filename(&path);
                let _ = fs::remove_file(&path);
                match texture {
                    Ok(texture) => self.annotate_texture(&texture),
                    Err(_) => self.show_screenshot_failure(),
                }
            }
            ScreenshotOutcome::Failed => self.show_screenshot_failure(),
        }
    }

    fn show_screenshot_failure(&self) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Could Not Take the Screenshot")),
            Some(&gettext(
                "The screenshot service did not give an image. Check that your desktop can take screenshots, then try again.",
            )),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    async fn paste_and_annotate(&self) {
        if self.paste_typed_text() {
            return;
        }
        let texture = match pasted_content::read(&self.clipboard()).await {
            Pasted::Image(texture) => Some(texture),
            Pasted::Files(paths) => match paths.as_slice() {
                [only] => gdk::Texture::from_filename(only).ok(),
                _ => None,
            },
            Pasted::Nothing => None,
        };
        match texture {
            Some(texture) => self.annotate_texture(&texture),
            None => self.show_toast(&gettext("No image to annotate in the clipboard")),
        }
    }

    fn show_image_too_large(&self) {
        let body = gettext("Images can be annotated up to {side} pixels on a side.")
            .replace("{side}", &LARGEST_CAPTURE_SIDE.to_string());
        let alert =
            adw::AlertDialog::new(Some(&gettext("Image Too Large to Annotate")), Some(&body));
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn annotate_texture(&self, texture: &gdk::Texture) {
        if texture.width() > LARGEST_CAPTURE_SIDE || texture.height() > LARGEST_CAPTURE_SIDE {
            self.show_image_too_large();
            return;
        }
        self.imp().asset_preview.close();
        self.imp().editor.open_texture(texture);
        self.show_editor();
    }

    async fn import_pasted_image(&self, texture: &gdk::Texture) {
        let png = texture.save_to_png_bytes();
        self.import_image_bytes(
            &pasted_content::image_name(),
            "png",
            &png,
            self.target_collection(),
        )
        .await;
    }

    async fn import_image_bytes(
        &self,
        name: &str,
        extension: &str,
        bytes: &[u8],
        collection: Option<CollectionId>,
    ) {
        let saved = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.save_pasted_image(name, extension, bytes));
        match saved {
            Some(Ok(path)) => self.import_paths_into(vec![path], collection).await,
            Some(Err(error)) => self.show_library_error(&error),
            None => {}
        }
    }

    fn paste_typed_text(&self) -> bool {
        self.activate_on_focused_text("clipboard.paste")
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
        let selected = self.targeted_ids();
        if selected.is_empty() {
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
        if !self.output_is_allowed(&selected, Operation::Export).await {
            return;
        }
        let flats = self.flat_files_for(&selected).await;
        let exported = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.export_to(&selected, &path, &flats));
        match exported {
            Some(Ok(report)) => {
                if !report.copies.is_empty() {
                    self.show_export_toast(report.copies.len(), &folder);
                }
                if !report.failures.is_empty() {
                    let failures: Vec<_> = report
                        .failures
                        .iter()
                        .map(|failure| conversion_report::Failure {
                            name: failure.display_name.clone(),
                            reason: conversion_report::copy_failure_reason(&failure.error),
                        })
                        .collect();
                    self.show_export_failures(&failures);
                }
            }
            Some(Err(error)) => self.show_export_error(&error),
            None => {}
        }
    }

    fn show_export_error(&self, error: &LibraryError) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Export")),
            Some(&error_messages::describe(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_export_failures(&self, failures: &[conversion_report::Failure]) {
        let alert = adw::AlertDialog::new(
            Some(&conversion_report::failures_heading(failures.len())),
            Some(&conversion_report::failures_body(failures)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    async fn export_library(&self) {
        let prepared = self.imp().library.borrow().as_ref().map(|library| {
            (
                library_display_name(library.root()),
                library.library_export_plan(&gettext("Unclassified"), &gettext("(annotated)")),
            )
        });
        let (name, plan) = match prepared {
            Some((name, Ok(plan))) => (name, plan),
            Some((_, Err(error))) => {
                self.show_export_error(&error);
                return;
            }
            None => return,
        };
        if plan.total() == 0 {
            let message = gettext("“{name}” has no assets to export").replace("{name}", &name);
            toasts::announce(&self.imp().toast_overlay, &adw::Toast::new(&message));
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
        let Some(destination) = folder.path() else {
            return;
        };
        if !self.confirm_library_export().await {
            return;
        }
        let progress = adw::Toast::new(&gettext("Exporting “{name}”…").replace("{name}", &name));
        progress.set_use_markup(false);
        progress.set_timeout(0);
        self.imp().toast_overlay.add_toast(progress.clone());
        let annotated: Vec<AssetId> = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.annotated_ids().ok())
            .map(|ids| ids.into_iter().collect())
            .unwrap_or_default();
        let flats = self.flat_files_for(&annotated).await;
        let outcome = gio::spawn_blocking(move || plan.run(&destination, &flats)).await;
        progress.dismiss();
        match outcome {
            Ok(Ok(report)) => self.finish_library_export(&report),
            Ok(Err(error)) => self.show_export_error(&error),
            Err(_) => {}
        }
    }

    async fn confirm_library_export(&self) -> bool {
        let annotated = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.annotated_ids().ok())
            .map_or(0, |ids| ids.len());
        if annotated == 0 {
            return true;
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Export the Original Images Too?")),
            Some(&gettext(
                "The export contains every original image, which may still show what the annotations hide, and a copy of each annotated image with its annotations included.",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("export", &gettext("_Export"));
        dialog.set_response_appearance("export", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.choose_future(Some(self)).await == "export"
    }

    fn finish_library_export(&self, report: &LibraryExportReport) {
        let folder = gio::File::for_path(&report.folder);
        match export_outcome::outcome_of(report) {
            ExportOutcome::Complete(count) => {
                if count > 0 {
                    self.show_export_toast(count, &folder);
                }
            }
            ExportOutcome::Partial { exported, missing } => {
                let alert = adw::AlertDialog::new(
                    Some(&export_outcome::partial_heading()),
                    Some(&export_outcome::partial_body(
                        exported,
                        &folder_name(&folder),
                        &missing,
                    )),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
        }
    }

    fn export_selected_as(&self) {
        let assets = self.targeted_assets();
        if assets.is_empty() {
            return;
        }
        let paused_frame = self.paused_preview_frame();
        let natural = output_natural_size(&assets[0]);
        let dialog = PigouneExportAsDialog::new(
            conversion_memory::load(self.settings(), natural),
            natural,
            assets.len(),
            screen_size::screen_of(self),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |settings| {
                    conversion_memory::store(window.settings(), &settings);
                    let assets = assets.clone();
                    let paused_frame = paused_frame.clone();
                    glib::spawn_future_local(async move {
                        window
                            .convert_into_folder(&assets, paused_frame, settings)
                            .await;
                    });
                }
            ),
        );
        dialog.present(Some(self));
    }

    fn paused_preview_frame(&self) -> Option<(gdk::Texture, usize)> {
        let imp = self.imp();
        (imp.window_stack.visible_child_name().as_deref() == Some(PREVIEW_PAGE))
            .then(|| imp.asset_preview.paused_frame())
            .flatten()
    }

    async fn convert_into_folder(
        &self,
        assets: &[PigouneAssetObject],
        paused_frame: Option<(gdk::Texture, usize)>,
        settings: ConversionSettings,
    ) {
        let ids: Vec<AssetId> = assets.iter().map(PigouneAssetObject::id).collect();
        if !self.output_is_allowed(&ids, Operation::Export).await {
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Convert and Export"))
            .accept_label(gettext("_Export"))
            .modal(true)
            .build();
        let Ok(folder) = dialog.select_folder_future(Some(self)).await else {
            return;
        };
        let Some(path) = folder.path() else {
            return;
        };
        let progress = adw::Toast::new(&conversion_report::progress_text(
            assets.len(),
            settings.format,
        ));
        progress.set_use_markup(false);
        progress.set_timeout(0);
        self.imp().toast_overlay.add_toast(progress.clone());
        let mut exported = 0;
        let mut failures = Vec::new();
        for asset in assets {
            match self
                .convert_one(asset, &path, paused_frame.clone(), settings)
                .await
            {
                Ok(()) => exported += 1,
                Err(reason) => failures.push(conversion_report::Failure {
                    name: asset.display_name(),
                    reason,
                }),
            }
        }
        progress.dismiss();
        if exported > 0 {
            self.show_folder_toast(
                &conversion_report::success_text(exported, settings.format, &folder_name(&folder)),
                &folder,
            );
        }
        if !failures.is_empty() {
            self.show_export_failures(&failures);
        }
    }

    async fn convert_one(
        &self,
        asset: &PigouneAssetObject,
        folder: &Path,
        paused_frame: Option<(gdk::Texture, usize)>,
        settings: ConversionSettings,
    ) -> Result<(), String> {
        let frame_number = paused_frame.as_ref().map(|(_, index)| index + 1);
        let (file, is_vector, natural) = self.output_source(asset).await?;
        let source = image_conversion::Source {
            file: &file,
            is_vector,
            natural,
            still: paused_frame.map(|(texture, _)| texture),
        };
        let converted = image_conversion::convert(&source, settings)
            .await
            .map_err(conversion_report::reason)?;
        Library::save_converted(
            asset.asset(),
            folder,
            conversion_report::name_suffix(frame_number, converted.name_suffix.as_deref())
                .as_deref(),
            settings.format.extension(),
            &converted.bytes,
        )
        .map(|_| ())
        .map_err(|error| error_messages::describe(&error))
    }

    async fn open_selected_with(&self) {
        let selected = self.targeted_ids();
        let [asset] = selected[..] else {
            return;
        };
        self.open_asset(asset, true).await;
    }

    async fn show_selected_in_files(&self) {
        let targeted = self.targeted_assets();
        let [asset] = targeted.as_slice() else {
            return;
        };
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(asset.source_file())));
        if let Err(error) = launcher.open_containing_folder_future(Some(self)).await
            && !is_dismissed(&error)
        {
            self.show_toast(&gettext("The file could not be shown in Files."));
        }
    }

    async fn open_asset(&self, asset: AssetId, choose_application: bool) {
        if !self.output_is_allowed(&[asset], Operation::Open).await {
            return;
        }
        let flats = self.flat_files_for(&[asset]).await;
        let prepared = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.opening_copy(asset, &flats));
        let copy = match prepared {
            Some(Ok(Some(copy))) => copy,
            Some(Err(error)) => {
                self.show_open_with_error(&error_messages::describe(&error));
                return;
            }
            Some(Ok(None)) | None => return,
        };
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(copy)));
        launcher.set_always_ask(choose_application);
        if let Err(error) = launcher.launch_future(Some(self)).await
            && !is_dismissed(&error)
        {
            self.show_open_with_error(error.message());
        }
    }

    async fn prepare_selected_as_wallpaper(&self) {
        let targeted = self.targeted_assets();
        let [asset] = targeted.as_slice() else {
            return;
        };
        if self.is_showing_trash() {
            return;
        }
        let Ok((file, is_vector, natural)) = self.output_source(asset).await else {
            self.show_toast(&gettext("The annotated image could not be created"));
            return;
        };
        let (mut screens, current) = screen_size::screens_of(self);
        if screens.is_empty() {
            let Some(size) = Dimensions::new(FALLBACK_SCREEN.0, FALLBACK_SCREEN.1) else {
                return;
            };
            screens.push(ScreenChoice {
                name: String::new(),
                size,
                scale: 1.0,
            });
        }
        let source = WallpaperSource {
            file,
            natural,
            name: asset.display_name(),
            is_vector,
            colors: asset
                .asset()
                .colors
                .iter()
                .map(|color| (color.average, asset_colors::color_name(color.family)))
                .collect(),
        };
        let asset = asset.clone();
        let dialog = PigouneWallpaperDialog::new(
            source,
            (screens, current),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |choice| {
                    let asset = asset.clone();
                    glib::spawn_future_local(async move {
                        window
                            .set_prepared_wallpaper(&asset, choice, choice.screen)
                            .await;
                    });
                }
            ),
        );
        dialog.present(Some(self));
    }

    async fn set_prepared_wallpaper(
        &self,
        asset: &PigouneAssetObject,
        choice: WallpaperChoice,
        screen: Dimensions,
    ) {
        let progress = PigouneWallpaperProgressDialog::new();
        progress.present(Some(self));
        let prepared = self
            .prepared_wallpaper(asset, choice, screen, &progress)
            .await;
        self.apply_wallpaper(prepared, &asset.display_name(), choice.adds_to_library)
            .await;
        progress.force_close();
    }

    async fn prepared_wallpaper(
        &self,
        asset: &PigouneAssetObject,
        choice: WallpaperChoice,
        screen: Dimensions,
        progress: &PigouneWallpaperProgressDialog,
    ) -> Result<PathBuf, String> {
        let (file, is_vector, natural) = self.output_source(asset).await?;
        let source = image_conversion::Source {
            file: &file,
            is_vector,
            natural,
            still: None,
        };
        let image = choice.framing.image_rect(
            Size {
                width: f64::from(natural.0),
                height: f64::from(natural.1),
            },
            Size {
                width: f64::from(screen.width()),
                height: f64::from(screen.height()),
            },
        );
        let bytes = image_conversion::wallpaper(
            &source,
            image,
            (screen.width(), screen.height()),
            choice.look,
            |step| progress.show_step(step),
        )
        .await
        .map_err(|error| format!("{error:?}"))?;
        let folder = wallpaper_folder()?.join(asset.id().to_string());
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        let file = folder.join(format!(
            "{}-{}x{}.png",
            asset.display_name().replace('/', "-"),
            screen.width(),
            screen.height()
        ));
        std::fs::write(&file, bytes).map_err(|error| error.to_string())?;
        Ok(file)
    }

    async fn apply_wallpaper(
        &self,
        prepared: Result<PathBuf, String>,
        name: &str,
        adds_to_library: bool,
    ) {
        let failure = gettext("Unable to set “{name}” as wallpaper").replace("{name}", name);
        let result = match prepared {
            Ok(file) => {
                let window = self.downgrade();
                let later_failure = failure.clone();
                let kept = adds_to_library.then(|| file.clone());
                wallpaper::set_wallpaper(&file, move |response| {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    match response {
                        WallpaperResponse::Applied => {
                            if let Some(kept) = kept.clone() {
                                glib::spawn_future_local(async move {
                                    window.import_paths(vec![kept]).await;
                                });
                            }
                        }
                        WallpaperResponse::Failed => window.show_toast(&later_failure),
                        WallpaperResponse::Cancelled => {}
                    }
                })
                .await
            }
            Err(reason) => Err(reason),
        };
        if let Err(reason) = result {
            glib::g_warning!("pigoune", "Unable to set the wallpaper: {reason}");
            self.show_toast(&failure);
        }
    }

    fn show_open_with_error(&self, details: &str) {
        let alert =
            adw::AlertDialog::new(Some(&gettext("Unable to Open the Asset")), Some(details));
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_export_toast(&self, count: usize, folder: &gio::File) {
        let message = ngettext(
            "{count} asset exported to “{name}”",
            "{count} assets exported to “{name}”",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{count}", &count.to_string())
        .replace("{name}", &folder_name(folder));
        self.show_folder_toast(&message, folder);
    }

    fn show_folder_toast(&self, message: &str, folder: &gio::File) {
        let toast = adw::Toast::new(message);
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
        toasts::announce(&self.imp().toast_overlay, &toast);
    }

    fn redo_typing(&self) -> bool {
        GtkWindowExt::focus(self).is_some_and(|focus| {
            (focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
                && focus.activate_action("text.redo", None).is_ok()
        })
    }

    fn undo_typing(&self) -> bool {
        GtkWindowExt::focus(self).is_some_and(|focus| {
            (focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
                && focus.activate_action("text.undo", None).is_ok()
        })
    }

    fn targeted_assets(&self) -> Vec<PigouneAssetObject> {
        let imp = self.imp();
        if self.is_previewing() {
            return imp.asset_preview.shown_asset().into_iter().collect();
        }
        imp.asset_grid.selected_assets()
    }

    fn is_previewing(&self) -> bool {
        self.imp().window_stack.visible_child_name().as_deref() == Some(PREVIEW_PAGE)
    }

    fn describe_targets(&self) {
        self.show_selection(&self.targeted_assets());
    }

    fn targeted_ids(&self) -> Vec<AssetId> {
        self.targeted_assets()
            .iter()
            .map(PigouneAssetObject::id)
            .collect()
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
            self.refresh_smart_collection_counts();
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
            self.refresh_smart_collection_counts();
        }
    }

    fn refresh_smart_collection_counts(&self) {
        let has_smart_collections = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.smart_collections().ok())
            .is_some_and(|collections| !collections.is_empty());
        if has_smart_collections {
            self.schedule_counts_refresh();
        }
    }

    fn apply_asset_command(&self, command: &AssetCommand) -> bool {
        let applied = self.change_library(|library| library.apply_asset_command(command));
        match applied {
            Some(Ok(_)) => true,
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Change the Asset")),
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
        let selected = self.targeted_ids();
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
        let selected = self.targeted_ids();
        if selected.is_empty() {
            return;
        }
        let single_name = match names.as_slice() {
            [name] => Some(name.clone()),
            _ => None,
        };
        let commands = names
            .into_iter()
            .map(|name| TagCommand::Add {
                assets: selected.clone(),
                name,
            })
            .collect();
        let tag_counts_before = self.tag_counts_of(&selected);
        if self.apply_tag_command(&TagCommand::Batch(commands)) {
            self.refresh_selected_tags();
            self.refresh_sidebar();
            let gained = self
                .tag_counts_of(&selected)
                .iter()
                .zip(&tag_counts_before)
                .filter(|(after, before)| after > before)
                .count();
            if gained > 0 {
                self.show_undoable_toast(&tags_added_text(single_name.as_deref(), gained));
            }
        }
    }

    fn tag_counts_of(&self, assets: &[AssetId]) -> Vec<usize> {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return vec![0; assets.len()];
        };
        assets
            .iter()
            .map(|asset| library.tags_of(*asset).map_or(0, |tags| tags.len()))
            .collect()
    }

    fn remove_tag_from_selected(&self, tag: TagId) {
        let selected = self.targeted_ids();
        if selected.is_empty() {
            return;
        }
        let linked = selected.len() - self.assets_without_tag(tag, &selected).len();
        let command = TagCommand::Unlink {
            tag,
            assets: selected.clone(),
        };
        if !self.apply_tag_command(&command) {
            return;
        }
        self.refresh_selected_tags();
        self.schedule_counts_refresh();
        self.drop_assets_leaving_view(&selected);
        if linked > 0 {
            self.show_undoable_toast(
                &ngettext(
                    "Tag “{name}” removed from {count} asset",
                    "Tag “{name}” removed from {count} assets",
                    u32::try_from(linked).unwrap_or(u32::MAX),
                )
                .replace("{count}", &linked.to_string())
                .replace("{name}", &self.tag_name(tag).unwrap_or_default()),
            );
        }
    }

    fn refresh_selected_collections(&self) {
        if let Some((shared, _)) = self.selected_collections() {
            self.imp().asset_details.show_collections(&shared);
        }
    }

    fn can_organize_selection(&self) -> bool {
        !self.is_showing_trash() && !self.targeted_ids().is_empty()
    }

    fn ask_collection_for_selected(&self) {
        if !self.can_organize_selection() {
            return;
        }
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
            &gettext("Add to a Collection"),
            all,
            held_by_all,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |collection| {
                    let assets = window.targeted_ids();
                    if !assets.is_empty() {
                        window.drop_assets_on_collection(collection, &assets, true);
                    }
                }
            ),
        );
    }

    fn ask_collection_to_move_selected(&self) {
        let AssetView::Collection(source) = self.displayed_view() else {
            return;
        };
        let assets = self.targeted_ids();
        let Some((_, all)) = self.selected_collections() else {
            return;
        };
        if assets.is_empty() {
            return;
        }
        collection_chooser::present(
            self,
            &gettext("Move to a Collection"),
            all,
            vec![source],
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |collection| window.drop_assets_on_collection(collection, &assets, false)
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

    fn find_similar_to_selected(&self) {
        let selected = self.imp().asset_grid.selected_assets();
        let [only] = selected.as_slice() else {
            return;
        };
        let Some((name, outcome)) = self.similar_outcome(only.id()) else {
            return;
        };
        let dialog = PigouneSimilarDialog::new();
        dialog.connect_show_in_all(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |id| window.show_in_all(id)
        ));
        dialog.show_outcome(&name, outcome);
        dialog.present(Some(self));
    }

    fn similar_outcome(&self, id: AssetId) -> Option<(String, SimilarOutcome)> {
        let library = self.imp().library.borrow();
        let library = library.as_ref()?;
        let asset = library.asset(id).ok().flatten()?;
        let outcome = match library.similar_assets(id, SIMILAR_DISTANCE).ok()? {
            SimilarSearch::Pending => SimilarOutcome::Pending,
            SimilarSearch::NoFingerprint => SimilarOutcome::NoFingerprint,
            SimilarSearch::Found(similar) if similar.is_empty() => SimilarOutcome::NothingSimilar,
            SimilarSearch::Found(similar) => SimilarOutcome::Found(
                similar
                    .into_iter()
                    .take(MOST_SIMILAR_SHOWN)
                    .filter_map(|similar| {
                        let other = library.asset(similar.asset).ok().flatten()?;
                        Some(SimilarEntry {
                            id: other.id,
                            summary: asset_facts::summary_text(&other, None),
                            distance: similar.distance,
                            file: library.file_of(&other),
                            thumbnail_file: library.thumbnail_file(other.id, THUMBNAIL_PIXELS),
                            name: other.display_name,
                        })
                    })
                    .collect(),
            ),
        };
        Some((asset.display_name, outcome))
    }

    fn show_in_all(&self, id: AssetId) {
        let imp = self.imp();
        self.reset_search();
        imp.current_view.set(AssetView::All);
        self.remember_view(AssetView::All);
        imp.asset_preview.close();
        self.refresh_sidebar_revealing(Vec::new());
        self.refresh_grid();
        imp.sidebar.point_out(AssetView::All);
        imp.asset_grid.select_asset(id);
        imp.asset_grid.point_out_selected_next();
    }

    fn open_tag_from_details(&self, tag: TagId) {
        let before = self.selected_ids();
        settings::store_bool(self.settings(), settings::SIDEBAR_TAGS_EXPANDED, true);
        self.go_to_view(AssetView::Tag(tag), Vec::new());
        self.tell_lost_selection(&before);
    }

    fn go_to_view(&self, view: AssetView, reveal: Vec<CollectionId>) {
        let imp = self.imp();
        self.note_view_chosen_by_click();
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
        let selected = self.targeted_ids();
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
            Ok::<_, LibraryError>((
                held,
                library.collection_paths()?,
                library.visible_collections()?,
            ))
        });
        if let Some(Ok((held, mut all, collections))) = read {
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
                    let look = collections
                        .iter()
                        .find(|collection| collection.id == path.id)
                        .map(|collection| collection.look.clone())
                        .unwrap_or_default();
                    Some(SharedCollection {
                        path: path.clone(),
                        look,
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

    fn holds_any_collection(&self, selected: &[PigouneAssetObject]) -> bool {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return false;
        };
        let ids: Vec<AssetId> = selected.iter().map(PigouneAssetObject::id).collect();
        library
            .unclassify_command(&ids)
            .is_ok_and(|command| command.is_some())
    }

    fn unclassify_selected(&self) {
        if self.can_organize_selection() {
            self.drop_assets_on_unclassified(&self.targeted_ids());
        }
    }

    fn remove_selected_from_collection(&self, collection: CollectionId) {
        let holders: Vec<AssetId> = {
            let library = self.imp().library.borrow();
            let Some(library) = library.as_ref() else {
                return;
            };
            self.targeted_ids()
                .into_iter()
                .filter(|asset| {
                    library
                        .collections_of(*asset)
                        .is_ok_and(|held| held.contains(&collection))
                })
                .collect()
        };
        if holders.is_empty() {
            return;
        }
        let command = CollectionCommand::RemoveAssets {
            collection,
            assets: holders.clone(),
        };
        if self.apply_collection_change(&command, &holders) {
            self.show_undoable_toast(
                &ngettext(
                    "{count} asset removed from “{name}”",
                    "{count} assets removed from “{name}”",
                    u32::try_from(holders.len()).unwrap_or(u32::MAX),
                )
                .replace("{count}", &holders.len().to_string())
                .replace(
                    "{name}",
                    &self.collection_name(collection).unwrap_or_default(),
                ),
            );
        }
    }

    fn directly_holds_any(
        &self,
        collection: CollectionId,
        selected: &[PigouneAssetObject],
    ) -> bool {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return false;
        };
        selected.iter().any(|asset| {
            library
                .collections_of(asset.id())
                .is_ok_and(|collections| collections.contains(&collection))
        })
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
            drop_message::describe(hovered, self.dragged_from_view(), library.as_ref()?)
        });
        imp.asset_grid.show_drag_caption(text.as_deref());
    }

    fn drop_assets_on(&self, target: AssetView, assets: &[AssetId], keep_source: bool) {
        if self.is_showing_trash() {
            return;
        }
        match target {
            AssetView::Trash => {
                self.set_trashed(assets, true);
            }
            AssetView::Collection(to) => self.drop_assets_on_collection(to, assets, keep_source),
            AssetView::Tag(tag) => self.drop_assets_on_tag(tag, assets),
            AssetView::Favorites => self.drop_assets_on_favorites(assets),
            AssetView::Unclassified => self.drop_assets_on_unclassified(assets),
            AssetView::All | AssetView::Smart(_) => {}
        }
    }

    fn drop_assets_on_unclassified(&self, assets: &[AssetId]) {
        if self.dragged_from_view() == AssetView::Unclassified {
            return;
        }
        let command = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.unclassify_command(assets).ok().flatten());
        let Some(command) = command else {
            let count = assets.len();
            self.show_toast(
                &ngettext(
                    "{count} asset is already unclassified",
                    "{count} assets are already unclassified",
                    u32::try_from(count).unwrap_or(u32::MAX),
                )
                .replace("{count}", &count.to_string()),
            );
            return;
        };
        let count = command.affected_assets();
        if self.apply_collection_change(&command, assets) {
            self.show_undoable_toast(
                &ngettext(
                    "{count} asset removed from all its collections",
                    "{count} assets removed from all their collections",
                    u32::try_from(count).unwrap_or(u32::MAX),
                )
                .replace("{count}", &count.to_string()),
            );
        }
    }

    fn drop_assets_on_collection(&self, to: CollectionId, assets: &[AssetId], keep_source: bool) {
        let name = self.collection_name(to).unwrap_or_default();
        let (command, message, count) = match self.dragged_from_view() {
            AssetView::Collection(from) if from == to && !keep_source => return,
            AssetView::Collection(from) if !keep_source => (
                CollectionCommand::MoveAssets {
                    from,
                    to,
                    assets: assets.to_vec(),
                },
                ngettext(
                    "{count} asset moved to “{name}”",
                    "{count} assets moved to “{name}”",
                    u32::try_from(assets.len()).unwrap_or(u32::MAX),
                ),
                assets.len(),
            ),
            AssetView::All | AssetView::Unclassified if !keep_source => {
                let Some(command) = self.move_to_collection_command(to, assets) else {
                    self.show_already_in_toast(&name, assets.len());
                    return;
                };
                let moved = command.affected_assets();
                let plural = u32::try_from(moved).unwrap_or(u32::MAX);
                let message = if command.removes_assets() {
                    ngettext(
                        "{count} asset moved to “{name}” and removed from its other collections",
                        "{count} assets moved to “{name}” and removed from their other collections",
                        plural,
                    )
                } else {
                    ngettext(
                        "{count} asset moved to “{name}”",
                        "{count} assets moved to “{name}”",
                        plural,
                    )
                };
                (command, message, moved)
            }
            _ => {
                let fresh = self.assets_outside_collection(to, assets);
                if fresh.is_empty() {
                    self.show_already_in_toast(&name, assets.len());
                    return;
                }
                let count = fresh.len();
                (
                    CollectionCommand::AddAssets {
                        collection: to,
                        assets: fresh,
                    },
                    ngettext(
                        "{count} asset added to “{name}”",
                        "{count} assets added to “{name}”",
                        u32::try_from(count).unwrap_or(u32::MAX),
                    ),
                    count,
                )
            }
        };
        if self.apply_collection_change(&command, assets) {
            self.show_undoable_toast(
                &message
                    .replace("{count}", &count.to_string())
                    .replace("{name}", &name),
            );
        }
    }

    fn move_to_collection_command(
        &self,
        target: CollectionId,
        assets: &[AssetId],
    ) -> Option<CollectionCommand> {
        self.imp().library.borrow().as_ref().and_then(|library| {
            library
                .move_to_collection_command(assets, target)
                .ok()
                .flatten()
        })
    }

    fn show_already_in_toast(&self, name: &str, count: usize) {
        self.show_toast(
            &ngettext(
                "{count} asset is already in “{name}”",
                "{count} assets are already in “{name}”",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
            .replace("{count}", &count.to_string())
            .replace("{name}", name),
        );
    }

    fn assets_outside_collection(
        &self,
        collection: CollectionId,
        assets: &[AssetId],
    ) -> Vec<AssetId> {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Vec::new();
        };
        assets
            .iter()
            .copied()
            .filter(|asset| {
                library
                    .collections_of(*asset)
                    .is_ok_and(|held| !held.contains(&collection))
            })
            .collect()
    }

    fn assets_without_tag(&self, tag: TagId, assets: &[AssetId]) -> Vec<AssetId> {
        let library = self.imp().library.borrow();
        let Some(library) = library.as_ref() else {
            return Vec::new();
        };
        assets
            .iter()
            .copied()
            .filter(|asset| {
                library
                    .tags_of(*asset)
                    .is_ok_and(|carried| carried.iter().all(|found| found.id != tag))
            })
            .collect()
    }

    fn drop_assets_on_favorites(&self, assets: &[AssetId]) {
        let imp = self.imp();
        let names: Vec<(AssetId, bool, String)> = imp
            .library
            .borrow()
            .as_ref()
            .map(|library| {
                assets
                    .iter()
                    .filter_map(|id| library.asset(*id).ok().flatten())
                    .map(|asset| (asset.id, asset.is_favorite, asset.display_name))
                    .collect()
            })
            .unwrap_or_default();
        let fresh: Vec<&(AssetId, bool, String)> =
            names.iter().filter(|(_, favorite, _)| !favorite).collect();
        let Some((_, _, first_name)) = fresh.first().copied().or(names.first()) else {
            return;
        };
        if fresh.is_empty() {
            let text = if names.len() == 1 {
                gettext("“{name}” is already in the favorites").replace("{name}", first_name)
            } else {
                ngettext(
                    "{count} asset is already in the favorites",
                    "{count} assets are already in the favorites",
                    u32::try_from(names.len()).unwrap_or(u32::MAX),
                )
                .replace("{count}", &names.len().to_string())
            };
            self.show_toast(&text);
            return;
        }
        let fresh_ids: Vec<AssetId> = fresh.iter().map(|(id, _, _)| *id).collect();
        let applied = self.change_library(|library| {
            library.apply_asset_command(&AssetCommand::SetFavorite {
                assets: fresh_ids.clone(),
                favorite: true,
            })
        });
        match applied {
            Some(Ok(_)) => {
                for asset in imp.asset_grid.visible_assets() {
                    if fresh_ids.contains(&asset.id()) {
                        asset.set_favorite(true);
                    }
                }
                self.schedule_counts_refresh();
                self.show_favorite_toast(true, &fresh_ids);
            }
            Some(Err(error)) => self.show_favorite_error(&error),
            None => {}
        }
    }

    fn show_favorite_toast(&self, favorite: bool, assets: &[AssetId]) {
        let first_name = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.asset(*assets.first()?).ok().flatten())
            .map(|asset| asset.display_name)
            .unwrap_or_default();
        let count = assets.len();
        let plural = u32::try_from(count).unwrap_or(u32::MAX);
        let text = match (favorite, count) {
            (true, 1) => gettext("“{name}” added to the favorites"),
            (false, 1) => gettext("“{name}” removed from the favorites"),
            (true, _) => ngettext(
                "{count} asset added to the favorites",
                "{count} assets added to the favorites",
                plural,
            ),
            (false, _) => ngettext(
                "{count} asset removed from the favorites",
                "{count} assets removed from the favorites",
                plural,
            ),
        };
        self.show_undoable_toast(
            &text
                .replace("{count}", &count.to_string())
                .replace("{name}", &first_name),
        );
    }

    fn show_favorite_error(&self, error: &AssetError) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Change the Favorites")),
            Some(&error_messages::describe_asset(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn drop_assets_on_tag(&self, tag: TagId, assets: &[AssetId]) {
        let name = self.tag_name(tag).unwrap_or_default();
        let fresh = self.assets_without_tag(tag, assets);
        if fresh.is_empty() {
            self.show_toast(
                &ngettext(
                    "{count} asset already has the tag “{name}”",
                    "{count} assets already have the tag “{name}”",
                    u32::try_from(assets.len()).unwrap_or(u32::MAX),
                )
                .replace("{count}", &assets.len().to_string())
                .replace("{name}", &name),
            );
            return;
        }
        let count = fresh.len();
        if !self.apply_tag_command(&TagCommand::Link { tag, assets: fresh }) {
            return;
        }
        self.refresh_selected_tags();
        self.schedule_counts_refresh();
        self.drop_assets_leaving_view(assets);
        self.show_undoable_toast(&tags_added_text(Some(&name), count));
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
        let changes_parent = plan
            .commands
            .iter()
            .any(|command| matches!(command, CollectionCommand::Move { .. }));
        let applied = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Batch(plan.commands))?;
            let parent = library.collection(dragged)?.and_then(|moved| moved.parent);
            Ok::<_, pigoune_core::CollectionError>((ancestors(library, parent), parent))
        });
        let (reveal, parent) = match applied {
            Some(Ok(placed)) => placed,
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
        if changes_parent {
            let name = self.collection_name(dragged).unwrap_or_default();
            let parent_name = parent.and_then(|parent| self.collection_name(parent));
            self.show_collection_moved_toast(&name, parent_name.as_deref());
        } else if self.imp().fresh_change.get().is_some() {
            self.show_undoable_toast(&gettext("The order of the collections was changed."));
        }
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
        toasts::announce(&self.imp().toast_overlay, &adw::Toast::new(text));
    }

    fn drop_assets_leaving_view(&self, assets: &[AssetId]) {
        let imp = self.imp();
        let view = self.displayed_view();
        let leaving: Vec<AssetId> = imp
            .library
            .borrow()
            .as_ref()
            .map(|library| {
                library
                    .assets_outside_view(view, assets)
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if leaving.is_empty() {
            return;
        }
        imp.asset_grid.remove_assets(&leaving);
        if imp.asset_grid.is_empty() {
            self.refresh_grid();
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
            Some(&self.describe_tag_error(error)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn describe_tag_error(&self, error: &TagError) -> String {
        if let TagError::NameTaken(existing) = error
            && let Some(text) = self.describe_taken_name(*existing)
        {
            return text;
        }
        error_messages::describe_tag(error)
    }

    fn describe_taken_name(&self, existing: TagId) -> Option<String> {
        let path = self
            .imp()
            .library
            .borrow()
            .as_ref()?
            .tag_path(existing)
            .ok()?;
        let (tag, parents) = path.split_last()?;
        Some(match parents.last() {
            Some(parent) => gettext("A tag named “{name}” already exists in “{parent}”.")
                .replace("{name}", &tag.name)
                .replace("{parent}", &parent.name),
            None => gettext("A tag named “{name}” already exists at the top level.")
                .replace("{name}", &tag.name),
        })
    }

    fn tag_is_already_in(&self, tag: TagId, parent: Option<TagId>) -> bool {
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.tag(tag).ok().flatten())
            .is_some_and(|found| found.parent == parent)
    }

    fn tag_name(&self, tag: TagId) -> Option<String> {
        self.imp()
            .library
            .borrow()
            .as_ref()?
            .tag(tag)
            .ok()?
            .map(|found| found.name)
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
            Some(Err(error)) => Err(self.describe_tag_error(&error)),
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
                move |_, _| window.merge_tag(from, into)
            ),
        );
        alert.present(Some(self));
    }

    fn current_filter(&self) -> AssetFilter {
        let imp = self.imp();
        AssetFilter {
            text: imp.search_query.borrow().clone(),
            ..imp.filters.borrow().clone()
        }
    }

    fn ask_new_smart_collection(&self) {
        let filter = self.current_filter();
        let filter = match self.imp().current_view.get() {
            AssetView::Trash | AssetView::Smart(_) => AssetFilter::default(),
            AssetView::Favorites => AssetFilter {
                favorites_only: true,
                ..filter
            },
            AssetView::All
            | AssetView::Unclassified
            | AssetView::Collection(_)
            | AssetView::Tag(_) => filter,
        };
        let wanted = gettext("Smart Collection");
        let taken: Vec<String> = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.smart_collections().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|collection| collection.name)
            .collect();
        let name = free_name(&wanted, &taken);
        let dialog = PigouneSmartCollectionDialog::new(
            &SmartCollectionDraft {
                title: &gettext("New Smart Collection"),
                confirm_label: &gettext("_Create"),
                name: &name,
                filter: &filter,
            },
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name, filter| window.create_smart_collection(name, filter)
            ),
        );
        dialog.present(Some(self));
    }

    fn scope_name(&self, scope: AssetView) -> String {
        match scope {
            AssetView::All => gettext("All"),
            AssetView::Favorites => gettext("Favorites"),
            AssetView::Unclassified => gettext("Unclassified"),
            AssetView::Collection(_)
            | AssetView::Tag(_)
            | AssetView::Smart(_)
            | AssetView::Trash => self
                .imp()
                .library
                .borrow()
                .as_ref()
                .map(|library| view_name(library, scope))
                .unwrap_or_default(),
        }
    }

    fn create_smart_collection(&self, name: &str, filter: &AssetFilter) -> Result<(), String> {
        let created = self.change_library(|library| library.create_smart_collection(name, filter));
        match created {
            Some(Ok(id)) => {
                self.reset_search();
                let view = AssetView::Smart(id);
                self.imp().current_view.set(view);
                self.remember_view(view);
                self.refresh_assets();
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_smart_collection(&error)),
            None => Ok(()),
        }
    }

    fn ask_smart_collection_changes(&self, id: SmartCollectionId) {
        let Some(collection) = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.smart_collection(id).ok().flatten())
        else {
            return;
        };
        let created_at_unix_ms = collection.created_at_unix_ms;
        let position = collection.position;
        let dialog = PigouneSmartCollectionDialog::new(
            &SmartCollectionDraft {
                title: &gettext("Edit Smart Collection"),
                confirm_label: &gettext("_Save"),
                name: &collection.name,
                filter: &collection.filter,
            },
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name, filter| {
                    window.change_smart_collection(&SmartCollectionCommand::Update {
                        collection: SmartCollection {
                            id,
                            name: name.to_owned(),
                            filter: filter.clone(),
                            position,
                            created_at_unix_ms,
                        },
                    })
                }
            ),
        );
        dialog.present(Some(self));
    }

    fn smart_collection_name(&self, collection: SmartCollectionId) -> Option<String> {
        Some(
            self.imp()
                .library
                .borrow()
                .as_ref()?
                .smart_collection(collection)
                .ok()??
                .name,
        )
    }

    fn change_smart_collection(&self, command: &SmartCollectionCommand) -> Result<(), String> {
        let changed =
            self.change_library(|library| library.apply_smart_collection_command(command));
        match changed {
            Some(Ok(_)) => {
                self.refresh_assets();
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_smart_collection(&error)),
            None => Ok(()),
        }
    }

    fn ask_smart_collection_deletion(&self, collection: SmartCollectionId) {
        let Some(name) = self.smart_collection_name(collection) else {
            return;
        };
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete the Smart Collection “{name}”?").replace("{name}", &name)),
            Some(&gettext(
                "Only the saved search is deleted. The assets it shows stay in the library.",
            )),
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
                move |_, _| window.delete_smart_collection(collection)
            ),
        );
        alert.present(Some(self));
    }

    fn delete_smart_collection(&self, collection: SmartCollectionId) {
        let name = self.smart_collection_name(collection).unwrap_or_default();
        let deleted =
            self.change_smart_collection(&SmartCollectionCommand::Delete { id: collection });
        match deleted {
            Ok(()) => self.show_undoable_toast(
                &gettext("The smart collection “{name}” was deleted.").replace("{name}", &name),
            ),
            Err(message) => self.show_smart_collection_error(&message),
        }
    }

    fn show_smart_collection_error(&self, message: &str) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unable to Change the Smart Collection")),
            Some(message),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn ask_tag_deletion(&self, tag: TagId) {
        let Some(name) = self.tag_name(tag) else {
            return;
        };
        let (used_by, sub_tags) = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| {
                let used_by = library.view_count(AssetView::Tag(tag)).unwrap_or(0);
                let sub_tags = library
                    .tags()
                    .map_or(0, |all| tag_tree::sub_tag_count(&all, tag));
                (used_by, sub_tags)
            })
            .unwrap_or_default();
        let body = if sub_tags == 0 {
            ngettext(
                "It will be removed from {count} asset. The assets stay in the library.",
                "It will be removed from {count} assets. The assets stay in the library.",
                u32::try_from(used_by).unwrap_or(u32::MAX),
            )
            .replace("{count}", &used_by.to_string())
        } else {
            ngettext(
                "It contains {count} sub-tag. The assets stay in the library.",
                "It contains {count} sub-tags. The assets stay in the library.",
                u32::try_from(sub_tags).unwrap_or(u32::MAX),
            )
            .replace("{count}", &sub_tags.to_string())
        };
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete the Tag “{name}”?").replace("{name}", &name)),
            Some(&body),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Cancel"));
        if sub_tags > 0 {
            alert.add_response(
                KEEP_SUB_TAGS_RESPONSE,
                &gettext("Delete and _Move Sub-tags Up"),
            );
        }
        alert.add_response(
            DELETE_RESPONSE,
            &if sub_tags > 0 {
                gettext("_Delete with Sub-tags")
            } else {
                gettext("_Delete")
            },
        );
        alert.set_response_appearance(DELETE_RESPONSE, adw::ResponseAppearance::Destructive);
        alert.set_close_response(CLOSE_RESPONSE);
        for (response, dissolve) in [(DELETE_RESPONSE, false), (KEEP_SUB_TAGS_RESPONSE, true)] {
            alert.connect_response(
                Some(response),
                glib::clone!(
                    #[weak(rename_to = window)]
                    self,
                    move |_, _| {
                        window.delete_tag(tag, dissolve);
                    }
                ),
            );
        }
        alert.present(Some(self));
    }

    fn delete_tag(&self, tag: TagId, dissolve: bool) {
        let Some(name) = self.tag_name(tag) else {
            return;
        };
        let has_sub_tags = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.sub_tags(tag).ok())
            .is_some_and(|below| !below.is_empty());
        let command = if dissolve {
            TagCommand::Dissolve { tag }
        } else {
            TagCommand::Delete { tag }
        };
        let previous_view = self.imp().current_view.get();
        let landing = self.view_after_deleting(tag, dissolve, previous_view);
        let landing_name = landing.and_then(|view| match view {
            AssetView::Tag(parent) => self.tag_name(parent),
            _ => None,
        });
        if !self.apply_tag_command(&command) {
            return;
        }
        if let Some(landing) = landing {
            self.imp().current_view.set(landing);
            self.remember_view(landing);
            self.imp().view_to_restore.set(
                self.imp()
                    .fresh_change
                    .get()
                    .map(|stamp| (stamp, previous_view)),
            );
        }
        self.refresh_assets();
        self.refresh_selected_tags();
        let text = match (dissolve, has_sub_tags) {
            (true, _) => gettext("The tag “{name}” was deleted and its sub-tags moved up."),
            (false, true) => gettext("The tag “{name}” and its sub-tags were deleted."),
            (false, false) => gettext("The tag “{name}” was deleted."),
        }
        .replace("{name}", &name);
        let text = match (landing, landing_name) {
            (_, Some(parent)) => {
                format!(
                    "{text} {}",
                    gettext("Showing “{name}”.").replace("{name}", &parent)
                )
            }
            (Some(_), None) => format!("{text} {}", gettext("Showing the whole library.")),
            (None, _) => text,
        };
        self.show_undoable_toast(&text);
    }

    fn view_after_deleting(
        &self,
        tag: TagId,
        dissolve: bool,
        current: AssetView,
    ) -> Option<AssetView> {
        let library = self.imp().library.borrow();
        let library = library.as_ref()?;
        let tags = library.tags().ok()?;
        let parent = tags.iter().find(|known| known.id == tag)?.parent;
        let removed = if dissolve {
            vec![tag]
        } else {
            tag_tree::descendants(&tags, tag)
        };
        displayed_view::view_after_tag_removal(current, &removed, parent)
    }

    fn ask_sub_tag_name(&self, parent: TagId) {
        let dialog = PigouneCollectionNameDialog::new(
            &gettext("New Sub-tag"),
            &gettext("_Create"),
            "",
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |name| window.create_sub_tag(parent, name)
            ),
        );
        dialog.present(Some(self));
    }

    fn create_sub_tag(&self, parent: TagId, name: &str) -> Result<(), String> {
        let path = {
            let library = self.imp().library.borrow();
            let Some(library) = library.as_ref() else {
                return Ok(());
            };
            let mut names: Vec<String> = library
                .tag_path(parent)
                .map_err(|error| error_messages::describe(&error))?
                .into_iter()
                .map(|step| step.name)
                .collect();
            names.push(name.trim().to_owned());
            names.join("/")
        };
        if name.contains('/') {
            return Err(gettext("A tag name cannot contain a slash."));
        }
        let taken = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.tag_named_in(parent, name).ok().flatten());
        if let Some(existing) = taken {
            return Err(self.describe_tag_error(&TagError::NameTaken(existing.id)));
        }
        let created = self.change_library(|library| {
            library.apply_tag_command(&TagCommand::Add {
                assets: Vec::new(),
                name: path,
            })
        });
        match created {
            Some(Ok(_)) => {
                self.go_to_view(AssetView::Tag(parent), Vec::new());
                Ok(())
            }
            Some(Err(error)) => Err(self.describe_tag_error(&error)),
            None => Ok(()),
        }
    }

    fn ask_tag_parent(&self, tag: TagId) {
        self.ask_tag_destination(
            tag,
            &gettext("Move the Tag “{name}”"),
            Some(&gettext("Top Level")),
            true,
            move |window, destination| window.move_tag(tag, destination),
        );
    }

    fn ask_tag_to_merge_into(&self, tag: TagId) {
        self.ask_tag_destination(
            tag,
            &gettext("Merge the Tag “{name}” Into"),
            None,
            false,
            move |window, destination| {
                if let Some(into) = destination {
                    window.merge_tag(tag, into);
                }
            },
        );
    }

    fn ask_tag_destination(
        &self,
        tag: TagId,
        title: &str,
        top_level_label: Option<&str>,
        leave_current_parent: bool,
        on_chosen: impl Fn(&Self, Option<TagId>) + 'static,
    ) {
        let Some(name) = self.tag_name(tag) else {
            return;
        };
        let Some(tags) = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.tags().ok())
        else {
            return;
        };
        let choices = tag_chooser::destinations(&tags, tag, top_level_label, leave_current_parent);
        tag_chooser::present(
            self,
            &title.replace("{name}", &name),
            choices,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |destination| on_chosen(&window, destination)
            ),
        );
    }

    fn move_tag(&self, tag: TagId, parent: Option<TagId>) {
        if self.tag_is_already_in(tag, parent) {
            return;
        }
        if self.apply_tag_command(&TagCommand::Move { tag, parent }) {
            self.refresh_sidebar();
            self.refresh_grid();
            self.refresh_selected_tags();
            let name = self.tag_name(tag).unwrap_or_default();
            let text = match parent.and_then(|parent| self.tag_name(parent)) {
                Some(parent_name) => gettext("The tag “{name}” was moved into “{parent}”.")
                    .replace("{name}", &name)
                    .replace("{parent}", &parent_name),
                None => {
                    gettext("The tag “{name}” was moved to the top level.").replace("{name}", &name)
                }
            };
            self.show_undoable_toast(&text);
        }
    }

    fn merge_tag(&self, from: TagId, into: TagId) {
        let from_name = self.tag_name(from).unwrap_or_default();
        let into_name = self.tag_name(into).unwrap_or_default();
        let previous_view = self.imp().current_view.get();
        if self.apply_tag_command(&TagCommand::Merge { from, into }) {
            if previous_view == AssetView::Tag(from) {
                self.imp().current_view.set(AssetView::Tag(into));
                self.remember_view(AssetView::Tag(into));
                self.imp().view_to_restore.set(
                    self.imp()
                        .fresh_change
                        .get()
                        .map(|stamp| (stamp, previous_view)),
                );
            }
            self.refresh_assets();
            self.refresh_selected_tags();
            self.show_undoable_toast(
                &gettext("The tag “{from}” was merged into “{into}”.")
                    .replace("{from}", &from_name)
                    .replace("{into}", &into_name),
            );
        }
    }

    fn connect_preview(&self) {
        let imp = self.imp();
        imp.asset_grid.connect_preview_requested(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |position| window.activate_asset(position)
        ));
        imp.asset_preview.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |last| window.leave_preview(last.as_ref())
        ));
        imp.asset_preview.connect_shown(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.describe_targets()
        ));
    }

    fn connect_editor(&self) {
        self.imp().editor.connect_closed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.leave_editor()
        ));
        self.imp().editor.connect_save(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            false,
            move |capture| window.save_capture(capture)
        ));
        self.imp().editor.connect_notice(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |text| window.show_toast(text)
        ));
    }

    fn save_item_annotations(&self, id: AssetId, capture: &EditedCapture) -> bool {
        let stored = self
            .imp()
            .library
            .borrow_mut()
            .as_mut()
            .map(|library| library.save_annotations(id, &capture.document));
        match stored {
            Some(Ok(())) => {
                self.imp().asset_grid.forget_thumbnails();
                self.refresh_assets();
                self.leave_editor();
                let grid = &self.imp().asset_grid;
                grid.select_asset(id);
                grid.reveal_asset(id);
                self.show_toast(&gettext("Annotations saved"));
                true
            }
            Some(Err(_)) => {
                self.show_capture_not_saved();
                false
            }
            None => false,
        }
    }

    fn show_capture_not_saved(&self) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Could Not Save the Capture")),
            Some(&gettext("The capture could not be saved to the library.")),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn save_capture(&self, capture: &EditedCapture) -> bool {
        if let Some(id) = capture.item {
            return self.save_item_annotations(id, capture);
        }
        let folder = std::env::temp_dir().join(format!("pigoune-capture-{}", std::process::id()));
        let image = folder.join(format!("{}.png", pasted_content::capture_name()));
        let written = fs::remove_dir_all(&folder)
            .or_else(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(error),
            })
            .and_then(|()| fs::create_dir_all(&folder))
            .and_then(|()| fs::write(&image, &capture.source_png));
        let stored = match written {
            Ok(()) => self
                .imp()
                .library
                .borrow_mut()
                .as_mut()
                .map(|library| library.import_with_annotations(&image, &capture.document)),
            Err(_) => Some(Err(AnnotatedError::Unreadable(image.clone()))),
        };
        let _ = fs::remove_dir_all(&folder);
        match stored {
            Some(Ok(id)) => {
                self.capture_saved(id);
                true
            }
            Some(Err(AnnotatedError::AlreadyPresent(existing))) => {
                self.show_capture_already_present(existing);
                false
            }
            Some(Err(_)) => {
                self.show_capture_not_saved();
                false
            }
            None => false,
        }
    }

    fn capture_saved(&self, id: AssetId) {
        self.apply_tag_command(&TagCommand::Add {
            assets: vec![id],
            name: CAPTURE_TAG.to_owned(),
        });
        self.refresh_assets();
        self.analyse_colors();
        self.leave_editor();
        let grid = &self.imp().asset_grid;
        grid.select_asset(id);
        grid.reveal_asset(id);
        self.show_toast(&gettext("Capture saved to the library"));
    }

    fn show_capture_already_present(&self, existing: AssetId) {
        let name = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.asset(existing).ok().flatten())
            .map_or_else(String::new, |asset| asset.display_name);
        let alert = adw::AlertDialog::new(
            Some(&gettext("Image Already in the Library")),
            Some(&gettext("This image is already stored as “{name}”.").replace("{name}", &name)),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_editor(&self) {
        let imp = self.imp();
        self.set_library_actions_enabled(false);
        imp.window_stack.set_visible_child_name(EDITOR_PAGE);
        self.refresh_import_availability();
    }

    fn offer_draft(&self) {
        if self.is_editing() {
            return;
        }
        let draft = capture_draft::Draft::standard();
        let Some((original, document)) = draft.load() else {
            draft.clear();
            return;
        };
        let alert = adw::AlertDialog::new(
            Some(&gettext("Resume the Unsaved Capture?")),
            Some(&gettext(
                "A capture was being edited when Pigoune was closed. You can resume where you left off.",
            )),
        );
        alert.add_response("discard", &gettext("_Discard"));
        alert.add_response("resume", &gettext("_Resume"));
        alert.set_response_appearance("resume", adw::ResponseAppearance::Suggested);
        alert.set_default_response(Some("resume"));
        alert.set_close_response("discard");
        alert.connect_response(
            None,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, response| {
                    if response == "resume"
                        && window
                            .imp()
                            .editor
                            .resume(&original, document.clone())
                            .is_ok()
                    {
                        window.show_editor();
                    } else {
                        capture_draft::Draft::standard().clear();
                    }
                }
            ),
        );
        alert.present(Some(self));
    }

    fn leave_editor(&self) {
        let imp = self.imp();
        if !self.is_editing() {
            return;
        }
        imp.editor.close();
        capture_draft::Draft::standard().clear();
        imp.window_stack.set_visible_child_name(MAIN_PAGE);
        self.set_library_actions_enabled(imp.library.borrow().is_some());
        self.refresh_import_availability();
        imp.asset_grid.focus_selected_later();
    }

    fn is_editing(&self) -> bool {
        self.imp().window_stack.visible_child_name().as_deref() == Some(EDITOR_PAGE)
    }

    fn lend_details_to_preview(&self, lent: bool) {
        let imp = self.imp();
        let details = imp.asset_details.get().upcast::<gtk::Widget>();
        imp.asset_details.set_beside_preview(lent);
        if lent {
            imp.details_view.set_content(None::<&gtk::Widget>);
            imp.asset_preview.hold_details(Some(&details));
        } else {
            imp.asset_preview.hold_details(None);
            imp.details_view.set_content(Some(&details));
        }
    }

    fn activate_asset(&self, activated: Option<u32>) {
        let opens_in_application = self.settings().boolean(settings::DOUBLE_CLICK_OPENS);
        let asset = activated
            .filter(|_| opens_in_application && !self.is_showing_trash())
            .and_then(|position| usize::try_from(position).ok())
            .and_then(|index| self.imp().asset_grid.visible_assets().get(index).cloned());
        match asset {
            Some(asset) => {
                let window = self.clone();
                glib::spawn_future_local(async move {
                    window.open_asset(asset.id(), false).await;
                });
            }
            None => self.open_preview(activated),
        }
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
        let flight_start = imp
            .asset_grid
            .thumbnail_bounds(start.id(), &*imp.preview_flight);
        let thumbnail = imp.asset_grid.thumbnails().remembered(start.id());
        self.lend_details_to_preview(true);
        imp.window_stack.set_visible_child_name(PREVIEW_PAGE);
        self.refresh_import_availability();
        imp.asset_preview.set_actionable(!self.is_showing_trash());
        imp.asset_preview
            .open(items, position, imp.asset_grid.thumbnails());
        if let (Some(start), Some(texture)) = (flight_start, thumbnail) {
            self.fly_into_preview(start, texture);
        }
    }

    fn fly_into_preview(&self, start: graphene::Rect, texture: gdk::Texture) {
        let imp = self.imp();
        let preview = imp.asset_preview.get();
        let flight = imp.preview_flight.get();
        preview.hide_image(true);
        let landing = glib::clone!(
            #[weak]
            preview,
            #[weak]
            flight,
            #[upgrade_or]
            None,
            move || preview.image_bounds(&flight)
        );
        imp.preview_flight.fly(
            Flight {
                texture,
                from: Box::new(move || Some(start)),
                to: Box::new(landing),
                milliseconds: OPENING_FLIGHT_MILLISECONDS,
            },
            glib::clone!(
                #[weak]
                preview,
                move || preview.hide_image(false)
            ),
        );
    }

    fn fly_back_to_grid(&self, last: &PigouneAssetObject) {
        let imp = self.imp();
        let preview = imp.asset_preview.get();
        let flight = imp.preview_flight.get();
        let (Some(start), Some(texture)) = (preview.image_bounds(&flight), preview.shown_texture())
        else {
            return;
        };
        preview.hide_image(true);
        let grid = imp.asset_grid.get();
        let id = last.id();
        let landing = glib::clone!(
            #[weak]
            grid,
            #[weak]
            flight,
            #[upgrade_or]
            None,
            move || grid.thumbnail_bounds(id, &flight)
        );
        imp.preview_flight.fly(
            Flight {
                texture,
                from: Box::new(move || Some(start)),
                to: Box::new(landing),
                milliseconds: CLOSING_FLIGHT_MILLISECONDS,
            },
            glib::clone!(
                #[weak]
                preview,
                move || preview.hide_image(false)
            ),
        );
    }

    fn leave_preview(&self, last: Option<&PigouneAssetObject>) {
        let imp = self.imp();
        if imp.window_stack.visible_child_name().as_deref() != Some(PREVIEW_PAGE) {
            return;
        }
        if let Some(last) = last {
            self.fly_back_to_grid(last);
        }
        imp.window_stack.set_visible_child_name(MAIN_PAGE);
        self.refresh_import_availability();
        self.lend_details_to_preview(false);
        match last {
            Some(last) if imp.browsing_selection.get() => imp.asset_grid.reveal_asset(last.id()),
            Some(last) => imp.asset_grid.select_asset(last.id()),
            None => imp.asset_grid.reveal_selected(),
        }
        self.describe_targets();
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
        let filter = self.current_filter();
        let read = imp.library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((
                library.visible_collections()?,
                library.view_counts_matching(&filter)?,
                library.tags()?,
                library.smart_collections()?,
            ))
        });
        let (collections, counts, mut tags, mut smart_collections) = match read {
            Some(Ok(read)) => read,
            Some(Err(error)) => {
                self.show_library_error(&error);
                return;
            }
            None => return,
        };
        if !self.settings().boolean(settings::SHOW_TAGS) {
            tags.clear();
        }
        let show_smart_collections = self.settings().boolean(settings::SHOW_SMART_COLLECTIONS);
        if !show_smart_collections {
            smart_collections.clear();
        }
        self.smart_collection_order()
            .sort(&mut smart_collections, |name: &str| {
                glib::FilenameCollationKey::from(name)
            });
        let still_exists = displayed_view::still_exists(
            imp.current_view.get(),
            &collections
                .iter()
                .map(|collection| collection.id)
                .collect::<Vec<_>>(),
            &tags.iter().map(|tag| tag.id).collect::<Vec<_>>(),
            &smart_collections
                .iter()
                .map(|collection| collection.id)
                .collect::<Vec<_>>(),
        );
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
            smart_collections,
            show_smart_collections,
            folded: FoldedSections {
                collections: !self
                    .settings()
                    .boolean(settings::SIDEBAR_COLLECTIONS_EXPANDED),
                smart_collections: !self
                    .settings()
                    .boolean(settings::SIDEBAR_SMART_COLLECTIONS_EXPANDED),
                tags: !self.settings().boolean(settings::SIDEBAR_TAGS_EXPANDED),
            },
        });
        self.refresh_selected_collections();
        if !still_exists {
            self.refresh_grid();
        }
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
                library.view_count(AssetView::Collection(id))?,
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
        let trashed = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.removal_of(id).ok())
            .map_or(0, |removal| removal.trashed_assets);
        let deleted = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Trash { id })
        });
        match deleted {
            Some(Ok(_)) => {
                self.refresh_assets();
                let text = if trashed == 0 {
                    gettext("Collection “{name}” deleted")
                } else {
                    ngettext(
                        "Collection “{name}” deleted, {count} asset moved to the trash",
                        "Collection “{name}” deleted, {count} assets moved to the trash",
                        u32::try_from(trashed).unwrap_or(u32::MAX),
                    )
                    .replace("{count}", &trashed.to_string())
                };
                self.show_undoable_toast(&text.replace("{name}", name));
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

    fn ask_collection_parent(&self, id: CollectionId) {
        let Some(name) = self.collection_name(id) else {
            return;
        };
        let read = self.imp().library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((library.visible_collections()?, library.collection_paths()?))
        });
        let Some(Ok((collections, paths))) = read else {
            return;
        };
        let has_parent = collections
            .iter()
            .any(|collection| collection.id == id && collection.parent.is_some());
        let unavailable = collection_choice::unavailable_destinations(&collections, id);
        let top_level = has_parent.then(|| gettext("Top Level"));
        collection_chooser::present_with_top_level(
            self,
            &gettext("Move the Collection “{name}”").replace("{name}", &name),
            paths,
            unavailable,
            top_level,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |parent| window.move_collection(id, parent)
            ),
        );
    }

    fn move_collection(&self, id: CollectionId, parent: Option<CollectionId>) {
        let name = self.collection_name(id).unwrap_or_default();
        let parent_name = parent.and_then(|parent| self.collection_name(parent));
        let applied = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Move { id, parent })?;
            Ok::<_, pigoune_core::CollectionError>(ancestors(library, parent))
        });
        match applied {
            Some(Ok(reveal)) => {
                self.refresh_sidebar_revealing(reveal);
                self.refresh_grid();
                self.show_collection_moved_toast(&name, parent_name.as_deref());
            }
            Some(Err(error)) => {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Unable to Move the Collection")),
                    Some(&error_messages::describe_collection(&error)),
                );
                alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
                alert.present(Some(self));
            }
            None => {}
        }
    }

    fn show_collection_moved_toast(&self, name: &str, parent_name: Option<&str>) {
        let text = match parent_name {
            Some(parent_name) => gettext("The collection “{name}” was moved into “{parent}”.")
                .replace("{name}", name)
                .replace("{parent}", parent_name),
            None => gettext("The collection “{name}” was moved to the top level.")
                .replace("{name}", name),
        };
        self.show_undoable_toast(&text);
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

    fn ask_collection_look(&self, id: CollectionId) {
        let collection = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.collection(id).ok().flatten());
        let Some(collection) = collection else {
            return;
        };
        let dialog = PigouneCollectionLookDialog::new(
            &collection.name,
            &collection.look,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                Ok(()),
                move |look| window.restyle_collection(id, look)
            ),
        );
        dialog.present(Some(self));
    }

    fn restyle_collection(&self, id: CollectionId, look: &CollectionLook) -> Result<(), String> {
        let restyled = self.change_library(|library| {
            library.apply_collection_command(&CollectionCommand::Restyle {
                id,
                look: look.clone(),
            })
        });
        match restyled {
            Some(Ok(_)) => {
                self.refresh_sidebar();
                Ok(())
            }
            Some(Err(error)) => Err(error_messages::describe_collection(&error)),
            None => Ok(()),
        }
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

    fn search_place(&self, view: AssetView) -> String {
        match view {
            AssetView::All => gettext("the whole library"),
            AssetView::Trash => gettext("the trash"),
            AssetView::Favorites
            | AssetView::Unclassified
            | AssetView::Collection(_)
            | AssetView::Tag(_)
            | AssetView::Smart(_) => gettext("“{name}”").replace("{name}", &self.scope_name(view)),
        }
    }

    fn view_has_sub_tags(&self, view: AssetView) -> bool {
        let AssetView::Tag(id) = view else {
            return false;
        };
        self.imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.sub_tags(id).ok())
            .is_some_and(|children| !children.is_empty())
    }

    fn announce_search_place(&self) {
        let imp = self.imp();
        let current = imp.current_view.get();
        let everywhere = displayed_view::everywhere(
            self.settings().boolean(settings::SEARCH_EVERYWHERE),
            imp.view_chosen_by_click.get(),
        );
        let name = self.scope_name(current);
        let choosable = current != AssetView::All && displayed_view::can_widen(current);
        self.show_search_scope_choice(everywhere);
        let scope = if choosable {
            let detail = (!everywhere && self.view_has_sub_tags(current))
                .then(|| gettext("“{name}”, with its sub-tags").replace("{name}", &name));
            SearchScope {
                label: if everywhere {
                    gettext("Everywhere")
                } else {
                    name.clone()
                },
                wide: everywhere,
                menu_name: Some(name),
                detail,
            }
        } else if current == AssetView::All {
            SearchScope {
                label: gettext("Everywhere"),
                wide: true,
                menu_name: None,
                detail: None,
            }
        } else {
            SearchScope {
                label: if current == AssetView::Trash {
                    gettext("Trash")
                } else {
                    name
                },
                wide: false,
                menu_name: None,
                detail: None,
            }
        };
        imp.grid_header.show_search_scope(&scope);
        imp.grid_header
            .search_entry()
            .set_placeholder_text(Some(&gettext("Search…")));
    }

    fn show_search_scope_choice(&self, everywhere: bool) {
        if let Some(action) = self
            .lookup_action("search-scope")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&(if everywhere { "everywhere" } else { "here" }).to_variant());
        }
    }

    fn searched_view(&self, view: AssetView, filter: &AssetFilter) -> AssetView {
        let everywhere = displayed_view::everywhere(
            self.settings().boolean(settings::SEARCH_EVERYWHERE),
            self.imp().view_chosen_by_click.get(),
        );
        displayed_view::shown(view, everywhere, filter.narrows())
    }

    fn dragged_from_view(&self) -> AssetView {
        displayed_view::drop_source(self.imp().current_view.get(), self.displayed_view())
    }

    fn displayed_view(&self) -> AssetView {
        self.searched_view(self.imp().current_view.get(), &self.current_filter())
    }

    fn refresh_grid(&self) {
        self.refresh_import_availability();
        let imp = self.imp();
        let filter = self.current_filter();
        let view = self.searched_view(imp.current_view.get(), &filter);
        let previously_selected = self.selected_ids();
        let read = imp.library.borrow().as_ref().map(|library| {
            Ok::<_, LibraryError>((
                asset_objects(library, view, &filter)?,
                library.view_count(AssetView::All)?,
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
                let place = self.search_place(view);
                imp.query_pills
                    .show_result_count(searching.then_some(assets.len()), &place);
                self.announce_search_place();
                imp.asset_grid.show_assets(&assets);
                if !previously_selected.is_empty() {
                    imp.asset_grid.select_assets(&previously_selected);
                }
                imp.library_stack.set_visible_child_name(page);
                imp.grid_header.details_button().set_visible(true);
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
        let selected = self.targeted_assets();
        if selected.is_empty() {
            return;
        }
        if self.is_showing_trash() {
            self.show_toast(&gettext(
                "Restore these assets to add them to the favorites.",
            ));
            return;
        }
        let favorite = !selected.iter().all(PigouneAssetObject::favorite);
        let changed: Vec<AssetId> = selected
            .iter()
            .filter(|asset| asset.favorite() != favorite)
            .map(PigouneAssetObject::id)
            .collect();
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
                self.schedule_counts_refresh();
                let showing_favorites = self.displayed_view() == AssetView::Favorites
                    || self.current_filter().favorites_only;
                if showing_favorites && !favorite {
                    let leaving: Vec<AssetId> =
                        selected.iter().map(PigouneAssetObject::id).collect();
                    imp.asset_grid.remove_assets(&leaving);
                    if imp.asset_grid.is_empty() {
                        self.refresh_grid();
                    }
                } else if selected.len() > 1 {
                    imp.asset_details
                        .show_group(&selected, &imp.asset_grid.thumbnails());
                }
                if !changed.is_empty() {
                    self.show_favorite_toast(favorite, &changed);
                }
            }
            Some(Err(error)) => self.show_favorite_error(&error),
            None => {}
        }
    }

    fn describe_missing_results(&self, view: AssetView, view_name: &str, filter: &AssetFilter) {
        let scope = match view {
            AssetView::All => None,
            AssetView::Favorites => Some(gettext("Favorites")),
            AssetView::Unclassified => Some(gettext("Unclassified")),
            AssetView::Trash => Some(gettext("Trash")),
            AssetView::Collection(_) | AssetView::Tag(_) | AssetView::Smart(_) => {
                Some(view_name.to_owned())
            }
        };
        let description = match scope {
            _ if filter.chosen_filters() > 0 => {
                gettext("No asset matches the chosen filters. Clear them to see more assets.")
            }
            Some(scope) => gettext(
                "No asset in “{name}” matches this search. Select “All” to search everywhere.",
            )
            .replace("{name}", &scope),
            None => gettext("No asset in the library matches this search."),
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
                "Assets moved to the trash can be restored from here.",
            )));
        } else if let AssetView::Tag(_) = view {
            if self.view_has_sub_tags(view) {
                page.set_title(
                    &gettext("Nothing Tagged “{name}” or Its Sub-tags")
                        .replace("{name}", view_name),
                );
                page.set_description(Some(&gettext(
                    "Add this tag or one of its sub-tags to assets from the details panel, or drag assets onto it.",
                )));
            } else {
                page.set_title(&gettext("No Asset Tagged “{name}”").replace("{name}", view_name));
                page.set_description(Some(&gettext(
                    "Add this tag to assets from the details panel, or drag assets onto it.",
                )));
            }
        } else if let AssetView::Smart(_) = view {
            page.set_icon_name(Some("media-playlist-shuffle-symbolic"));
            page.set_title(&gettext("No Asset in “{name}”").replace("{name}", view_name));
            page.set_description(Some(&gettext(
                "No asset matches this smart collection yet. Matching assets appear here by themselves.",
            )));
        } else if view == AssetView::Favorites {
            page.set_title(&gettext("No Favorites"));
            page.set_description(Some(
                &gettext(
                    "Add an asset to Favorites with the star in the details panel or with {shortcut}.",
                )
                .replace("{shortcut}", &shortcuts::current_label(TOGGLE_FAVORITE_ACTION)),
            ));
        } else if view == AssetView::Unclassified {
            page.set_title(&gettext("No Unclassified Assets"));
            page.set_description(Some(&gettext("Every asset is in at least one collection.")));
        } else {
            page.set_title(&gettext("“{name}” Is Empty").replace("{name}", view_name));
            page.set_description(Some(&gettext(
                "Assets imported while this collection is selected are placed in it.",
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
        self.imp().library_actions_enabled.set(enabled);
        self.refresh_import_availability();
    }

    fn import_is_possible_here(&self) -> bool {
        drop_places::accepts_files(self.imp().current_view.get())
            && !self.is_previewing()
            && !self.is_editing()
    }

    fn refresh_import_availability(&self) {
        let imp = self.imp();
        let available = imp.library_actions_enabled.get() && self.import_is_possible_here();
        for action in IMPORT_ACTIONS {
            self.action_set_enabled(action, available);
        }
        imp.import_button.set_sensitive(available);
        imp.import_button
            .set_tooltip_text(Some(&if self.import_is_possible_here() {
                gettext("Import")
            } else {
                self.why_import_is_refused()
            }));
    }

    fn why_import_is_refused(&self) -> String {
        if self.is_previewing() {
            gettext("Close the preview to import")
        } else if self.is_editing() {
            gettext("Close the editor to import")
        } else {
            gettext("Open All, Unclassified or a collection to import")
        }
    }

    fn explain_refused_import_shortcuts(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                let pressed = shortcuts::from_key_press(key, modifiers);
                let is_shortcut_of = |action: &str| {
                    pressed.is_some()
                        && shortcuts::current(action)
                            .and_then(|accelerator| shortcuts::canonical(&accelerator))
                            == pressed
                };
                let imports = is_shortcut_of(IMPORT_FILES_ACTION)
                    || is_shortcut_of(IMPORT_FOLDER_ACTION)
                    || (is_shortcut_of(PASTE_ACTION) && !window.text_has_focus());
                let refused =
                    window.imp().library.borrow().is_some() && !window.import_is_possible_here();
                if imports && refused {
                    window.show_toast(&window.why_import_is_refused());
                }
                glib::Propagation::Proceed
            }
        ));
        self.add_controller(keys);
    }

    fn text_has_focus(&self) -> bool {
        GtkWindowExt::focus(self)
            .is_some_and(|focus| focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
    }

    fn set_importing(&self, importing: bool) {
        for action in [CREATE_LIBRARY_ACTION, OPEN_LIBRARY_ACTION] {
            self.action_set_enabled(action, !importing);
        }
        self.set_library_actions_enabled(!importing && !self.is_editing());
    }

    fn accept_dropped_files(&self) {
        let drop_target =
            gtk::DropTargetAsync::new(Some(dropped_content::formats()), gdk::DragAction::COPY);
        drop_target.connect_accept(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            false,
            move |_, drop| {
                window.imp().library.borrow().is_some()
                    && window.import_is_possible_here()
                    && drop.drag().is_none()
                    && dropped_content::may_hold_an_image(&drop.formats())
            }
        ));
        drop_target.connect_drag_enter(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, _, _| {
                window.show_drop_hint();
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_drag_leave(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| window.imp().drop_hint.set_visible(false)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            false,
            move |_, drop, _, _| {
                window.imp().drop_hint.set_visible(false);
                let drop = drop.clone();
                glib::spawn_future_local(async move { window.import_dropped(&drop).await });
                true
            }
        ));
        self.imp().drop_area.add_controller(drop_target);
    }

    async fn import_dropped(&self, drop: &gdk::Drop) {
        let dropped = dropped_content::read(drop).await;
        drop.finish(gdk::DragAction::COPY);
        self.import_content(dropped, self.target_collection()).await;
    }

    async fn import_content(&self, dropped: Dropped, collection: Option<CollectionId>) {
        match dropped {
            Dropped::Files(paths) => self.import_paths_into(paths, collection).await,
            Dropped::Image(DroppedImage {
                bytes,
                name,
                extension,
            }) => {
                let name = name.unwrap_or_else(pasted_content::image_name);
                self.import_image_bytes(&name, &extension, &bytes, collection)
                    .await;
            }
            Dropped::Nothing => self.show_toast(&nothing_to_import_text()),
        }
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

    async fn import_paths_into(&self, paths: Vec<PathBuf>, collection: Option<CollectionId>) {
        if paths.is_empty() {
            return;
        }
        let destination_name = collection.and_then(|id| {
            self.imp()
                .library
                .borrow()
                .as_ref()
                .and_then(|library| library.collection(id).ok().flatten())
                .map(|collection| collection.name)
        });
        let Some(library) = self.imp().library.take() else {
            return;
        };

        self.set_importing(true);
        let finished = background_import::run(self, library, paths.clone(), collection).await;
        self.set_importing(false);

        if let Some(FinishedImport { library, result }) = finished {
            self.imp().library.replace(Some(library));
            self.refresh_assets();
            self.analyse_colors();
            self.report_import(result, &paths, destination_name.as_deref());
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
                toasts::announce(
                    &self.imp().toast_overlay,
                    &adw::Toast::new(&import_report::toast_text(&summary, destination)),
                );
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

fn descendant_button(widget: &gtk::Widget) -> Option<gtk::Button> {
    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        return Some(button);
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(button) = descendant_button(&current) {
            return Some(button);
        }
        child = current.next_sibling();
    }
    None
}

fn tags_added_text(single_name: Option<&str>, count: usize) -> String {
    let plural = u32::try_from(count).unwrap_or(u32::MAX);
    match single_name {
        Some(name) => ngettext(
            "Tag “{name}” added to {count} asset",
            "Tag “{name}” added to {count} assets",
            plural,
        )
        .replace("{count}", &count.to_string())
        .replace("{name}", name),
        None => ngettext(
            "Tags added to {count} asset",
            "Tags added to {count} assets",
            plural,
        )
        .replace("{count}", &count.to_string()),
    }
}

fn recent_removal_text(count: usize) -> String {
    ngettext(
        "{count} library removed from the recent libraries",
        "{count} libraries removed from the recent libraries",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
}

fn short_folder(folder: &Path) -> String {
    let home = glib::home_dir();
    match folder.strip_prefix(&home) {
        Ok(inside) if inside.as_os_str().is_empty() => "~".to_owned(),
        Ok(inside) => format!("~/{}", inside.to_string_lossy()),
        Err(_) => folder.to_string_lossy().into_owned(),
    }
}

fn welcome_recent_row(label: &str, root: &Path) -> adw::ActionRow {
    let found = root.is_dir();
    let location = host_path::shown_path(root)
        .parent()
        .map(short_folder)
        .unwrap_or_default();
    let subtitle = if found {
        location
    } else {
        gettext("Not found in {folder}").replace("{folder}", &location)
    };
    let row = adw::ActionRow::builder()
        .title(label)
        .subtitle(subtitle)
        .activatable(true)
        .build();
    if !found {
        row.add_css_class("dim-label");
    }
    row.add_prefix(&gtk::Image::from_icon_name("folder-pictures-symbolic"));
    row
}

fn wallpaper_folder() -> Result<PathBuf, String> {
    let folder = glib::user_cache_dir().join("pigoune").join("wallpapers");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

fn output_natural_size(asset: &PigouneAssetObject) -> (u32, u32) {
    asset
        .annotation()
        .and_then(|render| annotated_files::cropped_size(&render.document))
        .unwrap_or_else(|| natural_size(asset))
}

fn natural_size(asset: &PigouneAssetObject) -> (u32, u32) {
    asset.asset().dimensions.map_or(
        (DEFAULT_VECTOR_EXPORT_PIXELS, DEFAULT_VECTOR_EXPORT_PIXELS),
        |size| (size.width(), size.height()),
    )
}

fn folder_name(folder: &gio::File) -> String {
    folder
        .basename()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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

fn smart_collection_parameter(parameter: Option<&glib::Variant>) -> Option<SmartCollectionId> {
    SmartCollectionId::parse(&parameter?.get::<String>()?)
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
                "{count} asset found only here will go to the trash.",
                "{count} assets found only here will go to the trash.",
                count(removal.trashed_assets),
            )
            .replace("{count}", &removal.trashed_assets.to_string()),
        );
    }
    if contained > removal.trashed_assets {
        sentences.push(gettext("Assets also kept in other collections stay there."));
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

fn shortcut_tooltip(label: &str, action: &str) -> String {
    let shortcut = shortcuts::current_label(action);
    if shortcut.is_empty() {
        label.to_owned()
    } else {
        format!("{label} ({shortcut})")
    }
}

fn menu_item(label: &str, action: &str, accel: Option<&str>) -> gio::MenuItem {
    match accel {
        Some(accel) => {
            let item = gio::MenuItem::new(Some(label), Some(action));
            item.set_attribute_value("accel", Some(&accel.to_variant()));
            item
        }
        None => shortcuts::menu_item(label, action),
    }
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
            .tag_path(id)
            .ok()
            .map(|path| tag_tree::path_text(&path, id))
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| library.name()),
        AssetView::Smart(id) => library
            .smart_collection(id)
            .ok()
            .flatten()
            .map_or_else(|| library.name(), |collection| collection.name),
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
    let mut renders = library.annotation_renders()?;
    Ok(library
        .find_assets_in(view, filter)?
        .iter()
        .map(|asset| {
            PigouneAssetObject::new(AssetEntry {
                file: library.file_of(asset),
                thumbnail_file: library.thumbnail_file(asset.id, THUMBNAIL_PIXELS),
                annotation: renders.remove(&asset.id),
                asset: asset.clone(),
            })
        })
        .collect())
}

fn nothing_to_import_text() -> String {
    gettext("This drop holds no image. In the browser, use “Copy Image”, then press {shortcut}")
        .replace("{shortcut}", &shortcuts::current_label(PASTE_ACTION))
}

async fn analyse_drawing(file: &Path, thumbnail_file: &Path) -> Option<Option<VisualAnalysis>> {
    let texture = thumbnails::thumbnail(file, thumbnail_file).await?;
    let size = u32::try_from(texture.width())
        .ok()
        .zip(u32::try_from(texture.height()).ok());
    let pixels = asset_colors::rgba_pixels(&texture);
    Some(
        gio::spawn_blocking(move || visual_analysis_of(size, pixels))
            .await
            .unwrap_or_default(),
    )
}

fn visual_analysis_of(size: Option<(u32, u32)>, pixels: Vec<u8>) -> Option<VisualAnalysis> {
    let (width, height) = size?;
    visual_analysis(&RgbaImage::new(width, height, pixels)?)
}

fn ignoring_failure_title() -> String {
    gettext("Unable to Ignore")
}

fn showing_again_failure_title() -> String {
    gettext("Unable to Show Again")
}
