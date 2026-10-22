use std::collections::{HashMap, HashSet};

use super::cache_guard::{CacheFile, cache_file};
use super::collection::{id_batches, ids_as_json};
use super::{AssetId, FlatFiles, Library, LibraryError};
use crate::annotations::AnnotationDocument;

#[derive(Debug, Default)]
pub struct DragOutCheck {
    pub flats: FlatFiles,
    pub not_ready: usize,
    pub hidden_mask_names: Vec<String>,
}

struct AnnotatedRow {
    content_hash: String,
    display_name: String,
    document: String,
}

impl Library {
    #[must_use]
    pub fn check_drag_out(&self, assets: &[AssetId]) -> DragOutCheck {
        let mut rows = HashMap::new();
        let mut unreadable = HashSet::new();
        for batch in id_batches(assets) {
            match self.annotated_rows(&batch) {
                Ok(found) => rows.extend(found),
                Err(_) => unreadable.extend(batch),
            }
        }
        let mut check = DragOutCheck::default();
        for id in assets {
            if unreadable.contains(id) {
                check.not_ready += 1;
                continue;
            }
            let Some(row) = rows.get(id) else {
                continue;
            };
            let render = self.render_of(*id, &row.content_hash, row.document.clone());
            if cache_file(&render.file) == CacheFile::Plain {
                check.flats.insert(*id, render.file);
            } else {
                check.not_ready += 1;
            }
            if AnnotationDocument::from_json(&row.document)
                .is_ok_and(|document| !document.exposed_masks().is_empty())
            {
                check.hidden_mask_names.push(row.display_name.clone());
            }
        }
        check
    }

    fn annotated_rows(
        &self,
        batch: &[AssetId],
    ) -> Result<Vec<(AssetId, AnnotatedRow)>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT assets.id, assets.content_hash, assets.display_name, asset_annotations.document
             FROM asset_annotations JOIN assets ON assets.id = asset_annotations.asset_id
             WHERE asset_annotations.asset_id IN (SELECT value FROM json_each(?1))",
        )?;
        let rows = statement
            .query_map([ids_as_json(batch)], |row| {
                Ok((
                    row.get(0)?,
                    AnnotatedRow {
                        content_hash: row.get(1)?,
                        display_name: row.get(2)?,
                        document: row.get(3)?,
                    },
                ))
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}
