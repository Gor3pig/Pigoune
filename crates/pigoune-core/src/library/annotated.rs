use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use super::trash::forget_thumbnails;
use super::{
    AnnotatedError, AssetId, ImportOutcome, Library, LibraryError, clock, details_store, layout,
};
use crate::annotations::AnnotationDocument;

pub const RENDER_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationRender {
    pub key: String,
    pub document: String,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotations {
    pub document: String,
    pub has_masks: bool,
    pub updated_at_unix_ms: i64,
}

impl Library {
    pub fn annotations(&self, id: AssetId) -> Result<Option<Annotations>, LibraryError> {
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

    pub fn annotation_document(
        &self,
        id: AssetId,
    ) -> Result<Option<AnnotationDocument>, AnnotatedError> {
        self.annotations(id)?
            .map(|stored| Ok(AnnotationDocument::from_json(&stored.document)?))
            .transpose()
    }

    pub fn annotated_ids(&self) -> Result<HashSet<AssetId>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT asset_id FROM asset_annotations")?;
        let ids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn save_annotations(
        &mut self,
        id: AssetId,
        document: &AnnotationDocument,
    ) -> Result<(), AnnotatedError> {
        let stored_document = document.to_json()?;
        self.asset(id)?.ok_or(AnnotatedError::AssetNotFound(id))?;
        let has_any = !document.objects.is_empty() || document.crop.is_some();
        let transaction = self.connection.transaction()?;
        if has_any {
            transaction.execute(
                "INSERT INTO asset_annotations (asset_id, document, has_masks, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (asset_id) DO UPDATE SET
                    document = excluded.document,
                    has_masks = excluded.has_masks,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    id,
                    stored_document,
                    document.hides_content(),
                    clock::now_unix_ms()
                ],
            )?;
        } else {
            transaction.execute("DELETE FROM asset_annotations WHERE asset_id = ?1", [id])?;
        }
        transaction.commit()?;
        details_store::forget(&mut self.connection, &[id]);
        forget_thumbnails(&self.root, id);
        Ok(())
    }

    pub fn import_with_annotations(
        &mut self,
        source: &Path,
        document: &AnnotationDocument,
    ) -> Result<AssetId, AnnotatedError> {
        document.to_json()?;
        match self.import_file(source, None)? {
            ImportOutcome::Imported(id) => {
                self.save_annotations(id, document)?;
                Ok(id)
            }
            ImportOutcome::AlreadyPresent(id)
            | ImportOutcome::AddedToCollection(id)
            | ImportOutcome::RestoredFromTrash(id) => Err(AnnotatedError::AlreadyPresent(id)),
        }
    }

    pub fn annotation_render_key(&self, id: AssetId) -> Result<Option<String>, LibraryError> {
        Ok(self.annotation_render(id)?.map(|render| render.key))
    }

    pub fn annotation_render(&self, id: AssetId) -> Result<Option<AnnotationRender>, LibraryError> {
        let Some(asset) = self.asset(id)? else {
            return Ok(None);
        };
        let Some(stored) = self.annotations(id)? else {
            return Ok(None);
        };
        Ok(Some(self.render_of(
            id,
            &asset.content_hash,
            stored.document,
        )))
    }

    pub fn annotation_renders(&self) -> Result<HashMap<AssetId, AnnotationRender>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT assets.id, assets.content_hash, asset_annotations.document
             FROM asset_annotations JOIN assets ON assets.id = asset_annotations.asset_id",
        )?;
        let found: Vec<(AssetId, String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<_, _>>()?;
        Ok(found
            .into_iter()
            .map(|(id, hash, document)| (id, self.render_of(id, &hash, document)))
            .collect())
    }

    fn render_of(&self, id: AssetId, content_hash: &str, document: String) -> AnnotationRender {
        let mut hasher = blake3::Hasher::new();
        hasher.update(content_hash.as_bytes());
        hasher.update(&[0]);
        hasher.update(document.as_bytes());
        hasher.update(&[0]);
        hasher.update(&RENDER_VERSION.to_le_bytes());
        let key = hasher.finalize().to_hex()[..32].to_owned();
        AnnotationRender {
            file: layout::annotated_render_path(&self.root, id, &key),
            key,
            document,
        }
    }
}
