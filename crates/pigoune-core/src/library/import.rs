use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use super::asset::EmbeddedSizes;
use super::content::{self, ContentDigest};
use super::import_batch::ImportBatch;
use super::staging::StagingDir;
use super::{AssetId, CollectionId, ImportError, Library, LibraryError, clock, collection, layout};
use crate::media::{self, InspectError, MediaInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    Imported(AssetId),
    AlreadyPresent(AssetId),
    AddedToCollection(AssetId),
    RestoredFromTrash(AssetId),
}

pub(super) struct PreparedFile {
    source: PathBuf,
    original_file_name: String,
    media: MediaInfo,
    digest: ContentDigest,
}

impl PreparedFile {
    pub(super) fn byte_size(&self) -> u64 {
        self.digest.byte_size
    }
}

impl Library {
    pub fn import_file(
        &mut self,
        source: &Path,
        target: Option<CollectionId>,
    ) -> Result<ImportOutcome, ImportError> {
        self.ensure_target_is_usable(target)?;
        let prepared = prepare(source)?;
        let mut batch = ImportBatch::begin(&self.connection)?;
        match self.store_prepared(&prepared, |_| Ok(target), &mut batch) {
            Ok(outcome) => {
                batch.commit(&self.connection)?;
                Ok(outcome)
            }
            Err(error) => {
                batch.abandon(&self.connection);
                Err(error)
            }
        }
    }

    pub(super) fn ensure_target_is_usable(
        &self,
        target: Option<CollectionId>,
    ) -> Result<(), ImportError> {
        match target {
            Some(target) if !collection::is_usable_collection(&self.connection, target)? => {
                Err(ImportError::CollectionNotFound(target))
            }
            _ => Ok(()),
        }
    }

    pub(super) fn is_already_stored(&self, prepared: &PreparedFile) -> Result<bool, ImportError> {
        Ok(self.find_by_content_hash(&prepared.digest.hash)?.is_some())
    }

    pub(super) fn store_prepared(
        &mut self,
        prepared: &PreparedFile,
        place: impl FnOnce(&Connection) -> Result<Option<CollectionId>, LibraryError>,
        batch: &mut ImportBatch,
    ) -> Result<ImportOutcome, ImportError> {
        if let Some(existing) = self.find_by_content_hash(&prepared.digest.hash)? {
            return self.reuse_existing(existing, place);
        }
        let id = AssetId::generate();
        self.store_new(prepared, id, place, batch)?;
        Ok(ImportOutcome::Imported(id))
    }

    fn store_new(
        &mut self,
        prepared: &PreparedFile,
        id: AssetId,
        place: impl FnOnce(&Connection) -> Result<Option<CollectionId>, LibraryError>,
        batch: &mut ImportBatch,
    ) -> Result<(), ImportError> {
        let staging = StagingDir::create(layout::unfinished_import_dir(&self.root, id))?;
        let copy_path = staging.path().join(&prepared.original_file_name);
        if content::copy_with_digest(&prepared.source, &copy_path)? != prepared.digest {
            return Err(ImportError::Unreadable(prepared.source.clone()));
        }

        let savepoint = self.connection.savepoint()?;
        insert_asset(&savepoint, id, prepared)?;
        if let Some(collection) = place(&savepoint)? {
            add_to_collection(&savepoint, id, collection)?;
        }
        savepoint.commit()?;
        batch.stage(staging, layout::asset_dir(&self.root, id));
        Ok(())
    }

    fn reuse_existing(
        &mut self,
        existing: AssetId,
        place: impl FnOnce(&Connection) -> Result<Option<CollectionId>, LibraryError>,
    ) -> Result<ImportOutcome, ImportError> {
        let savepoint = self.connection.savepoint()?;
        let restored = restore_from_trash(&savepoint, existing)?;
        let added = match place(&savepoint)? {
            Some(collection) => add_to_collection(&savepoint, existing, collection)?,
            None => false,
        };
        savepoint.commit()?;

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

fn restore_from_trash(connection: &Connection, id: AssetId) -> Result<bool, ImportError> {
    let changed = connection.execute(
        "UPDATE assets SET trashed_at_unix_ms = NULL
         WHERE id = ?1 AND trashed_at_unix_ms IS NOT NULL",
        [id],
    )?;
    Ok(changed > 0)
}

fn add_to_collection(
    connection: &Connection,
    asset: AssetId,
    collection: CollectionId,
) -> Result<bool, ImportError> {
    let changed = connection.execute(
        "INSERT OR IGNORE INTO asset_collections (asset_id, collection_id) VALUES (?1, ?2)",
        params![asset, collection],
    )?;
    Ok(changed > 0)
}

pub(super) fn prepare(source: &Path) -> Result<PreparedFile, ImportError> {
    let original_file_name = importable_file_name(source)?;
    let media = media::inspect(source).map_err(|error| inspect_failure(error, source))?;
    let digest = content::digest(source)?;
    Ok(PreparedFile {
        source: source.to_path_buf(),
        original_file_name,
        media,
        digest,
    })
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

fn insert_asset(
    connection: &Connection,
    id: AssetId,
    asset: &PreparedFile,
) -> Result<(), ImportError> {
    connection.execute(
        "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                             width, height, byte_size, content_hash, is_animated,
                             embedded_sizes, added_at_unix_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            display_name_of(&asset.original_file_name),
            asset.original_file_name,
            layout::stored_path(id, &asset.original_file_name),
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
