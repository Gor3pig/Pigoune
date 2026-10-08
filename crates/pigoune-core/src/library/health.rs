use std::collections::HashSet;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use super::{AssetId, Library, LibraryError, content, layout};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthTarget {
    pub id: AssetId,
    pub display_name: String,
    pub collection: Option<String>,
    stored_path: PathBuf,
    expected_hash: String,
    expected_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthPlan {
    root: PathBuf,
    targets: Vec<HealthTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthProgress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthIssue {
    pub id: AssetId,
    pub display_name: String,
    pub collection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HealthReport {
    pub checked: usize,
    pub missing: Vec<HealthIssue>,
    pub damaged: Vec<HealthIssue>,
    pub unrecorded: Vec<PathBuf>,
}

impl HealthReport {
    #[must_use]
    pub fn problems(&self) -> usize {
        self.missing.len() + self.damaged.len() + self.unrecorded.len()
    }
}

impl Library {
    pub fn health_plan(&self) -> Result<HealthPlan, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, display_name, stored_path, content_hash, byte_size,
                (SELECT min(collections.name) FROM asset_collections
                    JOIN collections ON collections.id = asset_collections.collection_id
                    WHERE asset_collections.asset_id = assets.id
                    AND collections.trashed_at_unix_ms IS NULL)
             FROM assets ORDER BY added_at_unix_ms, id",
        )?;
        let targets = statement
            .query_map([], |row| {
                Ok(HealthTarget {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    stored_path: PathBuf::from(row.get::<_, String>(2)?),
                    expected_hash: row.get(3)?,
                    expected_bytes: row.get(4)?,
                    collection: row.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(HealthPlan {
            root: self.root.clone(),
            targets,
        })
    }
}

impl HealthPlan {
    #[must_use]
    pub fn total(&self) -> usize {
        self.targets.len()
    }

    pub fn run(
        &self,
        mut progress: impl FnMut(HealthProgress) -> ControlFlow<()>,
    ) -> Option<HealthReport> {
        let total = self.targets.len();
        let mut report = HealthReport::default();
        for (done, target) in self.targets.iter().enumerate() {
            if progress(HealthProgress { done, total }).is_break() {
                return None;
            }
            match self.condition_of(target) {
                Condition::Intact => {}
                Condition::Missing => report.missing.push(issue_of(target)),
                Condition::Damaged => report.damaged.push(issue_of(target)),
            }
            report.checked += 1;
        }
        if progress(HealthProgress { done: total, total }).is_break() {
            return None;
        }
        report.unrecorded = self.unrecorded_files();
        Some(report)
    }

    fn condition_of(&self, target: &HealthTarget) -> Condition {
        let file = self.root.join(&target.stored_path);
        match fs::metadata(&file) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Condition::Missing,
            Err(_) => Condition::Damaged,
            Ok(metadata) if !metadata.is_file() => Condition::Missing,
            Ok(metadata) if metadata.len() != target.expected_bytes => Condition::Damaged,
            Ok(_) => match content::digest(&file) {
                Ok(digest) if digest.hash == target.expected_hash => Condition::Intact,
                _ => Condition::Damaged,
            },
        }
    }

    fn unrecorded_files(&self) -> Vec<PathBuf> {
        let recorded: HashSet<&Path> = self
            .targets
            .iter()
            .map(|target| target.stored_path.as_path())
            .collect();
        let mut found = Vec::new();
        collect_files(&self.root, &layout::files_dir(&self.root), &mut found);
        found.retain(|file| !recorded.contains(file.as_path()));
        found.sort();
        found
    }
}

enum Condition {
    Intact,
    Missing,
    Damaged,
}

fn issue_of(target: &HealthTarget) -> HealthIssue {
    HealthIssue {
        id: target.id,
        display_name: target.display_name.clone(),
        collection: target.collection.clone(),
    }
}

fn collect_files(root: &Path, folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if layout::is_unfinished_import(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(root, &path, found);
        } else if let Ok(relative) = path.strip_prefix(root) {
            found.push(relative.to_path_buf());
        }
    }
}
