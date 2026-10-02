use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::{AssetId, Library, LibraryError, layout};

const MAX_NAME_BYTES: usize = 200;
const FORBIDDEN_CHARACTERS: [char; 2] = ['/', '\0'];
const REPLACEMENT: char = '-';

impl Library {
    pub fn export_copies(&self, assets: &[AssetId]) -> Result<Vec<PathBuf>, LibraryError> {
        self.fresh_copies(assets, &layout::export_dir(&self.root))
    }

    pub fn clipboard_copies(&self, assets: &[AssetId]) -> Result<Vec<PathBuf>, LibraryError> {
        self.fresh_copies(assets, &layout::clipboard_dir(&self.root))
    }

    pub fn export_to(
        &self,
        assets: &[AssetId],
        folder: &Path,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        if !folder.is_dir() {
            return Err(LibraryError::NotFound(folder.to_path_buf()));
        }
        self.copy_into(assets, folder)
    }

    fn fresh_copies(
        &self,
        assets: &[AssetId],
        folder: &Path,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        let _ = fs::remove_dir_all(folder);
        fs::create_dir_all(folder)?;
        self.copy_into(assets, folder)
    }

    fn copy_into(&self, assets: &[AssetId], folder: &Path) -> Result<Vec<PathBuf>, LibraryError> {
        let mut taken = HashSet::new();
        let mut copies = Vec::new();
        for id in assets {
            let Some(asset) = self.asset(*id)? else {
                continue;
            };
            let extension = Path::new(&asset.original_file_name)
                .extension()
                .map(|extension| extension.to_string_lossy().into_owned());
            let base = export_base_name(&asset.display_name, extension.as_deref())
                .unwrap_or_else(|| id.to_string());
            let copy = loop {
                let name = free_name(&base, extension.as_deref(), &mut taken);
                let candidate = folder.join(name);
                if candidate.symlink_metadata().is_err() {
                    break candidate;
                }
            };
            copy_without_overwriting(&self.file_of(&asset), &copy)?;
            copies.push(copy);
        }
        Ok(copies)
    }
}

fn copy_without_overwriting(source: &Path, destination: &Path) -> Result<(), LibraryError> {
    let mut reader = fs::File::open(source)?;
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut reader, &mut writer)?;
    Ok(())
}

pub fn forget_exports(root: &Path) {
    let _ = fs::remove_dir_all(layout::export_dir(root));
    let _ = fs::remove_dir_all(layout::clipboard_dir(root));
}

fn export_base_name(display_name: &str, extension: Option<&str>) -> Option<String> {
    let cleaned: String = display_name
        .trim()
        .chars()
        .map(|character| {
            if FORBIDDEN_CHARACTERS.contains(&character) || character.is_control() {
                REPLACEMENT
            } else {
                character
            }
        })
        .collect();
    let without_extension = match extension {
        Some(extension) => strip_extension(&cleaned, extension),
        None => &cleaned,
    };
    let visible = without_extension.trim().trim_start_matches('.').trim();
    (!visible.is_empty()).then(|| truncated(visible).to_owned())
}

fn strip_extension<'a>(name: &'a str, extension: &str) -> &'a str {
    let suffix = format!(".{}", extension.to_lowercase());
    if name.to_lowercase().ends_with(&suffix) && name.len() > suffix.len() {
        &name[..name.len() - suffix.len()]
    } else {
        name
    }
}

fn truncated(name: &str) -> &str {
    if name.len() <= MAX_NAME_BYTES {
        return name;
    }
    let mut end = MAX_NAME_BYTES;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

fn free_name(base: &str, extension: Option<&str>, taken: &mut HashSet<String>) -> String {
    let with_extension = |stem: &str| match extension {
        Some(extension) => format!("{stem}.{extension}"),
        None => stem.to_owned(),
    };
    let mut name = with_extension(base);
    let mut rank = 2;
    while !taken.insert(name.to_lowercase()) {
        name = with_extension(&format!("{base} ({rank})"));
        rank += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{export_base_name, free_name};

    #[test]
    fn the_display_name_becomes_a_safe_file_name() {
        assert_eq!(
            export_base_name("  Logo / GitHub  ", Some("svg")).as_deref(),
            Some("Logo - GitHub")
        );
        assert_eq!(
            export_base_name(".cache", Some("png")).as_deref(),
            Some("cache")
        );
        assert_eq!(export_base_name("   ", Some("png")), None);
    }

    #[test]
    fn an_extension_already_typed_is_not_doubled() {
        assert_eq!(
            export_base_name("logo.SVG", Some("svg")).as_deref(),
            Some("logo")
        );
    }

    #[test]
    fn identical_names_are_numbered() {
        let mut taken = HashSet::new();
        assert_eq!(free_name("Logo", Some("svg"), &mut taken), "Logo.svg");
        assert_eq!(free_name("logo", Some("svg"), &mut taken), "logo (2).svg");
        assert_eq!(free_name("Logo", Some("svg"), &mut taken), "Logo (3).svg");
    }
}
