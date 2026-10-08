use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{Asset, AssetId, Library, LibraryError, layout};

const MAX_NAME_BYTES: usize = 200;
const FORBIDDEN_CHARACTERS: [char; 2] = ['/', '\0'];
const REPLACEMENT: char = '-';
const MAX_EXTENSION_CHARS: usize = 5;
const DEFAULT_EXTENSION: &str = "png";

impl Library {
    pub fn export_copies(&self, assets: &[AssetId]) -> Result<Vec<PathBuf>, LibraryError> {
        self.fresh_copies(assets, &layout::export_dir(&self.root))
    }

    pub fn clipboard_copies(&self, assets: &[AssetId]) -> Result<Vec<PathBuf>, LibraryError> {
        self.fresh_copies(assets, &layout::clipboard_dir(&self.root))
    }

    pub fn save_pasted_image(
        &self,
        name: &str,
        extension: &str,
        contents: &[u8],
    ) -> Result<PathBuf, LibraryError> {
        let folder = layout::pasted_dir(&self.root);
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder)?;
        let extension = safe_extension(extension);
        let base = export_base_name(name, Some(&extension)).unwrap_or_else(|| name.to_owned());
        let (mut file, path) =
            create_free_file(&folder, &base, Some(&extension), &mut HashSet::new())?;
        file.write_all(contents)?;
        Ok(path)
    }

    pub fn opening_copy(&self, asset: AssetId) -> Result<Option<PathBuf>, LibraryError> {
        let folder = layout::opening_dir(&self.root).join(asset.to_string());
        Ok(self.fresh_copies(&[asset], &folder)?.pop())
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

    pub fn save_converted(
        asset: &Asset,
        folder: &Path,
        name_suffix: Option<&str>,
        extension: &str,
        contents: &[u8],
    ) -> Result<PathBuf, LibraryError> {
        if !folder.is_dir() {
            return Err(LibraryError::NotFound(folder.to_path_buf()));
        }
        let base = match name_suffix {
            Some(suffix) => format!("{}-{suffix}", base_name_of(asset)),
            None => base_name_of(asset),
        };
        let (mut file, path) =
            create_free_file(folder, &base, Some(extension), &mut HashSet::new())?;
        file.write_all(contents)?;
        Ok(path)
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
            let extension = original_extension(&asset);
            let (mut file, copy) = create_free_file(
                folder,
                &base_name_of(&asset),
                extension.as_deref(),
                &mut taken,
            )?;
            io::copy(&mut File::open(self.file_of(&asset))?, &mut file)?;
            copies.push(copy);
        }
        Ok(copies)
    }
}

pub(super) fn original_extension(asset: &Asset) -> Option<String> {
    Path::new(&asset.original_file_name)
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
}

pub(super) fn base_name_of(asset: &Asset) -> String {
    export_base_name(&asset.display_name, original_extension(asset).as_deref())
        .unwrap_or_else(|| asset.id.to_string())
}

pub(super) fn create_free_file(
    folder: &Path,
    base: &str,
    extension: Option<&str>,
    taken: &mut HashSet<String>,
) -> io::Result<(File, PathBuf)> {
    loop {
        let path = folder.join(free_name(base, extension, taken));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

pub fn forget_exports(root: &Path) {
    let _ = fs::remove_dir_all(layout::export_dir(root));
    let _ = fs::remove_dir_all(layout::clipboard_dir(root));
    let _ = fs::remove_dir_all(layout::opening_dir(root));
    let _ = fs::remove_dir_all(layout::pasted_dir(root));
}

fn safe_extension(extension: &str) -> String {
    let kept: String = extension
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(MAX_EXTENSION_CHARS)
        .collect::<String>()
        .to_lowercase();
    if kept.is_empty() {
        DEFAULT_EXTENSION.to_owned()
    } else {
        kept
    }
}

pub(super) fn export_base_name(display_name: &str, extension: Option<&str>) -> Option<String> {
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

pub(super) fn free_name(
    base: &str,
    extension: Option<&str>,
    taken: &mut HashSet<String>,
) -> String {
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
