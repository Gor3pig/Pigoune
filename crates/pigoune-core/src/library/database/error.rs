use crate::{LibraryId, ObjectHash, OriginalFilenameError, ParseLibraryIdError};
use std::{error::Error, fmt, io, path::PathBuf};

#[derive(Debug)]
pub enum DatabaseError {
    Io(io::Error),
    Sqlite(rusqlite::Error),
    SQLiteTooOld(i32),
    SchemaTooNew(i32),
    UninitializedDatabase,
    InvalidJournalMode(String),
    InvalidMetadata,
    InvalidStoredLibraryId(ParseLibraryIdError),
    LibraryIdMismatch {
        expected: LibraryId,
        found: LibraryId,
    },
    InvalidObjectPath(PathBuf),
    ObjectConflict(ObjectHash),
    SizeOutOfRange(u64),
    InvalidStoredValue(&'static str),
    InvalidOriginalFilename(OriginalFilenameError),
    EmptyDisplayName,
    NegativeImportDate,
    AssetObjectMismatch,
    MissingObject(ObjectHash),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "database I/O error: {error}"),
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
            Self::SQLiteTooOld(version) => write!(f, "SQLite version {version} is below 3.37"),
            Self::SchemaTooNew(version) => {
                write!(f, "database schema version {version} is too new")
            }
            Self::UninitializedDatabase => f.write_str("database has no Pigoune schema"),
            Self::InvalidJournalMode(mode) => {
                write!(f, "database journal mode is {mode}, expected wal")
            }
            Self::InvalidMetadata => f.write_str("invalid or missing library metadata"),
            Self::InvalidStoredLibraryId(error) => write!(f, "invalid stored library ID: {error}"),
            Self::LibraryIdMismatch { expected, found } => {
                write!(f, "library ID mismatch: expected {expected}, found {found}")
            }
            Self::InvalidObjectPath(path) => write!(f, "invalid object path: {}", path.display()),
            Self::ObjectConflict(hash) => write!(f, "conflicting object metadata for {hash}"),
            Self::SizeOutOfRange(size) => {
                write!(f, "object size {size} exceeds SQLite INTEGER range")
            }
            Self::InvalidStoredValue(name) => write!(f, "invalid stored value: {name}"),
            Self::InvalidOriginalFilename(error) => error.fmt(f),
            Self::EmptyDisplayName => f.write_str("asset display name is empty"),
            Self::NegativeImportDate => f.write_str("asset import date is before Unix epoch"),
            Self::AssetObjectMismatch => f.write_str("asset and published object hashes differ"),
            Self::MissingObject(hash) => write!(f, "asset refers to an absent object: {hash}"),
        }
    }
}

impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::InvalidStoredLibraryId(error) => Some(error),
            Self::InvalidOriginalFilename(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
