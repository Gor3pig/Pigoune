use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub source: PathBuf,
    pub folders: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportPlan {
    pub files: Vec<PlannedFile>,
    pub unreadable: Vec<PathBuf>,
}

impl ImportPlan {
    pub fn of(paths: &[PathBuf]) -> Self {
        let mut plan = Self::default();
        for path in paths {
            match fs::metadata(path) {
                Ok(metadata) if metadata.is_dir() => {
                    plan.add_folder(path, &folder_chain(&[], root_folder_name(path)));
                }
                Ok(_) => plan.files.push(PlannedFile {
                    source: path.clone(),
                    folders: Vec::new(),
                }),
                Err(_) => plan.unreadable.push(path.clone()),
            }
        }
        plan
    }

    fn add_folder(&mut self, folder: &Path, folders: &[String]) {
        let Ok(entries) = fs::read_dir(folder) else {
            self.unreadable.push(folder.to_path_buf());
            return;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(fs::DirEntry::file_name);

        for entry in entries {
            let name = lossy(&entry.file_name());
            if is_hidden(&name) {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    self.add_folder(&entry.path(), &folder_chain(folders, Some(name)));
                }
                Ok(kind) if kind.is_file() => self.files.push(PlannedFile {
                    source: entry.path(),
                    folders: folders.to_vec(),
                }),
                Ok(_) => {}
                Err(_) => self.unreadable.push(entry.path()),
            }
        }
    }
}

fn root_folder_name(folder: &Path) -> Option<String> {
    folder
        .file_name()
        .map(lossy)
        .or_else(|| fs::canonicalize(folder).ok()?.file_name().map(lossy))
}

fn lossy(name: &std::ffi::OsStr) -> String {
    name.to_string_lossy().into_owned()
}

fn folder_chain(parents: &[String], name: Option<String>) -> Vec<String> {
    let mut chain = parents.to_vec();
    chain.extend(name.filter(|name| !name.trim().is_empty()));
    chain
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}
