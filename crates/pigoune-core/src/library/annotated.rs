use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use super::collection::{id_batches, ids_as_json};
use super::trash::forget_thumbnails;
use super::{
    AnnotatedError, AssetId, ImportOutcome, Library, LibraryError, clock, details_store, layout,
};
use crate::annotations::AnnotationDocument;

pub const RENDER_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationRender {
    pub key: String,
    pub document: String,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotations {
    pub document: String,
    pub updated_at_unix_ms: i64,
}

impl Library {
    pub fn annotations(&self, id: AssetId) -> Result<Option<Annotations>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT document, updated_at_unix_ms
                 FROM asset_annotations WHERE asset_id = ?1",
                [id],
                |row| {
                    Ok(Annotations {
                        document: row.get(0)?,
                        updated_at_unix_ms: row.get(1)?,
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

    pub(super) fn annotated_among(
        &self,
        assets: &[AssetId],
    ) -> Result<HashSet<AssetId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT asset_id FROM asset_annotations
             WHERE asset_id IN (SELECT value FROM json_each(?1))",
        )?;
        let mut found = HashSet::new();
        for batch in id_batches(assets) {
            for id in statement.query_map([ids_as_json(&batch)], |row| row.get(0))? {
                found.insert(id?);
            }
        }
        Ok(found)
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
                "INSERT INTO asset_annotations (asset_id, document, updated_at_unix_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT (asset_id) DO UPDATE SET
                    document = excluded.document,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![id, stored_document, clock::now_unix_ms()],
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

    pub(super) fn render_of(
        &self,
        id: AssetId,
        content_hash: &str,
        document: String,
    ) -> AnnotationRender {
        let key = render_key(content_hash, &document, RENDER_VERSION);
        AnnotationRender {
            file: layout::annotated_render_path(&self.root, id, &key),
            key,
            document,
        }
    }
}

fn render_key(content_hash: &str, document: &str, version: u32) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(content_hash.as_bytes());
    hasher.update(&[0]);
    hasher.update(document.as_bytes());
    hasher.update(&[0]);
    hasher.update(&version.to_le_bytes());
    hasher.finalize().to_hex()[..32].to_owned()
}

impl Library {
    pub(super) fn forget_stale_annotated_renders(&self) {
        let Ok(renders) = self.annotation_renders() else {
            return;
        };
        let current: std::collections::HashSet<std::path::PathBuf> =
            renders.into_values().map(|render| render.file).collect();
        let annotated = layout::annotated_dir(&self.root);
        if super::cache_guard::check_folder(&self.root, &annotated).is_err() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(annotated) else {
            return;
        };
        for entry in entries.flatten() {
            let is_ours = entry
                .file_name()
                .to_str()
                .is_some_and(|name| layout::annotated_render_owner(name).is_some());
            let is_plain_file = entry.file_type().is_ok_and(|kind| kind.is_file());
            if is_ours && is_plain_file && !current.contains(&entry.path()) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RENDER_VERSION, render_key};

    #[test]
    fn a_new_version_of_the_drawing_gives_a_new_key_for_the_same_document() {
        let document = r#"{"version":1}"#;
        assert_ne!(
            render_key("hash", document, RENDER_VERSION),
            render_key("hash", document, RENDER_VERSION - 1)
        );
        assert_eq!(
            render_key("hash", document, RENDER_VERSION),
            render_key("hash", document, RENDER_VERSION)
        );
    }
}
