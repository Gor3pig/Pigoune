use std::path::PathBuf;

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
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}
