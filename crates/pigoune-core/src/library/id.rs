use std::{error::Error, fmt, str::FromStr};
use uuid::{Uuid, Version};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LibraryId(Uuid);

#[derive(Debug)]
pub enum ParseLibraryIdError {
    InvalidUuid(uuid::Error),
    NotVersionFour,
}

impl fmt::Display for ParseLibraryIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUuid(error) => write!(f, "invalid library UUID: {error}"),
            Self::NotVersionFour => f.write_str("library UUID must be version 4"),
        }
    }
}

impl Error for ParseLibraryIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidUuid(error) => Some(error),
            Self::NotVersionFour => None,
        }
    }
}

impl LibraryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, ParseLibraryIdError> {
        let uuid = Uuid::from_bytes(bytes);
        if uuid.get_version() != Some(Version::Random) {
            return Err(ParseLibraryIdError::NotVersionFour);
        }
        Ok(Self(uuid))
    }

    pub fn to_bytes(self) -> [u8; 16] {
        *self.0.as_bytes()
    }
}

impl Default for LibraryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LibraryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for LibraryId {
    type Err = ParseLibraryIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(value).map_err(ParseLibraryIdError::InvalidUuid)?;
        Self::from_bytes(*uuid.as_bytes())
    }
}
