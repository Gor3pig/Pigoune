mod adopt;
mod animation_recheck;
mod annotated;
mod asset;
mod asset_command;
mod batch_import;
mod buckets;
mod cache_guard;
mod clock;
mod collection;
mod collection_command;
mod content;
mod details_store;
mod dimension_recheck;
mod duplicate_removal;
mod duplicates;
mod error;
mod export;
mod export_lifetime;
mod health;
mod history;
mod id;
mod ignored_duplicates;
mod import;
mod import_batch;
mod import_plan;
mod layout;
mod library_export;
mod live_transfer;
mod overview;
mod query_cache;
mod records;
mod remove_record;
mod replace;
mod schema;
mod search;
mod smart_collection;
mod smart_collection_command;
mod staging;
mod storage;
mod tag;
mod tag_command;
mod thumbnail_cache;
mod trash;
mod view;

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;

use crate::media::Dimensions;
use history::History;
use staging::StagingDir;

pub use annotated::{AnnotationRender, Annotations, RENDER_VERSION};
pub use asset::Asset;
pub use asset_command::{AssetCommand, AssetError, TextField};
pub use batch_import::{
    ImportControl, ImportEnding, ImportProgress, ImportSummary, LARGE_FILE_BYTES,
};
pub use cache_guard::create_cache_folder;
pub use collection::{Collection, CollectionLook, CollectionPath};
pub use collection_command::{CollectionCommand, CollectionRemoval};
pub use duplicate_removal::{ConfirmedRemoval, MOST_CHOSEN_AT_ONCE, RemovalError};
pub use duplicates::{
    ALMOST_IDENTICAL_DISTANCE, DUPLICATE_DISTANCE, DuplicateGroup, DuplicateKind, DuplicatePlan,
    DuplicateProgress, GroupMember, IDENTICAL_DETAIL_GAP, MOST_IN_A_GROUP, PROBABLE_DETAIL_GAP,
    SIMILAR_DISTANCE, SimilarAsset, SimilarSearch, VERY_CLOSE_DISTANCE,
};
pub use error::{
    AdoptError, AnnotatedError, CollectionError, ImportError, LibraryError, RemoveRecordError,
    ReplaceError, cache_entry_label,
};
pub use export::{CopyFailure, CopyReport, FlatFiles};
pub use export_lifetime::{SpaceProbe, VolumeSpace};
pub use health::{HealthIssue, HealthPlan, HealthProgress, HealthReport, HealthTarget};
pub use history::{Change, ChangeStamp, HISTORY_LIMIT, UndoError};
pub use id::{AssetId, CollectionId, IgnoreId, SmartCollectionId, TagId};
pub use ignored_duplicates::{
    DuplicateCommand, DuplicateError, IgnoreAction, IgnoreKind, IgnoreOutcome, IgnoredEntry,
};
pub use import::ImportOutcome;
pub use layout::{
    CACHE_DIR_NAME, DATABASE_FILE_NAME, FILES_DIR_NAME, LIBRARY_EXTENSION, library_display_name,
};
pub use library_export::{
    ExportFailure, ExportOutline, LibraryExportPlan, LibraryExportReport, OutlineFolder,
};
pub use live_transfer::ClipboardInstall;
pub use overview::LibraryOverview;
pub use records::LibraryRecords;
#[cfg(feature = "migration-stops")]
pub use schema::MigrationStop;
pub use schema::{CURRENT_FORMAT_VERSION, oldest_compatible_version};
pub use search::{
    AssetFilter, MAX_QUERY_WORDS, comparable, query_groups, query_text, query_word_count,
};
pub use smart_collection::SmartCollection;
pub use smart_collection_command::{SmartCollectionCommand, SmartCollectionError};
pub use storage::{ColorShare, FormatShare, StorageUse};
pub use tag::{Tag, same_tag_name};
pub use tag_command::{LONGEST_TAG_NAME, TagCommand, TagError};
pub use thumbnail_cache::thumbnail_cache_bytes_in;
pub use trash::TRASH_RETENTION;
pub use view::{AssetView, ViewCounts};

#[derive(Debug)]
pub struct Library {
    root: PathBuf,
    connection: Connection,
    history: History,
    screen: Option<Dimensions>,
    query_cache: query_cache::SharedQueryCache,
    _exclusive_access: Option<File>,
}

impl Library {
    pub fn create(parent_dir: &Path, requested_name: &str) -> Result<Self, LibraryError> {
        let folder_name = layout::library_folder_name(requested_name)?;

        if !parent_dir.is_dir() {
            return Err(LibraryError::NotFound(parent_dir.to_path_buf()));
        }

        let root = parent_dir.join(&folder_name);
        if root.symlink_metadata().is_ok() {
            return Err(LibraryError::AlreadyExists(root));
        }

        let staging = StagingDir::create(
            parent_dir.join(format!(".{folder_name}.{}.partial", Uuid::now_v7())),
        )?;
        build_new_library(staging.path())?;
        staging.promote_to(&root)?;

        Self::open(&root)
    }

    pub fn open(root: &Path) -> Result<Self, LibraryError> {
        Self::open_with(root, |connection, root| {
            schema::migrate_to_current_version(connection, root)
        })
    }

    #[cfg(feature = "migration-stops")]
    pub fn open_halting_during_migration(
        root: &Path,
        stop: schema::MigrationStop,
    ) -> Result<Self, LibraryError> {
        Self::open_with(root, |connection, root| {
            schema::migrate_halting_at(connection, root, stop)
        })
    }

    fn open_with(
        root: &Path,
        migrate: impl FnOnce(&mut Connection, &Path) -> Result<(), LibraryError>,
    ) -> Result<Self, LibraryError> {
        if !root.is_dir() {
            return Err(LibraryError::NotFound(root.to_path_buf()));
        }

        let database_path = layout::database_path(root);
        if !database_path.is_file() {
            return Err(LibraryError::NotALibrary(root.to_path_buf()));
        }

        let exclusive_access = acquire_exclusive_access(root)?;

        let mut connection =
            Connection::open_with_flags(&database_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        if !schema::ensure_is_pigoune_database(&connection)? {
            return Err(LibraryError::NotALibrary(root.to_path_buf()));
        }
        schema::ensure_is_writable(&connection)?;
        schema::configure_connection(&connection)?;
        schema::ensure_is_intact(&connection)?;
        let found_version = schema::read_format_version(&connection)?;
        cache_guard::verify_cache(root)?;
        migrate(&mut connection, root)?;

        fs::create_dir_all(root.join(FILES_DIR_NAME))?;
        cache_guard::ensure_cache_folder(root)?;
        details_store::attach(&connection, root)?;
        if found_version < schema::FORMAT_MOVING_VISUAL_DETAILS {
            let _ = connection.execute_batch("VACUUM");
        }
        buckets::move_into_buckets(root);
        remove_unfinished_imports(root);
        export::forget_exports(root);

        let library = Self {
            root: root.to_path_buf(),
            connection,
            history: History::default(),
            screen: None,
            query_cache: query_cache::SharedQueryCache::default(),
            _exclusive_access: exclusive_access,
        };
        library.forget_stale_annotated_renders();
        Ok(library)
    }

    pub fn set_screen(&mut self, screen: Option<Dimensions>) {
        self.screen = screen;
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn name(&self) -> String {
        layout::library_display_name(&self.root)
    }

    pub fn format_version(&self) -> Result<u32, LibraryError> {
        schema::read_format_version(&self.connection)
    }
}

fn build_new_library(staging_dir: &Path) -> Result<(), LibraryError> {
    fs::create_dir(staging_dir.join(FILES_DIR_NAME))?;
    fs::create_dir(staging_dir.join(CACHE_DIR_NAME))?;

    let mut connection = Connection::open(layout::database_path(staging_dir))?;
    schema::configure_connection(&connection)?;
    schema::initialize_new_database(&mut connection, staging_dir)?;
    connection.close().map_err(|(_, error)| error)?;
    Ok(())
}

fn remove_unfinished_imports(root: &Path) {
    let Ok(entries) = fs::read_dir(root.join(FILES_DIR_NAME)) else {
        return;
    };
    for entry in entries.flatten() {
        if layout::is_unfinished_import(&entry.file_name().to_string_lossy()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn acquire_exclusive_access(root: &Path) -> Result<Option<File>, LibraryError> {
    let root_handle = File::open(root)?;
    match root_handle.try_lock() {
        Ok(()) => Ok(Some(root_handle)),
        Err(TryLockError::WouldBlock) => Err(LibraryError::InUse),
        Err(TryLockError::Error(_)) => Ok(None),
    }
}
