use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::export::{
    base_name_of, create_free_file, export_base_name, free_name, original_extension,
};
use super::layout::library_display_name;
use super::{Asset, AssetView, Collection, CollectionId, Library, LibraryError};

const OUTLINE_DEPTH: usize = 2;
const OUTLINE_LINES: usize = 5;
const LIBRARY_FALLBACK: &str = "Library";
const UNCLASSIFIED_FALLBACK: &str = "Unclassified";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExportPlan {
    root_folder: FolderPlan,
    total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderPlan {
    name: String,
    files: Vec<FilePlan>,
    folders: Vec<FolderPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FilePlan {
    display_name: String,
    base_name: String,
    extension: Option<String>,
    source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineFolder {
    pub depth: usize,
    pub name: String,
    pub files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutline {
    pub root: String,
    pub folders: Vec<OutlineFolder>,
    pub more: usize,
}

#[derive(Debug)]
pub struct ExportFailure {
    pub name: String,
    pub error: LibraryError,
}

#[derive(Debug, Default)]
pub struct LibraryExportReport {
    pub folder: PathBuf,
    pub exported: usize,
    pub failures: Vec<ExportFailure>,
}

impl Library {
    pub fn library_export_plan(
        &self,
        unclassified_name: &str,
    ) -> Result<LibraryExportPlan, LibraryError> {
        let collections = self.visible_collections()?;
        let mut folders = collections
            .iter()
            .filter(|collection| collection.parent.is_none())
            .map(|collection| self.folder_plan(collection, &collections))
            .collect::<Result<Vec<_>, _>>()?;
        let unclassified = self.visible_assets_in(AssetView::Unclassified)?;
        if !unclassified.is_empty() {
            folders.push(FolderPlan {
                name: export_base_name(unclassified_name, None)
                    .unwrap_or_else(|| UNCLASSIFIED_FALLBACK.to_owned()),
                files: unclassified
                    .iter()
                    .map(|asset| self.file_plan(asset))
                    .collect(),
                folders: Vec::new(),
            });
        }
        let root_folder = FolderPlan {
            name: export_base_name(&library_display_name(&self.root), None)
                .unwrap_or_else(|| LIBRARY_FALLBACK.to_owned()),
            files: Vec::new(),
            folders,
        };
        let total = root_folder.file_count();
        Ok(LibraryExportPlan { root_folder, total })
    }

    pub fn export_outline(&self, unclassified_name: &str) -> Result<ExportOutline, LibraryError> {
        let collections = self.visible_collections()?;
        let own_files = self.files_per_collection()?;
        let mut folders = Vec::new();
        for top in collections.iter().filter(|c| c.parent.is_none()) {
            outline_of(top, 1, &collections, &own_files, &mut folders);
        }
        let unclassified = self.view_count(AssetView::Unclassified)?;
        let reserved = usize::from(unclassified > 0);
        let kept_lines = OUTLINE_LINES - reserved;
        let more = folders.len().saturating_sub(kept_lines);
        folders.truncate(kept_lines);
        if unclassified > 0 {
            folders.push(OutlineFolder {
                depth: 1,
                name: export_base_name(unclassified_name, None)
                    .unwrap_or_else(|| UNCLASSIFIED_FALLBACK.to_owned()),
                files: unclassified,
            });
        }
        Ok(ExportOutline {
            root: export_base_name(&library_display_name(&self.root), None)
                .unwrap_or_else(|| LIBRARY_FALLBACK.to_owned()),
            folders,
            more,
        })
    }

    fn files_per_collection(&self) -> Result<HashMap<CollectionId, usize>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT asset_collections.collection_id, count(*) FROM asset_collections
             JOIN assets ON assets.id = asset_collections.asset_id
             WHERE assets.trashed_at_unix_ms IS NULL
             GROUP BY asset_collections.collection_id",
        )?;
        let counts = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                ))
            })?
            .collect::<Result<_, _>>()?;
        Ok(counts)
    }

    fn folder_plan(
        &self,
        collection: &Collection,
        all: &[Collection],
    ) -> Result<FolderPlan, LibraryError> {
        let folders = all
            .iter()
            .filter(|child| child.parent == Some(collection.id))
            .map(|child| self.folder_plan(child, all))
            .collect::<Result<_, _>>()?;
        Ok(FolderPlan {
            name: export_base_name(&collection.name, None)
                .unwrap_or_else(|| collection.id.to_string()),
            files: self.files_directly_in(collection.id)?,
            folders,
        })
    }

    fn files_directly_in(&self, collection: CollectionId) -> Result<Vec<FilePlan>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT assets.id FROM asset_collections
             JOIN assets ON assets.id = asset_collections.asset_id
             WHERE asset_collections.collection_id = ?1 AND assets.trashed_at_unix_ms IS NULL
             ORDER BY assets.added_at_unix_ms, assets.id",
        )?;
        let ids = statement
            .query_map([collection], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut files = Vec::new();
        for id in ids {
            if let Some(asset) = self.asset(id)? {
                files.push(self.file_plan(&asset));
            }
        }
        Ok(files)
    }

    fn file_plan(&self, asset: &Asset) -> FilePlan {
        FilePlan {
            display_name: asset.display_name.clone(),
            base_name: base_name_of(asset),
            extension: original_extension(asset),
            source: self.file_of(asset),
        }
    }
}

impl LibraryExportPlan {
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    pub fn run(&self, destination: &Path) -> Result<LibraryExportReport, LibraryError> {
        if !destination.is_dir() {
            return Err(LibraryError::NotFound(destination.to_path_buf()));
        }
        let folder = create_free_folder(destination, &self.root_folder.name, &mut HashSet::new())?;
        let mut report = LibraryExportReport {
            folder: folder.clone(),
            ..LibraryExportReport::default()
        };
        write_folder(&self.root_folder, &folder, &mut report)?;
        Ok(report)
    }
}

impl FolderPlan {
    fn file_count(&self) -> usize {
        self.files.len()
            + self
                .folders
                .iter()
                .map(FolderPlan::file_count)
                .sum::<usize>()
    }
}

fn write_folder(
    plan: &FolderPlan,
    path: &Path,
    report: &mut LibraryExportReport,
) -> Result<(), LibraryError> {
    let mut taken = HashSet::new();
    for file in &plan.files {
        match copy_file(file, path, &mut taken) {
            Ok(()) => report.exported += 1,
            Err(error) => report.failures.push(ExportFailure {
                name: file.display_name.clone(),
                error: error.into(),
            }),
        }
    }
    for sub_plan in &plan.folders {
        let sub_path = create_free_folder(path, &sub_plan.name, &mut taken)?;
        write_folder(sub_plan, &sub_path, report)?;
    }
    Ok(())
}

fn copy_file(file: &FilePlan, folder: &Path, taken: &mut HashSet<String>) -> io::Result<()> {
    let (mut target, copy) =
        create_free_file(folder, &file.base_name, file.extension.as_deref(), taken)?;
    let copied = File::open(&file.source).and_then(|mut source| io::copy(&mut source, &mut target));
    if copied.is_err() {
        let _ = fs::remove_file(&copy);
    }
    copied.map(|_| ())
}

fn create_free_folder(
    parent: &Path,
    base: &str,
    taken: &mut HashSet<String>,
) -> io::Result<PathBuf> {
    loop {
        let path = parent.join(free_name(base, None, taken));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

fn outline_of(
    collection: &Collection,
    depth: usize,
    all: &[Collection],
    own_files: &HashMap<CollectionId, usize>,
    folders: &mut Vec<OutlineFolder>,
) {
    folders.push(OutlineFolder {
        depth,
        name: export_base_name(&collection.name, None).unwrap_or_else(|| collection.id.to_string()),
        files: files_below(collection, all, own_files),
    });
    if depth < OUTLINE_DEPTH {
        for child in all
            .iter()
            .filter(|child| child.parent == Some(collection.id))
        {
            outline_of(child, depth + 1, all, own_files, folders);
        }
    }
}

fn files_below(
    collection: &Collection,
    all: &[Collection],
    own_files: &HashMap<CollectionId, usize>,
) -> usize {
    own_files.get(&collection.id).copied().unwrap_or(0)
        + all
            .iter()
            .filter(|child| child.parent == Some(collection.id))
            .map(|child| files_below(child, all, own_files))
            .sum::<usize>()
}
