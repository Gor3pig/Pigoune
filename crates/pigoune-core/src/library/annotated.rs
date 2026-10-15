use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use super::content::{self, ContentDigest};
use super::import::{self, PreparedFile};
use super::import_batch::ImportBatch;
use super::staging::StagingDir;
use super::trash::forget_thumbnails;
use super::{AnnotatedError, AssetId, Library, clock, details_store, layout};
use crate::media::{self, AssetFormat};

#[derive(Debug, Clone, Copy)]
pub struct AnnotatedImage<'a> {
    pub rendered: &'a Path,
    pub original: &'a Path,
    pub document: &'a str,
    pub has_masks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotations {
    pub document: String,
    pub has_masks: bool,
    pub updated_at_unix_ms: i64,
}

impl Library {
    pub fn import_annotated(
        &mut self,
        image: AnnotatedImage<'_>,
    ) -> Result<AssetId, AnnotatedError> {
        let prepared = prepared_png(image.rendered)?;
        if prepared.original_file_name() == layout::ORIGINAL_FILE_NAME {
            return Err(AnnotatedError::ReservedName(image.rendered.to_path_buf()));
        }
        ensure_png(image.original)?;
        if let Some(existing) = self.find_by_content_hash(&prepared.digest().hash)? {
            return Err(AnnotatedError::AlreadyPresent(existing));
        }
        let mut batch = ImportBatch::begin(&self.connection)?;
        match self.store_annotated(&prepared, image, &mut batch) {
            Ok(id) => {
                batch.commit(&self.connection)?;
                Ok(id)
            }
            Err(error) => {
                batch.abandon(&self.connection);
                Err(error)
            }
        }
    }

    pub fn update_annotated(
        &mut self,
        id: AssetId,
        rendered: &Path,
        document: &str,
        has_masks: bool,
    ) -> Result<(), AnnotatedError> {
        let asset = self.asset(id)?.ok_or(AnnotatedError::AssetNotFound(id))?;
        if self.annotations(id)?.is_none() {
            return Err(AnnotatedError::NotAnnotated(id));
        }
        let prepared = prepared_png(rendered)?;
        match self.find_by_content_hash(&prepared.digest().hash)? {
            Some(other) if other != id => return Err(AnnotatedError::AlreadyPresent(other)),
            _ => {}
        }
        let destination = self.root.join(&asset.stored_path);
        let temporary = destination.with_file_name(layout::unfinished_replacement_name(id));
        let _ = fs::remove_file(&temporary);
        let copied = content::copy_with_digest(rendered, &temporary);
        if !matches!(&copied, Ok(copied) if copied == prepared.digest()) {
            let _ = fs::remove_file(&temporary);
            return Err(AnnotatedError::Unreadable(rendered.to_path_buf()));
        }
        let result = self.swap_in(id, &prepared, &temporary, &destination, document, has_masks);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return result;
        }
        details_store::forget(&mut self.connection, &[id]);
        forget_thumbnails(&self.root, id);
        Ok(())
    }

    pub fn annotations(&self, id: AssetId) -> Result<Option<Annotations>, super::LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT document, has_masks, updated_at_unix_ms
                 FROM asset_annotations WHERE asset_id = ?1",
                [id],
                |row| {
                    Ok(Annotations {
                        document: row.get(0)?,
                        has_masks: row.get(1)?,
                        updated_at_unix_ms: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn original_path(&self, id: AssetId) -> Result<Option<PathBuf>, super::LibraryError> {
        Ok(self
            .annotations(id)?
            .map(|_| self.root.join(layout::original_stored_path(id))))
    }

    fn store_annotated(
        &mut self,
        prepared: &PreparedFile,
        image: AnnotatedImage<'_>,
        batch: &mut ImportBatch,
    ) -> Result<AssetId, AnnotatedError> {
        let id = AssetId::generate();
        let staging = StagingDir::create(layout::unfinished_import_dir(&self.root, id))?;
        let rendered_copy = staging.path().join(prepared.original_file_name());
        if &content::copy_with_digest(image.rendered, &rendered_copy)? != prepared.digest() {
            return Err(AnnotatedError::Unreadable(image.rendered.to_path_buf()));
        }
        let original_copy = staging.path().join(layout::ORIGINAL_FILE_NAME);
        let original: ContentDigest = content::copy_with_digest(image.original, &original_copy)?;

        let savepoint = self.connection.savepoint()?;
        import::insert_asset(&savepoint, id, prepared)?;
        insert_annotations(&savepoint, id, image, original.byte_size)?;
        savepoint.commit()?;
        batch.stage(staging, layout::asset_dir(&self.root, id));
        Ok(id)
    }

    fn swap_in(
        &mut self,
        id: AssetId,
        prepared: &PreparedFile,
        temporary: &Path,
        destination: &Path,
        document: &str,
        has_masks: bool,
    ) -> Result<(), AnnotatedError> {
        let transaction = self.connection.transaction()?;
        let dimensions = prepared.media().dimensions;
        transaction.execute(
            "UPDATE assets SET width = ?2, height = ?3, byte_size = ?4, content_hash = ?5,
                               is_animated = ?6, colors = NULL, visual_hash = NULL
             WHERE id = ?1",
            params![
                id,
                dimensions.map(media::Dimensions::width),
                dimensions.map(media::Dimensions::height),
                prepared.digest().byte_size,
                prepared.digest().hash,
                prepared.media().is_animated,
            ],
        )?;
        transaction.execute(
            "UPDATE asset_annotations SET document = ?2, has_masks = ?3, updated_at_unix_ms = ?4
             WHERE asset_id = ?1",
            params![id, document, has_masks, clock::now_unix_ms()],
        )?;
        fs::rename(temporary, destination)?;
        transaction.commit()?;
        Ok(())
    }
}

fn prepared_png(path: &Path) -> Result<PreparedFile, AnnotatedError> {
    let prepared = import::prepare(path)?;
    if prepared.media().format != AssetFormat::Png {
        return Err(AnnotatedError::NotPng(path.to_path_buf()));
    }
    Ok(prepared)
}

fn ensure_png(path: &Path) -> Result<(), AnnotatedError> {
    match media::inspect(path) {
        Ok(info) if info.format == AssetFormat::Png => Ok(()),
        Ok(_) | Err(media::InspectError::Unsupported) => {
            Err(AnnotatedError::NotPng(path.to_path_buf()))
        }
        Err(media::InspectError::Unreadable) => Err(AnnotatedError::Unreadable(path.to_path_buf())),
    }
}

fn insert_annotations(
    connection: &Connection,
    id: AssetId,
    image: AnnotatedImage<'_>,
    original_byte_size: u64,
) -> Result<(), AnnotatedError> {
    connection.execute(
        "INSERT INTO asset_annotations
            (asset_id, document, has_masks, original_byte_size, updated_at_unix_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            image.document,
            image.has_masks,
            original_byte_size,
            clock::now_unix_ms()
        ],
    )?;
    Ok(())
}
