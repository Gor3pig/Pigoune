use std::fs;
use std::path::Path;

use super::content::{self, ContentDigest};
use super::trash::forget_thumbnails;
use super::{AssetId, Library, LibraryError, ReplaceError, layout};
use super::{cache_guard, storage_guard};

impl Library {
    pub fn replace_stored_file(&self, id: AssetId, source: &Path) -> Result<(), ReplaceError> {
        let asset = self.asset(id)?.ok_or(ReplaceError::AssetNotFound(id))?;
        let wanted = ContentDigest {
            hash: asset.content_hash,
            byte_size: asset.byte_size,
        };
        if content::digest(source)? != wanted {
            return Err(ReplaceError::DifferentContent(source.to_path_buf()));
        }
        if !layout::is_stored_path_of(id, &asset.stored_path) {
            return Err(ReplaceError::OutsideLibrary(id));
        }
        let destination = self.root.join(&asset.stored_path);
        let folder = destination.parent().ok_or_else(|| {
            LibraryError::Io(std::io::Error::other("a stored file always has a folder"))
        })?;
        storage_guard::prepare_asset_folder(&self.root, id)?;
        let temporary = folder.join(layout::unfinished_replacement_name(id));
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) if metadata.is_file() => {
                cache_guard::remove_plain_file(&temporary);
            }
            Ok(_) => return Err(LibraryError::UnsafeStorage(temporary).into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(LibraryError::from(error).into()),
        }
        let copied = content::copy_with_digest(source, &temporary);
        let result = match copied {
            Ok(copied) if copied == wanted => publish(&temporary, &destination),
            Ok(_) => Err(ReplaceError::DifferentContent(source.to_path_buf())),
            Err(error) => Err(error.into()),
        };
        if result.is_err() {
            cache_guard::remove_plain_file(&temporary);
            return result;
        }
        forget_thumbnails(&self.root, id);
        Ok(())
    }
}

fn publish(temporary: &Path, destination: &Path) -> Result<(), ReplaceError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_file() => {
            Err(LibraryError::UnsafeStorage(destination.to_path_buf()).into())
        }
        Ok(_) => fs::rename(temporary, destination).map_err(Into::into),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::rename(temporary, destination).map_err(Into::into)
        }
        Err(error) => Err(LibraryError::from(error).into()),
    }
}
