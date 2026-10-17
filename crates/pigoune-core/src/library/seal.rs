use std::fs::{self, File};
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::content;
use super::{AnnotatedError, AssetId, Library, LibraryError, layout};
use crate::annotations::AnnotationDocument;

const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealStep {
    AfterPrepare,
    AfterOriginalPartial,
    AfterRenderedPartial,
    AfterCommit,
    AfterInstallOriginal,
    AfterInstallRendered,
    AfterPurge,
    BeforeClear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealProblem {
    DocumentUnreadable,
    Contradiction,
    OriginalMissing,
    OriginalChanged,
    RenderedMissing,
    RenderedChanged,
    JournalDamaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealStatus {
    Unsealed,
    Verified { generation: u32 },
    Finalizing,
    Unverified(SealProblem),
}

impl SealStatus {
    #[must_use]
    pub fn allows_output(self) -> bool {
        matches!(self, Self::Unsealed | Self::Verified { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinalizeReport {
    pub complete: bool,
    pub kept: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Install {
    partial: String,
    target: String,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Journal {
    version: u32,
    to_generation: u32,
    installs: Vec<Install>,
    purge: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StoredSeal {
    pub generation: u32,
    pub base: Option<String>,
    pub phase: String,
    pub journal: Option<String>,
}

pub struct SaveRequest<'a> {
    pub rendered: &'a Path,
    pub document: &'a AnnotationDocument,
    pub sealed_original: Option<&'a Path>,
}

impl Library {
    #[doc(hidden)]
    pub fn fail_at(&mut self, step: Option<SealStep>) {
        self.failpoint = step;
    }

    fn reach(&mut self, step: SealStep) -> Result<(), AnnotatedError> {
        if self.failpoint == Some(step) {
            self.failpoint = None;
            return Err(AnnotatedError::Interrupted(step));
        }
        Ok(())
    }

    pub(super) fn stored_seal(&self, id: AssetId) -> Result<Option<StoredSeal>, LibraryError> {
        Ok(self
            .connection
            .query_row(
                "SELECT seal_generation, seal_base_hash, seal_phase, seal_journal
                 FROM asset_annotations WHERE asset_id = ?1",
                [id],
                |row| {
                    Ok(StoredSeal {
                        generation: row.get(0)?,
                        base: row.get(1)?,
                        phase: row.get(2)?,
                        journal: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn seal_status(&self, id: AssetId) -> Result<SealStatus, LibraryError> {
        let Some(stored) = self.stored_seal(id)? else {
            return Ok(SealStatus::Unsealed);
        };
        let journal = stored
            .journal
            .as_deref()
            .map(serde_json::from_str::<Journal>);
        if stored.phase == "finalizing" {
            return Ok(match journal {
                Some(Ok(journal)) if journal.to_generation == 0 && stored.generation == 0 => {
                    SealStatus::Unsealed
                }
                Some(Ok(_)) => SealStatus::Finalizing,
                _ => SealStatus::Unverified(SealProblem::JournalDamaged),
            });
        }
        let Some(annotations) = self.annotations(id)? else {
            return Ok(SealStatus::Unsealed);
        };
        let document = AnnotationDocument::from_json(&annotations.document);
        let claims = stored.generation >= 1
            || stored.base.is_some()
            || document
                .as_ref()
                .is_ok_and(|document| document.seal.is_some() || document.sealed_count() > 0);
        if !claims {
            return Ok(SealStatus::Unsealed);
        }
        let Ok(document) = document else {
            return Ok(SealStatus::Unverified(SealProblem::DocumentUnreadable));
        };
        let (Some(base), Some(record)) = (stored.base.as_deref(), document.seal.as_ref()) else {
            return Ok(SealStatus::Unverified(SealProblem::Contradiction));
        };
        if stored.generation == 0 || record.generation != stored.generation || record.base != base {
            return Ok(SealStatus::Unverified(SealProblem::Contradiction));
        }
        Ok(self.verify_files(id, stored.generation, base))
    }

    fn verify_files(&self, id: AssetId, generation: u32, base: &str) -> SealStatus {
        let original = self
            .root
            .join(layout::original_generation_path(id, generation));
        match digest_of(&original) {
            Err(Missing::Absent) => return SealStatus::Unverified(SealProblem::OriginalMissing),
            Err(Missing::Unreadable) => {
                return SealStatus::Unverified(SealProblem::OriginalChanged);
            }
            Ok(found) if found != base => {
                return SealStatus::Unverified(SealProblem::OriginalChanged);
            }
            Ok(_) => {}
        }
        let Ok(Some(asset)) = self.asset(id) else {
            return SealStatus::Unverified(SealProblem::RenderedMissing);
        };
        match digest_of(&self.root.join(&asset.stored_path)) {
            Err(Missing::Absent) => SealStatus::Unverified(SealProblem::RenderedMissing),
            Err(Missing::Unreadable) => SealStatus::Unverified(SealProblem::RenderedChanged),
            Ok(found) if found != asset.content_hash => {
                SealStatus::Unverified(SealProblem::RenderedChanged)
            }
            Ok(_) => SealStatus::Verified { generation },
        }
    }

    pub fn ensure_may_leave(&self, id: AssetId) -> Result<(), AnnotatedError> {
        let status = self.seal_status(id)?;
        if status.allows_output() {
            Ok(())
        } else {
            Err(AnnotatedError::SealBlocked(status))
        }
    }

    pub fn seal_attention(&self) -> Result<Vec<(AssetId, SealStatus)>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT asset_id FROM asset_annotations")?;
        let ids: Vec<AssetId> = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut attention = Vec::new();
        for id in ids {
            let status = self.seal_status(id)?;
            if !status.allows_output() {
                attention.push((id, status));
            }
        }
        Ok(attention)
    }

    pub fn blocked_paths(&self) -> Result<Vec<PathBuf>, LibraryError> {
        let mut paths = Vec::new();
        for (id, _) in self.seal_attention()? {
            if let Some(asset) = self.asset(id)? {
                paths.push(self.root.join(&asset.stored_path));
            }
            let folder = self.asset_folder(id);
            if let Ok(entries) = fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    let is_original = entry
                        .file_name()
                        .to_str()
                        .and_then(layout::original_generation_of)
                        .is_some();
                    if is_original {
                        paths.push(entry.path());
                    }
                }
            }
        }
        Ok(paths)
    }

    pub(super) fn commit_save(
        &mut self,
        id: AssetId,
        request: &SaveRequest<'_>,
    ) -> Result<(), AnnotatedError> {
        let asset = self.asset(id)?.ok_or(AnnotatedError::AssetNotFound(id))?;
        let stored = self
            .stored_seal(id)?
            .ok_or(AnnotatedError::NotAnnotated(id))?;
        let status = self.seal_status(id)?;
        if !status.allows_output() {
            return Err(AnnotatedError::SealBlocked(status));
        }
        let next_generation = Self::check_seal_request(&stored, request)?;
        let document_text = request.document.to_json()?;
        let prepared = super::annotated::prepared_png(request.rendered)?;
        match self.find_by_content_hash(&prepared.digest().hash)? {
            Some(other) if other != id => return Err(AnnotatedError::AlreadyPresent(other)),
            _ => {}
        }
        let destination = self.root.join(&asset.stored_path);
        let folder = destination
            .parent()
            .ok_or_else(|| AnnotatedError::Unreadable(destination.clone()))?
            .to_path_buf();
        if request.sealed_original.is_some()
            && exists_at(
                &self
                    .root
                    .join(layout::original_generation_path(id, next_generation)),
            )
        {
            return Err(AnnotatedError::InvalidSeal(
                "a file already uses the name of the new sealed image",
            ));
        }
        let rendered_partial_name = layout::unfinished_replacement_name(id);
        let original_partial_name = layout::unfinished_seal_name(id, next_generation);
        let mut installs = vec![Install {
            partial: rendered_partial_name.clone(),
            target: asset.stored_path.to_string_lossy().into_owned(),
            hash: prepared.digest().hash.clone(),
        }];
        let mut purge = Vec::new();
        let mut original_hash = None;
        if let Some(original) = request.sealed_original {
            let hash = content::digest(original)?.hash;
            installs.push(Install {
                partial: original_partial_name.clone(),
                target: layout::original_generation_path(id, next_generation),
                hash: hash.clone(),
            });
            purge.push(layout::original_generation_path(id, stored.generation));
            original_hash = Some(hash);
        }
        let journal = Journal {
            version: JOURNAL_VERSION,
            to_generation: next_generation,
            installs: installs.clone(),
            purge,
        };
        let journal_text = serde_json::to_string(&journal).map_err(io_error)?;
        let prepare = Journal {
            purge: Vec::new(),
            ..journal.clone()
        };
        let prepare_text = serde_json::to_string(&prepare).map_err(io_error)?;
        self.set_phase(id, "preparing", Some(&prepare_text))?;
        let result = self.write_and_commit(
            id,
            (request, &folder),
            (&installs, original_hash.as_deref()),
            (&document_text, &prepared, next_generation, &journal_text),
        );
        if let Err(error) = result {
            if !matches!(error, AnnotatedError::Interrupted(_)) {
                self.rollback_preparing(id);
            }
            return Err(error);
        }
        self.finalize_save(id)?;
        super::details_store::forget(&mut self.connection, &[id]);
        super::trash::forget_thumbnails(&self.root, id);
        Ok(())
    }

    fn check_seal_request(
        stored: &StoredSeal,
        request: &SaveRequest<'_>,
    ) -> Result<u32, AnnotatedError> {
        let record = request.document.seal.as_ref();
        match request.sealed_original {
            None => {
                let matches_stored = match (record, stored.base.as_deref()) {
                    (None, None) => stored.generation == 0,
                    (Some(record), Some(base)) => {
                        record.generation == stored.generation && record.base == base
                    }
                    _ => false,
                };
                if matches_stored {
                    Ok(stored.generation)
                } else {
                    Err(AnnotatedError::InvalidSeal(
                        "the document does not match the sealing recorded in the library",
                    ))
                }
            }
            Some(original) => {
                let record = record.ok_or(AnnotatedError::InvalidSeal(
                    "a sealing needs a sealing record in the document",
                ))?;
                if !record.is_well_formed() || record.generation != stored.generation + 1 {
                    return Err(AnnotatedError::InvalidSeal(
                        "the sealing generation does not follow the stored one",
                    ));
                }
                super::annotated::ensure_png(original)?;
                if content::digest(original)?.hash != record.base {
                    return Err(AnnotatedError::InvalidSeal(
                        "the sealed image is not the one the record names",
                    ));
                }
                Ok(record.generation)
            }
        }
    }

    fn write_and_commit(
        &mut self,
        id: AssetId,
        (request, folder): (&SaveRequest<'_>, &Path),
        (installs, original_hash): (&[Install], Option<&str>),
        (document_text, prepared, next_generation, journal_text): (
            &str,
            &super::import::PreparedFile,
            u32,
            &str,
        ),
    ) -> Result<(), AnnotatedError> {
        self.reach(SealStep::AfterPrepare)?;
        if let Some(original) = request.sealed_original {
            let partial = folder.join(&installs[1].partial);
            remove_if_regular(&partial);
            let copied = content::copy_with_digest(original, &partial)?;
            if Some(copied.hash.as_str()) != original_hash {
                return Err(AnnotatedError::Unreadable(original.to_path_buf()));
            }
            self.reach(SealStep::AfterOriginalPartial)?;
        }
        let rendered_partial = folder.join(&installs[0].partial);
        remove_if_regular(&rendered_partial);
        let copied = content::copy_with_digest(request.rendered, &rendered_partial)?;
        if &copied != prepared.digest() {
            return Err(AnnotatedError::Unreadable(request.rendered.to_path_buf()));
        }
        sync_folder(folder);
        self.reach(SealStep::AfterRenderedPartial)?;
        let base = request.document.seal.as_ref().map(|seal| seal.base.clone());
        let original_size = match request.sealed_original {
            Some(original) => Some(content::digest(original)?.byte_size),
            None => None,
        };
        let transaction = self.connection.transaction()?;
        let dimensions = prepared.media().dimensions;
        transaction.execute(
            "UPDATE assets SET width = ?2, height = ?3, byte_size = ?4, content_hash = ?5,
                               is_animated = ?6, colors = NULL, visual_hash = NULL
             WHERE id = ?1",
            params![
                id,
                dimensions.map(crate::media::Dimensions::width),
                dimensions.map(crate::media::Dimensions::height),
                prepared.digest().byte_size,
                prepared.digest().hash,
                prepared.media().is_animated,
            ],
        )?;
        transaction.execute(
            "UPDATE asset_annotations
             SET document = ?2, has_masks = ?3, updated_at_unix_ms = ?4,
                 seal_generation = ?5, seal_base_hash = ?6,
                 original_byte_size = COALESCE(?7, original_byte_size),
                 seal_phase = 'finalizing', seal_journal = ?8
             WHERE asset_id = ?1",
            params![
                id,
                document_text,
                request.document.hides_content(),
                super::clock::now_unix_ms(),
                next_generation,
                base,
                original_size,
                journal_text,
            ],
        )?;
        transaction.commit()?;
        self.reach(SealStep::AfterCommit)?;
        Ok(())
    }

    fn set_phase(
        &mut self,
        id: AssetId,
        phase: &str,
        journal: Option<&str>,
    ) -> Result<(), AnnotatedError> {
        self.connection.execute(
            "UPDATE asset_annotations SET seal_phase = ?2, seal_journal = ?3 WHERE asset_id = ?1",
            params![id, phase, journal],
        )?;
        Ok(())
    }

    fn asset_folder(&self, id: AssetId) -> PathBuf {
        layout::asset_dir(&self.root, id)
    }

    fn rollback_preparing(&mut self, id: AssetId) -> Vec<PathBuf> {
        let folder = self.asset_folder(id);
        let journal = self
            .stored_seal(id)
            .ok()
            .flatten()
            .and_then(|stored| stored.journal)
            .and_then(|text| serde_json::from_str::<Journal>(&text).ok());
        let mut kept = Vec::new();
        if let Some(journal) = journal {
            for install in &journal.installs {
                let partial = folder.join(&install.partial);
                let removable = is_ours(id, &install.partial);
                if !removable || (exists_at(&partial) && !remove_if_regular(&partial)) {
                    kept.push(partial);
                }
            }
        }
        let _ = self.set_phase(id, "none", None);
        kept
    }

    pub fn finalize_save(&mut self, id: AssetId) -> Result<FinalizeReport, AnnotatedError> {
        let Some(stored) = self.stored_seal(id)? else {
            return Ok(FinalizeReport {
                complete: true,
                kept: Vec::new(),
            });
        };
        match stored.phase.as_str() {
            "preparing" => {
                let kept = self.rollback_preparing(id);
                Ok(FinalizeReport {
                    complete: true,
                    kept,
                })
            }
            "finalizing" => self.finish_installing(id, &stored),
            _ => Ok(FinalizeReport {
                complete: true,
                kept: Vec::new(),
            }),
        }
    }

    fn finish_installing(
        &mut self,
        id: AssetId,
        stored: &StoredSeal,
    ) -> Result<FinalizeReport, AnnotatedError> {
        let journal: Journal = stored
            .journal
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .ok_or(AnnotatedError::InvalidSeal("the saving journal is damaged"))?;
        let folder = self.asset_folder(id);
        for install in &journal.installs {
            self.install(id, &folder, install)?;
            let step = if layout::original_generation_of(file_name_of(&install.target)).is_some() {
                SealStep::AfterInstallOriginal
            } else {
                SealStep::AfterInstallRendered
            };
            self.reach(step)?;
        }
        let mut kept = Vec::new();
        for path in &journal.purge {
            let full = self.root.join(path);
            if !purgeable(&folder, &full, journal.to_generation) {
                kept.push(full);
                continue;
            }
            if exists_at(&full) && !remove_if_regular(&full) {
                kept.push(full);
            }
        }
        sync_folder(&folder);
        self.reach(SealStep::AfterPurge)?;
        if !kept.is_empty() {
            return Ok(FinalizeReport {
                complete: false,
                kept,
            });
        }
        self.reach(SealStep::BeforeClear)?;
        self.set_phase(id, "none", None)?;
        Ok(FinalizeReport {
            complete: true,
            kept,
        })
    }

    fn install(&self, id: AssetId, folder: &Path, install: &Install) -> Result<(), AnnotatedError> {
        if !is_ours(id, &install.partial) {
            return Err(AnnotatedError::InvalidSeal(
                "the journal names a file that does not belong to Pigoune",
            ));
        }
        let partial = folder.join(&install.partial);
        let target = self.root.join(&install.target);
        if target.parent() != Some(folder) {
            return Err(AnnotatedError::InvalidSeal(
                "the journal names a file outside the folder of the image",
            ));
        }
        if is_regular(&partial) {
            match digest_of(&partial) {
                Ok(found) if found == install.hash => {}
                _ => return Err(AnnotatedError::Unreadable(partial)),
            }
            let is_new_original = layout::original_generation_of(file_name_of(&install.target))
                .is_some_and(|generation| generation >= 1);
            if is_new_original && exists_at(&target) {
                if digest_of(&target).is_ok_and(|found| found == install.hash) {
                    remove_if_regular(&partial);
                } else {
                    return Err(AnnotatedError::InvalidSeal(
                        "a file already uses the name of the new sealed image",
                    ));
                }
            } else {
                fs::rename(&partial, &target)?;
            }
            sync_folder(folder);
        }
        match digest_of(&target) {
            Ok(found) if found == install.hash => Ok(()),
            _ => Err(AnnotatedError::Unreadable(target)),
        }
    }

    pub fn recover_interrupted_saves(&mut self) -> Vec<(AssetId, FinalizeReport)> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT asset_id FROM asset_annotations WHERE seal_phase != 'none'")
        else {
            return Vec::new();
        };
        let ids: Vec<AssetId> = statement
            .query_map([], |row| row.get(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        drop(statement);
        ids.into_iter()
            .filter_map(|id| self.finalize_save(id).ok().map(|report| (id, report)))
            .collect()
    }
}

enum Missing {
    Absent,
    Unreadable,
}

fn digest_of(path: &Path) -> Result<String, Missing> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Missing::Absent),
        Err(_) => Err(Missing::Unreadable),
        Ok(metadata) if !metadata.is_file() => Err(Missing::Unreadable),
        Ok(_) => content::digest(path)
            .map(|digest| digest.hash)
            .map_err(|_| Missing::Unreadable),
    }
}

fn io_error(error: serde_json::Error) -> AnnotatedError {
    AnnotatedError::Library(LibraryError::Io(std::io::Error::other(error)))
}

fn is_regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn exists_at(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove_if_regular(path: &Path) -> bool {
    is_regular(path) && fs::remove_file(path).is_ok()
}

fn sync_folder(folder: &Path) {
    if let Ok(handle) = File::open(folder) {
        let _ = handle.sync_all();
    }
}

fn file_name_of(stored: &str) -> &str {
    stored.rsplit('/').next().unwrap_or(stored)
}

fn is_ours(id: AssetId, partial: &str) -> bool {
    partial == layout::unfinished_replacement_name(id)
        || layout::is_unfinished_seal_name(id, partial)
}

fn purgeable(folder: &Path, full: &Path, to_generation: u32) -> bool {
    full.parent() == Some(folder)
        && full
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(layout::original_generation_of)
            .is_some_and(|generation| generation < to_generation)
}
