mod database;
mod filename;
mod id;
mod manifest;
mod path;

pub use database::{AssetRecord, DatabaseError, LibraryDatabase};
pub use filename::{OriginalFilename, OriginalFilenameError};
pub use id::{LibraryId, ParseLibraryIdError};
pub use manifest::{LibraryManifest, ManifestError};

use crate::{ObjectStore, StoreError};
use std::{error::Error, fmt, fs, io, path::Path};

pub const LIBRARY_FORMAT_VERSION: u32 = 1;
pub const DATABASE_SCHEMA_VERSION: i32 = 1;

pub struct Library {
    pub id: LibraryId,
    pub database: LibraryDatabase,
    pub object_store: ObjectStore,
}

#[derive(Debug)]
pub enum LibraryError {
    Io(io::Error),
    NotEmpty,
    Manifest(ManifestError),
    Database(DatabaseError),
    Store(StoreError),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "library I/O error: {error}"),
            Self::NotEmpty => f.write_str("new library directory is not empty"),
            Self::Manifest(error) => error.fmt(f),
            Self::Database(error) => error.fmt(f),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl Error for LibraryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::NotEmpty => None,
        }
    }
}

impl Library {
    /// Create the v1 layout in an existing, empty directory.
    pub fn create(root: &Path) -> Result<Self, LibraryError> {
        let mut entries = fs::read_dir(root).map_err(LibraryError::Io)?;
        if entries
            .next()
            .transpose()
            .map_err(LibraryError::Io)?
            .is_some()
        {
            return Err(LibraryError::NotEmpty);
        }
        let id = LibraryId::new();
        fs::create_dir(root.join("recovery")).map_err(LibraryError::Io)?;
        let object_store = ObjectStore::new(root).map_err(LibraryError::Store)?;
        let database = LibraryDatabase::create(root, id).map_err(LibraryError::Database)?;
        LibraryManifest::new(id)
            .write_new(root)
            .map_err(LibraryError::Manifest)?;
        Ok(Self {
            id,
            database,
            object_store,
        })
    }

    pub fn open(root: &Path) -> Result<Self, LibraryError> {
        let manifest = LibraryManifest::read(root).map_err(LibraryError::Manifest)?;
        let database =
            LibraryDatabase::open(root, manifest.library_id).map_err(LibraryError::Database)?;
        let object_store = ObjectStore::new(root).map_err(LibraryError::Store)?;
        Ok(Self {
            id: manifest.library_id,
            database,
            object_store,
        })
    }
}
