use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use super::{ImportError, LibraryError};

const CHUNK_LENGTH: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDigest {
    pub hash: String,
    pub byte_size: u64,
}

pub fn digest(source: &Path) -> Result<ContentDigest, ImportError> {
    stream(source, |_| Ok(()))
}

pub fn copy_with_digest(source: &Path, destination: &Path) -> Result<ContentDigest, ImportError> {
    let mut copy = File::create_new(destination).map_err(LibraryError::from)?;
    let digest = stream(source, |chunk| copy.write_all(chunk))?;
    copy.sync_all().map_err(LibraryError::from)?;
    Ok(digest)
}

fn stream(
    source: &Path,
    mut consume: impl FnMut(&[u8]) -> io::Result<()>,
) -> Result<ContentDigest, ImportError> {
    let unreadable = |_| ImportError::Unreadable(source.to_path_buf());
    let mut reader = File::open(source).map_err(unreadable)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0; CHUNK_LENGTH];
    let mut byte_size = 0_u64;

    loop {
        let length = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(unreadable(error)),
        };
        let chunk = &buffer[..length];
        hasher.update(chunk);
        consume(chunk).map_err(LibraryError::from)?;
        byte_size += chunk.len() as u64;
    }

    Ok(ContentDigest {
        hash: hasher.finalize().to_hex().to_string(),
        byte_size,
    })
}
