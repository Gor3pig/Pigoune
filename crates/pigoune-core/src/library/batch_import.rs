use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::import::{self, ImportOutcome};
use super::import_plan::{ImportPlan, PlannedFile};
use super::{AssetId, CollectionId, ImportError, Library, LibraryError, collection};

pub const LARGE_FILE_BYTES: u64 = 50_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportProgress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportControl {
    Continue,
    Cancel,
}

#[derive(Debug, Default)]
pub enum ImportEnding {
    #[default]
    Completed,
    Cancelled,
    Interrupted(ImportError),
}

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: Vec<AssetId>,
    pub large_imported: usize,
    pub already_present: usize,
    pub added_to_collection: usize,
    pub restored_from_trash: usize,
    pub unsupported: usize,
    pub unreadable: Vec<PathBuf>,
    pub ending: ImportEnding,
}

impl ImportSummary {
    fn record(&mut self, result: Result<(ImportOutcome, u64), ImportError>) -> Option<ImportError> {
        match result {
            Ok((ImportOutcome::Imported(id), byte_size)) => {
                self.imported.push(id);
                if byte_size > LARGE_FILE_BYTES {
                    self.large_imported += 1;
                }
            }
            Ok((ImportOutcome::AlreadyPresent(_), _)) => self.already_present += 1,
            Ok((ImportOutcome::AddedToCollection(_), _)) => self.added_to_collection += 1,
            Ok((ImportOutcome::RestoredFromTrash(_), _)) => self.restored_from_trash += 1,
            Err(ImportError::UnsupportedFormat(_)) => self.unsupported += 1,
            Err(ImportError::Unreadable(path)) => self.unreadable.push(path),
            Err(serious) => return Some(serious),
        }
        None
    }
}

impl Library {
    pub fn import_paths(
        &mut self,
        paths: &[PathBuf],
        target: Option<CollectionId>,
        mut is_intact: impl FnMut(&Path) -> bool,
        mut on_progress: impl FnMut(ImportProgress) -> ImportControl,
    ) -> Result<ImportSummary, ImportError> {
        self.ensure_target_is_usable(target)?;
        let plan = ImportPlan::of(paths);
        let total = plan.files.len();
        let mut summary = ImportSummary {
            unreadable: plan.unreadable,
            ..ImportSummary::default()
        };
        let mut folders = FolderCollections::new(target);

        for (done, file) in plan.files.iter().enumerate() {
            if on_progress(ImportProgress { done, total }) == ImportControl::Cancel {
                summary.ending = ImportEnding::Cancelled;
                break;
            }
            let result = self.import_planned(file, &mut folders, &mut is_intact);
            if let Some(serious) = summary.record(result) {
                summary.ending = ImportEnding::Interrupted(serious);
                break;
            }
        }
        Ok(summary)
    }

    fn import_planned(
        &mut self,
        file: &PlannedFile,
        folders: &mut FolderCollections,
        is_intact: &mut impl FnMut(&Path) -> bool,
    ) -> Result<(ImportOutcome, u64), ImportError> {
        let prepared = import::prepare(&file.source)?;
        if !self.is_already_stored(&prepared)? && !is_intact(&file.source) {
            return Err(ImportError::Unreadable(file.source.clone()));
        }
        let outcome = self.store_prepared(&prepared, |connection| {
            folders.resolve(connection, &file.folders)
        })?;
        Ok((outcome, prepared.byte_size()))
    }
}

struct FolderCollections {
    target: Option<CollectionId>,
    known: HashMap<Vec<String>, CollectionId>,
}

impl FolderCollections {
    fn new(target: Option<CollectionId>) -> Self {
        Self {
            target,
            known: HashMap::new(),
        }
    }

    fn resolve(
        &mut self,
        connection: &Connection,
        folders: &[String],
    ) -> Result<Option<CollectionId>, LibraryError> {
        let mut parent = self.target;
        let mut key = Vec::with_capacity(folders.len());
        for name in folders {
            key.push(collection::comparable_name(name));
            let id = if let Some(&known) = self.known.get(&key) {
                known
            } else {
                let found = find_or_create(connection, parent, name)?;
                self.known.insert(key.clone(), found);
                found
            };
            parent = Some(id);
        }
        Ok(parent)
    }
}

fn find_or_create(
    connection: &Connection,
    parent: Option<CollectionId>,
    name: &str,
) -> Result<CollectionId, LibraryError> {
    let name = name.trim();
    match collection::find_child_named(connection, parent, name)? {
        Some(existing) => Ok(existing),
        None => collection::insert_collection(connection, name, parent),
    }
}
