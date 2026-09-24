use std::{error::Error, ffi::OsString, fmt};

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalFilename(Vec<u8>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalFilenameError {
    Empty,
    ContainsSeparator,
    ContainsNul,
}

impl fmt::Display for OriginalFilenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("original filename is empty"),
            Self::ContainsSeparator => f.write_str("original filename contains a path separator"),
            Self::ContainsNul => f.write_str("original filename contains NUL"),
        }
    }
}

impl Error for OriginalFilenameError {}

impl OriginalFilename {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, OriginalFilenameError> {
        if bytes.is_empty() {
            return Err(OriginalFilenameError::Empty);
        }
        if bytes.contains(&b'/') {
            return Err(OriginalFilenameError::ContainsSeparator);
        }
        if bytes.contains(&0) {
            return Err(OriginalFilenameError::ContainsNul);
        }
        Ok(Self(bytes))
    }

    #[cfg(unix)]
    pub fn from_os_str(value: &std::ffi::OsStr) -> Result<Self, OriginalFilenameError> {
        Self::from_bytes(value.as_bytes().to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[cfg(unix)]
    pub fn to_os_string(&self) -> OsString {
        OsString::from_vec(self.0.clone())
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }
}
