use super::{LIBRARY_FORMAT_VERSION, LibraryId, ParseLibraryIdError};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs::File, fs::OpenOptions, io, io::Write, path::Path};

const DOCUMENT_TYPE: &str = "pigoune-library";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibraryManifest {
    pub library_id: LibraryId,
}

#[derive(Debug)]
pub enum ManifestError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidType(String),
    UnsupportedFormatVersion(u32),
    InvalidLibraryId(ParseLibraryIdError),
    NonCanonicalLibraryId,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "library manifest I/O error: {error}"),
            Self::Json(error) => write!(f, "invalid library manifest JSON: {error}"),
            Self::InvalidType(value) => write!(f, "unexpected library document type: {value}"),
            Self::UnsupportedFormatVersion(value) => {
                write!(f, "unsupported library format version: {value}")
            }
            Self::InvalidLibraryId(error) => write!(f, "invalid library ID: {error}"),
            Self::NonCanonicalLibraryId => f.write_str("library ID is not canonical"),
        }
    }
}

impl Error for ManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidLibraryId(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(rename = "type")]
    document_type: String,
    library_id: String,
    format_version: u32,
}

impl LibraryManifest {
    pub fn new(library_id: LibraryId) -> Self {
        Self { library_id }
    }

    pub fn read(root: &Path) -> Result<Self, ManifestError> {
        let file = File::open(root.join("library.json")).map_err(ManifestError::Io)?;
        let document: Document = serde_json::from_reader(file).map_err(ManifestError::Json)?;
        if document.document_type != DOCUMENT_TYPE {
            return Err(ManifestError::InvalidType(document.document_type));
        }
        if document.format_version != LIBRARY_FORMAT_VERSION {
            return Err(ManifestError::UnsupportedFormatVersion(
                document.format_version,
            ));
        }
        let library_id = document
            .library_id
            .parse::<LibraryId>()
            .map_err(ManifestError::InvalidLibraryId)?;
        if document.library_id != library_id.to_string() {
            return Err(ManifestError::NonCanonicalLibraryId);
        }
        Ok(Self { library_id })
    }

    pub fn write_new(&self, root: &Path) -> Result<(), ManifestError> {
        let document = Document {
            document_type: DOCUMENT_TYPE.into(),
            library_id: self.library_id.to_string(),
            format_version: LIBRARY_FORMAT_VERSION,
        };
        let path = root.join("library.json");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(ManifestError::Io)?;
        serde_json::to_writer_pretty(&mut file, &document).map_err(ManifestError::Json)?;
        file.write_all(b"\n").map_err(ManifestError::Io)?;
        file.sync_all().map_err(ManifestError::Io)?;
        File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(ManifestError::Io)?;
        Ok(())
    }
}
