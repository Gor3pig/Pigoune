use std::fs;
use std::path::Path;

use super::content::{self, ContentDigest};
use super::trash::forget_thumbnails;
use super::{AssetId, Library, LibraryError, ReplaceError, layout};

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
        fs::create_dir_all(folder)?;
        let temporary = folder.join(layout::unfinished_replacement_name(id));
        let _ = fs::remove_file(&temporary);
        let copied = content::copy_with_digest(source, &temporary);
        let result = match copied {
            Ok(copied) if copied == wanted => {
                fs::rename(&temporary, &destination).map_err(Into::into)
            }
            Ok(_) => Err(ReplaceError::DifferentContent(source.to_path_buf())),
            Err(error) => Err(error.into()),
        };
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return result;
        }
        forget_thumbnails(&self.root, id);
        Ok(())
    }
}
