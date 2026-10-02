mod asset;
mod asset_command;
mod batch_import;
mod clock;
mod collection;
mod collection_command;
mod content;
mod error;
mod history;
mod id;
mod import;
mod import_plan;
mod layout;
mod schema;
mod search;
mod staging;
mod tag;
mod tag_command;
mod trash;
mod view;

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;

use history::History;
use staging::StagingDir;

pub use asset::Asset;
pub use asset_command::{AssetCommand, AssetError, TextField};
pub use batch_import::{
    ImportControl, ImportEnding, ImportProgress, ImportSummary, LARGE_FILE_BYTES,
};
pub use collection::{Collection, CollectionPath};
pub use collection_command::{CollectionCommand, CollectionRemoval};
pub use error::{CollectionError, ImportError, LibraryError};
pub use history::{Change, ChangeStamp, HISTORY_LIMIT, UndoError};
pub use id::{AssetId, CollectionId, TagId};
pub use import::ImportOutcome;
pub use layout::{
    CACHE_DIR_NAME, DATABASE_FILE_NAME, FILES_DIR_NAME, LIBRARY_EXTENSION, library_display_name,
};
pub use schema::CURRENT_FORMAT_VERSION;
pub use tag::Tag;
pub use tag_command::{TagCommand, TagError};
pub use trash::TRASH_RETENTION;
pub use view::{AssetView, ViewCounts};

#[derive(Debug)]
pub struct Library {
    root: PathBuf,
    connection: Connection,
    history: History,
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
        schema::migrate_to_current_version(&mut connection)?;

        fs::create_dir_all(root.join(FILES_DIR_NAME))?;
        fs::create_dir_all(root.join(CACHE_DIR_NAME))?;
        remove_unfinished_imports(root);

        Ok(Self {
            root: root.to_path_buf(),
            connection,
            history: History::default(),
            _exclusive_access: exclusive_access,
        })
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
    schema::initialize_new_database(&mut connection)?;
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
