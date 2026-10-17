use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use super::content::{self, ContentDigest};
use super::import::{self, PreparedFile};
use super::import_batch::ImportBatch;
use super::seal::SaveRequest;
use super::staging::StagingDir;
use super::{AnnotatedError, AssetId, Library, clock, layout};
use crate::annotations::AnnotationDocument;
use crate::media::{self, AssetFormat};

#[derive(Debug, Clone, Copy)]
pub struct AnnotatedImage<'a> {
    pub rendered: &'a Path,
    pub original: &'a Path,
    pub document: &'a AnnotationDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotations {
    pub document: String,
    pub has_masks: bool,
    pub updated_at_unix_ms: i64,
    pub seal_generation: u32,
}

impl Library {
    pub fn import_annotated(
        &mut self,
        image: AnnotatedImage<'_>,
    ) -> Result<AssetId, AnnotatedError> {
        let stored_document = image.document.to_json()?;
        let prepared = prepared_png(image.rendered)?;
        if layout::original_generation_of(prepared.original_file_name()).is_some() {
            return Err(AnnotatedError::ReservedName(image.rendered.to_path_buf()));
        }
        ensure_png(image.original)?;
        if let Some(record) = &image.document.seal
            && (!record.is_well_formed() || content::digest(image.original)?.hash != record.base)
        {
            return Err(AnnotatedError::InvalidSeal(
                "the sealed image is not the one the record names",
            ));
        }
        if let Some(existing) = self.find_by_content_hash(&prepared.digest().hash)? {
            return Err(AnnotatedError::AlreadyPresent(existing));
        }
        let mut batch = ImportBatch::begin(&self.connection)?;
        match self.store_annotated(&prepared, image, &stored_document, &mut batch) {
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
        document: &AnnotationDocument,
    ) -> Result<(), AnnotatedError> {
        self.commit_save(
            id,
            &SaveRequest {
                rendered,
                document,
                sealed_original: None,
            },
        )
    }

    pub fn seal_annotated(
        &mut self,
        id: AssetId,
        rendered: &Path,
        sealed_original: &Path,
        document: &AnnotationDocument,
    ) -> Result<(), AnnotatedError> {
        self.commit_save(
            id,
            &SaveRequest {
                rendered,
                document,
                sealed_original: Some(sealed_original),
            },
        )
    }

    pub fn annotations(&self, id: AssetId) -> Result<Option<Annotations>, super::LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT document, has_masks, updated_at_unix_ms, seal_generation
                 FROM asset_annotations WHERE asset_id = ?1",
                [id],
                |row| {
                    Ok(Annotations {
                        document: row.get(0)?,
                        has_masks: row.get(1)?,
                        updated_at_unix_ms: row.get(2)?,
                        seal_generation: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn annotation_document(
        &self,
        id: AssetId,
    ) -> Result<Option<AnnotationDocument>, AnnotatedError> {
        self.annotations(id)?
            .map(|stored| Ok(AnnotationDocument::from_json(&stored.document)?))
            .transpose()
    }

    pub fn original_path(&self, id: AssetId) -> Result<Option<PathBuf>, super::LibraryError> {
        Ok(self.annotations(id)?.map(|stored| {
            self.root
                .join(layout::original_generation_path(id, stored.seal_generation))
        }))
    }

    fn store_annotated(
        &mut self,
        prepared: &PreparedFile,
        image: AnnotatedImage<'_>,
        stored_document: &str,
        batch: &mut ImportBatch,
    ) -> Result<AssetId, AnnotatedError> {
        let id = AssetId::generate();
        let staging = StagingDir::create(layout::unfinished_import_dir(&self.root, id))?;
        let rendered_copy = staging.path().join(prepared.original_file_name());
        if &content::copy_with_digest(image.rendered, &rendered_copy)? != prepared.digest() {
            return Err(AnnotatedError::Unreadable(image.rendered.to_path_buf()));
        }
        let generation = image
            .document
            .seal
            .as_ref()
            .map_or(0, |seal| seal.generation);
        let original_copy = staging
            .path()
            .join(layout::original_generation_name(generation));
        let original: ContentDigest = content::copy_with_digest(image.original, &original_copy)?;

        let savepoint = self.connection.savepoint()?;
        import::insert_asset(&savepoint, id, prepared)?;
        insert_annotations(&savepoint, id, image, stored_document, original.byte_size)?;
        savepoint.commit()?;
        batch.stage(staging, layout::asset_dir(&self.root, id));
        Ok(id)
    }
}

pub(super) fn prepared_png(path: &Path) -> Result<PreparedFile, AnnotatedError> {
    let prepared = import::prepare(path)?;
    if prepared.media().format != AssetFormat::Png {
        return Err(AnnotatedError::NotPng(path.to_path_buf()));
    }
    Ok(prepared)
}

pub(super) fn ensure_png(path: &Path) -> Result<(), AnnotatedError> {
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
    stored_document: &str,
    original_byte_size: u64,
) -> Result<(), AnnotatedError> {
    connection.execute(
        "INSERT INTO asset_annotations
            (asset_id, document, has_masks, original_byte_size, updated_at_unix_ms,
             seal_generation, seal_base_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            stored_document,
            image.document.hides_content(),
            original_byte_size,
            clock::now_unix_ms(),
            image
                .document
                .seal
                .as_ref()
                .map_or(0, |seal| seal.generation),
            image.document.seal.as_ref().map(|seal| seal.base.as_str()),
        ],
    )?;
    Ok(())
}
