use std::fs;
use std::path::Path;

use rusqlite::{OptionalExtension, Transaction, params};

use super::asset::EmbeddedSizes;
use super::content::{self, ContentDigest};
use super::staging::StagingDir;
use super::{AssetId, CollectionId, ImportError, Library, clock, collection, layout};
use crate::media::{self, InspectError, MediaInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    Imported(AssetId),
    AlreadyPresent(AssetId),
    AddedToCollection(AssetId),
    RestoredFromTrash(AssetId),
}

struct NewAsset<'a> {
    id: AssetId,
    original_file_name: &'a str,
    media: MediaInfo,
    digest: ContentDigest,
}

impl Library {
    pub fn import_file(
        &mut self,
        source: &Path,
        target: Option<CollectionId>,
    ) -> Result<ImportOutcome, ImportError> {
        if let Some(target) = target
            && !collection::is_usable_collection(&self.connection, target)?
        {
            return Err(ImportError::CollectionNotFound(target));
        }

        let original_file_name = importable_file_name(source)?;
        let media = media::inspect(source).map_err(|error| inspect_failure(error, source))?;

        let digest = content::digest(source)?;
        if let Some(existing) = self.find_by_content_hash(&digest.hash)? {
            return self.reuse_existing(existing, target);
        }

        let asset = NewAsset {
            id: AssetId::generate(),
            original_file_name: &original_file_name,
            media,
            digest,
        };
        self.store_new(source, &asset, target)?;
        Ok(ImportOutcome::Imported(asset.id))
    }

    fn store_new(
        &mut self,
        source: &Path,
        asset: &NewAsset,
        target: Option<CollectionId>,
    ) -> Result<(), ImportError> {
        let staging = StagingDir::create(layout::unfinished_import_dir(&self.root, asset.id))?;
        let copy_path = staging.path().join(asset.original_file_name);
        if content::copy_with_digest(source, &copy_path)? != asset.digest {
            return Err(ImportError::Unreadable(source.to_path_buf()));
        }

        let transaction = self.connection.transaction()?;
        insert_asset(&transaction, asset)?;
        if let Some(target) = target {
            add_to_collection(&transaction, asset.id, target)?;
        }
        let asset_dir = layout::asset_dir(&self.root, asset.id);
        staging.promote_to(&asset_dir)?;
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_dir_all(&asset_dir);
            return Err(error.into());
        }
        Ok(())
    }

    fn reuse_existing(
        &mut self,
        existing: AssetId,
        target: Option<CollectionId>,
    ) -> Result<ImportOutcome, ImportError> {
        let transaction = self.connection.transaction()?;
        let restored = restore_from_trash(&transaction, existing)?;
        let added = match target {
            Some(target) => add_to_collection(&transaction, existing, target)?,
            None => false,
        };
        transaction.commit()?;

        Ok(if restored {
            ImportOutcome::RestoredFromTrash(existing)
        } else if added {
            ImportOutcome::AddedToCollection(existing)
        } else {
            ImportOutcome::AlreadyPresent(existing)
        })
    }

    fn find_by_content_hash(&self, hash: &str) -> Result<Option<AssetId>, ImportError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id FROM assets WHERE content_hash = ?1",
                [hash],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn restore_from_trash(transaction: &Transaction, id: AssetId) -> Result<bool, ImportError> {
    let changed = transaction.execute(
        "UPDATE assets SET trashed_at_unix_ms = NULL
         WHERE id = ?1 AND trashed_at_unix_ms IS NOT NULL",
        [id],
    )?;
    Ok(changed > 0)
}

fn add_to_collection(
    transaction: &Transaction,
    asset: AssetId,
    collection: CollectionId,
) -> Result<bool, ImportError> {
    let changed = transaction.execute(
        "INSERT OR IGNORE INTO asset_collections (asset_id, collection_id) VALUES (?1, ?2)",
        params![asset, collection],
    )?;
    Ok(changed > 0)
}

fn importable_file_name(source: &Path) -> Result<String, ImportError> {
    let metadata =
        fs::metadata(source).map_err(|_| ImportError::Unreadable(source.to_path_buf()))?;
    match source.file_name() {
        Some(name) if metadata.is_file() => Ok(name.to_string_lossy().into_owned()),
        _ => Err(ImportError::UnsupportedFormat(source.to_path_buf())),
    }
}

fn inspect_failure(error: InspectError, source: &Path) -> ImportError {
    match error {
        InspectError::Unsupported => ImportError::UnsupportedFormat(source.to_path_buf()),
        InspectError::Unreadable => ImportError::Unreadable(source.to_path_buf()),
    }
}

fn insert_asset(transaction: &Transaction, asset: &NewAsset) -> Result<(), ImportError> {
    transaction.execute(
        "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                             width, height, byte_size, content_hash, is_animated,
                             embedded_sizes, added_at_unix_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            asset.id,
            display_name_of(asset.original_file_name),
            asset.original_file_name,
            layout::stored_path(asset.id, asset.original_file_name),
            asset.media.format,
            asset.media.dimensions.map(media::Dimensions::width),
            asset.media.dimensions.map(media::Dimensions::height),
            asset.digest.byte_size,
            asset.digest.hash,
            asset.media.is_animated,
            EmbeddedSizes(asset.media.embedded_sizes.clone()),
            clock::now_unix_ms(),
        ],
    )?;
    Ok(())
}

fn display_name_of(original_file_name: &str) -> String {
    Path::new(original_file_name).file_stem().map_or_else(
        || original_file_name.to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::display_name_of;

    #[test]
    fn the_display_name_is_the_file_name_without_its_extension() {
        assert_eq!(
            display_name_of("github-mark-white.svg"),
            "github-mark-white"
        );
        assert_eq!(display_name_of("logo.final.png"), "logo.final");
        assert_eq!(display_name_of("README"), "README");
        assert_eq!(display_name_of(".hidden.png"), ".hidden");
    }
}
