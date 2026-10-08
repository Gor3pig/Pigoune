use std::io;
use std::path::PathBuf;

use rusqlite::ErrorCode;

use super::{AssetId, CollectionId};

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("the library name is empty or contains forbidden characters")]
    InvalidName,
    #[error("a file or folder already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("no folder exists at {0}")]
    NotFound(PathBuf),
    #[error("{0} is not a Pigoune library")]
    NotALibrary(PathBuf),
    #[error(
        "the library uses format version {found}, but this version of Pigoune supports up to {supported}"
    )]
    NewerFormat { found: u32, supported: u32 },
    #[error("the library is already open in another instance of Pigoune")]
    InUse,
    #[error("there is not enough free space on the disk")]
    StorageFull,
    #[error("writing to this location is not permitted")]
    PermissionDenied,
    #[error("the library database is damaged")]
    Damaged,
    #[error(transparent)]
    Io(io::Error),
    #[error(transparent)]
    Database(rusqlite::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{0} is not in a supported format")]
    UnsupportedFormat(PathBuf),
    #[error("{0} could not be read")]
    Unreadable(PathBuf),
    #[error("the collection {0} does not exist")]
    CollectionNotFound(CollectionId),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

#[derive(Debug, thiserror::Error)]
pub enum AdoptError {
    #[error("{0} is not stored the way Pigoune stores its files")]
    NotAdoptable(PathBuf),
    #[error("{0} has the same content as the resource {1}")]
    AlreadyKnown(PathBuf, AssetId),
    #[error("{0} is not in a supported format")]
    UnsupportedFormat(PathBuf),
    #[error("{0} could not be read")]
    Unreadable(PathBuf),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<ImportError> for AdoptError {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::UnsupportedFormat(path) => Self::UnsupportedFormat(path),
            ImportError::Unreadable(path) => Self::Unreadable(path),
            ImportError::CollectionNotFound(_) => Self::Library(LibraryError::Io(
                io::Error::other("no collection is involved"),
            )),
            ImportError::Library(error) => Self::Library(error),
        }
    }
}

impl From<rusqlite::Error> for AdoptError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("a collection name cannot be empty")]
    InvalidName,
    #[error("the collection {0} does not exist")]
    NotFound(CollectionId),
    #[error("a collection named {0} already exists here")]
    NameTaken(String),
    #[error("a collection cannot be moved into itself or one of its sub-collections")]
    WouldContainItself,
    #[error("the collections to arrange no longer match the library")]
    OutdatedOrder,
    #[error("the resource {0} does not exist")]
    AssetNotFound(AssetId),
    #[error("the collection {0} is no longer empty")]
    NotEmpty(String),
    #[error("this icon or color cannot be used for a collection")]
    InvalidLook,
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<rusqlite::Error> for CollectionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

impl From<io::Error> for ImportError {
    fn from(error: io::Error) -> Self {
        Self::Library(error.into())
    }
}

impl From<rusqlite::Error> for ImportError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

impl From<io::Error> for LibraryError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => Self::StorageFull,
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                Self::PermissionDenied
            }
            _ => Self::Io(error),
        }
    }
}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(ErrorCode::DiskFull) => Self::StorageFull,
            Some(ErrorCode::ReadOnly | ErrorCode::PermissionDenied) => Self::PermissionDenied,
            Some(ErrorCode::DatabaseCorrupt) => Self::Damaged,
            _ => Self::Database(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use rusqlite::ffi;

    use super::LibraryError;

    fn sqlite_error(code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(ffi::Error::new(code), None)
    }

    #[test]
    fn a_full_disk_is_recognized_from_the_file_system() {
        let error = LibraryError::from(io::Error::from(io::ErrorKind::StorageFull));
        assert!(matches!(error, LibraryError::StorageFull));
    }

    #[test]
    fn an_exceeded_quota_is_reported_as_a_full_disk() {
        let error = LibraryError::from(io::Error::from(io::ErrorKind::QuotaExceeded));
        assert!(matches!(error, LibraryError::StorageFull));
    }

    #[test]
    fn a_full_disk_is_recognized_from_the_database() {
        let error = LibraryError::from(sqlite_error(ffi::SQLITE_FULL));
        assert!(matches!(error, LibraryError::StorageFull));
    }

    #[test]
    fn a_refused_permission_is_recognized_from_the_file_system() {
        let error = LibraryError::from(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(matches!(error, LibraryError::PermissionDenied));
    }

    #[test]
    fn a_read_only_file_system_is_reported_as_a_refused_permission() {
        let error = LibraryError::from(io::Error::from(io::ErrorKind::ReadOnlyFilesystem));
        assert!(matches!(error, LibraryError::PermissionDenied));
    }

    #[test]
    fn a_read_only_database_is_reported_as_a_refused_permission() {
        let error = LibraryError::from(sqlite_error(ffi::SQLITE_READONLY));
        assert!(matches!(error, LibraryError::PermissionDenied));
    }

    #[test]
    fn a_corrupt_database_is_reported_as_damaged() {
        let error = LibraryError::from(sqlite_error(ffi::SQLITE_CORRUPT));
        assert!(matches!(error, LibraryError::Damaged));
    }

    #[test]
    fn other_problems_keep_their_technical_detail() {
        let io_error = LibraryError::from(io::Error::other("cable unplugged"));
        let database_error = LibraryError::from(sqlite_error(ffi::SQLITE_BUSY));
        assert!(matches!(io_error, LibraryError::Io(_)));
        assert!(matches!(database_error, LibraryError::Database(_)));
    }
}
