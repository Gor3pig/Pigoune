use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::export_lifetime::{self, SpaceProbe};
use super::live_transfer;
use super::{Asset, AssetId, Library, LibraryError, layout};

#[derive(Debug)]
pub struct CopyFailure {
    pub asset: AssetId,
    pub display_name: String,
    pub error: io::Error,
}

#[derive(Debug)]
pub struct CopyReport {
    pub copies: Vec<PathBuf>,
    pub failures: Vec<CopyFailure>,
}

const MAX_NAME_BYTES: usize = 200;
const FORBIDDEN_CHARACTERS: [char; 2] = ['/', '\0'];
const REPLACEMENT: char = '-';
const MAX_EXTENSION_CHARS: usize = 5;
const DEFAULT_EXTENSION: &str = "png";
pub const FLAT_EXTENSION: &str = "png";

pub type FlatFiles = std::collections::HashMap<AssetId, PathBuf>;

pub fn flat_unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        "the annotated image could not be rendered",
    )
}

impl Library {
    pub fn export_copies(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        self.export_copies_checking(assets, flats, &|_| None)
    }

    pub fn export_copies_checking(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
        space: SpaceProbe<'_>,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        self.export_operation(assets, flats, now_ms(), space)
    }

    pub(super) fn export_operation(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
        now_ms: u64,
        space: SpaceProbe<'_>,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        run_operation(
            &layout::export_dir(&self.root),
            &self.root,
            &live_transfer::is_protected,
            || self.transfer_items(assets, flats),
            now_ms,
            space,
        )
    }

    fn transfer_items(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
    ) -> Result<Vec<ExportItem>, LibraryError> {
        let annotated = self.annotated_ids()?;
        let mut items = Vec::new();
        for id in assets {
            let Some(asset) = self.asset(*id)? else {
                continue;
            };
            let (source, extension) = if annotated.contains(id) {
                let Some(flat) = flats.get(id) else {
                    return Err(flat_unavailable().into());
                };
                (flat.clone(), Some(FLAT_EXTENSION.to_owned()))
            } else {
                (self.file_of(&asset), original_extension(&asset))
            };
            let bytes = fs::metadata(&source)?.len();
            items.push(ExportItem {
                source,
                base: base_name_of(&asset),
                extension,
                bytes,
            });
        }
        Ok(items)
    }

    pub fn clipboard_copies(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        self.clipboard_copies_checking(assets, flats, &|_| None)
    }

    pub fn clipboard_copies_checking(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
        space: SpaceProbe<'_>,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        self.clipboard_operation(assets, flats, now_ms(), space)
    }

    pub(super) fn clipboard_operation(
        &self,
        assets: &[AssetId],
        flats: &FlatFiles,
        now_ms: u64,
        space: SpaceProbe<'_>,
    ) -> Result<Vec<PathBuf>, LibraryError> {
        let items = self.transfer_items(assets, flats)?;
        if items.is_empty() {
            return Ok(Vec::new());
        }
        run_operation(
            &layout::clipboard_dir(&self.root),
            &self.root,
            &live_transfer::is_protected,
            || Ok(items),
            now_ms,
            space,
        )
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

    pub fn opening_copy(
        &self,
        asset: AssetId,
        flats: &FlatFiles,
    ) -> Result<Option<PathBuf>, LibraryError> {
        self.opening_copy_checking(asset, flats, &|_| None)
    }

    pub fn opening_copy_checking(
        &self,
        asset: AssetId,
        flats: &FlatFiles,
        space: SpaceProbe<'_>,
    ) -> Result<Option<PathBuf>, LibraryError> {
        self.opening_operation(asset, flats, now_ms(), space)
    }

    pub(super) fn opening_operation(
        &self,
        asset: AssetId,
        flats: &FlatFiles,
        now_ms: u64,
        space: SpaceProbe<'_>,
    ) -> Result<Option<PathBuf>, LibraryError> {
        let items = self.transfer_items(&[asset], flats)?;
        if items.is_empty() {
            return Ok(None);
        }
        let opening = layout::opening_dir(&self.root);
        export_lifetime::secure_root(&opening)?;
        let copies = run_operation(
            &opening.join(asset.to_string()),
            &self.root,
            &live_transfer::is_protected,
            || Ok(items),
            now_ms,
            space,
        )?;
        Ok(copies.into_iter().next())
    }

    pub fn export_to(
        &self,
        assets: &[AssetId],
        folder: &Path,
        flats: &FlatFiles,
    ) -> Result<CopyReport, LibraryError> {
        if !folder.is_dir() {
            return Err(LibraryError::NotFound(folder.to_path_buf()));
        }
        self.copy_into(assets, folder, flats)
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

    fn copy_into(
        &self,
        assets: &[AssetId],
        folder: &Path,
        flats: &FlatFiles,
    ) -> Result<CopyReport, LibraryError> {
        let annotated = self.annotated_ids()?;
        let mut taken = HashSet::new();
        let mut copies = Vec::new();
        let mut failures = Vec::new();
        for id in assets {
            let Some(asset) = self.asset(*id)? else {
                continue;
            };
            let (path, extension) = if annotated.contains(id) {
                let Some(flat) = flats.get(id) else {
                    failures.push(failure_of(&asset, flat_unavailable()));
                    continue;
                };
                (flat.clone(), Some(FLAT_EXTENSION.to_owned()))
            } else {
                (self.file_of(&asset), original_extension(&asset))
            };
            let mut source = match File::open(path) {
                Ok(source) => source,
                Err(error) => {
                    failures.push(failure_of(&asset, error));
                    continue;
                }
            };
            let (mut file, copy) = create_free_file(
                folder,
                &base_name_of(&asset),
                extension.as_deref(),
                &mut taken,
            )?;
            match io::copy(&mut source, &mut file) {
                Ok(_) => copies.push(copy),
                Err(error) => {
                    drop(file);
                    let _ = fs::remove_file(&copy);
                    failures.push(failure_of(&asset, error));
                }
            }
        }
        Ok(CopyReport { copies, failures })
    }
}

fn failure_of(asset: &Asset, error: io::Error) -> CopyFailure {
    CopyFailure {
        asset: asset.id,
        display_name: asset.display_name.clone(),
        error,
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

struct ExportItem {
    source: PathBuf,
    base: String,
    extension: Option<String>,
    bytes: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0))
}

fn random_part() -> u64 {
    Uuid::now_v7().as_u64_pair().1
}

pub(super) fn transfer_budget_roots(library_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        layout::export_dir(library_root),
        layout::clipboard_dir(library_root),
    ];
    if let Ok(children) = fs::read_dir(layout::opening_dir(library_root)) {
        roots.extend(children.flatten().filter_map(|child| {
            let name = child.file_name();
            let is_folder = child.file_type().is_ok_and(|kind| kind.is_dir());
            (is_folder && AssetId::parse(&name.to_string_lossy()).is_some()).then(|| child.path())
        }));
    }
    roots
}

fn run_operation(
    root: &Path,
    library_root: &Path,
    protected: &dyn Fn(&Path) -> bool,
    items_for: impl FnOnce() -> Result<Vec<ExportItem>, LibraryError>,
    now_ms: u64,
    space: SpaceProbe<'_>,
) -> Result<Vec<PathBuf>, LibraryError> {
    export_lifetime::secure_root(root)?;
    export_lifetime::remove_unfinished(root);
    let items = items_for()?;
    let needed = items.iter().map(|item| item.bytes).sum();
    let budget_roots = transfer_budget_roots(library_root);
    let budget_roots: Vec<&Path> = budget_roots.iter().map(PathBuf::as_path).collect();
    export_lifetime::make_room_across(&budget_roots, now_ms, needed, protected, &mut || {
        space(root)
    })
    .map_err(|_| LibraryError::StorageFull)?;
    let operation = loop {
        let name = export_lifetime::operation_name(now_ms, random_part());
        match export_lifetime::create_staging(root, &name) {
            Ok(_) => break name,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    };
    let staging = root.join(export_lifetime::staging_name(&operation));
    let outcome = fill_operation(&items, &staging).and_then(|(names, bytes)| {
        export_lifetime::publish(root, &operation, bytes)?;
        Ok(names)
    });
    match outcome {
        Ok(names) => {
            let published = root.join(&operation);
            Ok(names.iter().map(|name| published.join(name)).collect())
        }
        Err(error) => {
            export_lifetime::discard_staging(root, &operation);
            Err(error)
        }
    }
}

fn fill_operation(
    items: &[ExportItem],
    staging: &Path,
) -> Result<(Vec<String>, u64), LibraryError> {
    let mut taken = HashSet::new();
    let mut names = Vec::with_capacity(items.len());
    let mut bytes = 0;
    for item in items {
        let mut source = File::open(&item.source)?;
        let (mut file, copy) =
            create_free_file(staging, &item.base, item.extension.as_deref(), &mut taken)?;
        bytes += io::copy(&mut source, &mut file)?;
        if let Some(name) = copy.file_name() {
            names.push(name.to_string_lossy().into_owned());
        }
    }
    Ok((names, bytes))
}

pub fn forget_exports(root: &Path) {
    let now = now_ms();
    export_lifetime::tidy(&layout::export_dir(root), now);
    export_lifetime::tidy_except(
        &layout::clipboard_dir(root),
        now,
        &live_transfer::is_protected,
    );
    export_lifetime::tidy_children(&layout::opening_dir(root), now, &|name| {
        AssetId::parse(name).is_some()
    });
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

#[cfg(test)]
mod operations {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use super::super::export_lifetime::VolumeSpace;
    use super::super::{AssetId, ImportOutcome, Library, LibraryError};
    use super::FlatFiles;

    const HOUR: u64 = 60 * 60 * 1000;
    const GIB: u64 = 1024 * 1024 * 1024;
    const NOW: u64 = 5_000 * HOUR;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn library() -> (tempfile::TempDir, Library) {
        let workspace = tempfile::tempdir().expect("workspace");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        (workspace, library)
    }

    fn import(library: &mut Library, name: &str) -> AssetId {
        match library.import_file(&fixture(name), None).expect("import") {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "it stands in for a probe that may not know the free space"
    )]
    fn roomy(_: &Path) -> Option<VolumeSpace> {
        Some(VolumeSpace {
            free: 500 * GIB,
            total: 1000 * GIB,
        })
    }

    fn entries(library: &Library) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(super::layout::export_dir(library.root()))
            .expect("export folder")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_next_export_removes_what_is_older_than_a_day_and_keeps_the_rest() {
        let (_workspace, mut library) = library();
        let png = import(&mut library, "red-dot.png");
        let old = library
            .export_operation(&[png], &FlatFiles::new(), NOW - 30 * HOUR, &roomy)
            .expect("old");
        let middle = library
            .export_operation(&[png], &FlatFiles::new(), NOW - 5 * HOUR, &roomy)
            .expect("middle");
        let recent = library
            .export_operation(&[png], &FlatFiles::new(), NOW - HOUR, &roomy)
            .expect("recent");

        let current = library
            .export_operation(&[png], &FlatFiles::new(), NOW, &roomy)
            .expect("current");

        assert!(!old[0].exists());
        assert!(middle[0].exists() && recent[0].exists() && current[0].exists());
    }

    #[test]
    fn an_export_that_does_not_fit_fails_without_removing_anything_recent() {
        let (_workspace, mut library) = library();
        let png = import(&mut library, "red-dot.png");
        let recent = library
            .export_operation(&[png], &FlatFiles::new(), NOW - HOUR, &roomy)
            .expect("recent");
        let before = entries(&library);
        let cramped = |_: &Path| {
            Some(VolumeSpace {
                free: GIB / 2,
                total: 100 * GIB,
            })
        };

        let result = library.export_operation(&[png], &FlatFiles::new(), NOW, &cramped);

        assert!(matches!(result, Err(LibraryError::StorageFull)));
        assert!(recent[0].exists());
        assert_eq!(entries(&library), before);
    }

    #[test]
    fn a_short_volume_frees_the_oldest_normal_export_and_then_succeeds() {
        let (_workspace, mut library) = library();
        let png = import(&mut library, "red-dot.png");
        let normal = library
            .export_operation(&[png], &FlatFiles::new(), NOW - 10 * HOUR, &roomy)
            .expect("normal");
        let recent = library
            .export_operation(&[png], &FlatFiles::new(), NOW - HOUR, &roomy)
            .expect("recent");
        let watched = normal[0].clone();
        let space = move |_: &Path| {
            Some(VolumeSpace {
                free: if watched.exists() { GIB / 2 } else { 500 * GIB },
                total: 100 * GIB,
            })
        };

        let current = library
            .export_operation(&[png], &FlatFiles::new(), NOW, &space)
            .expect("there is room once the old export is gone");

        assert!(!normal[0].exists());
        assert!(recent[0].exists() && current[0].exists());
    }

    #[test]
    fn the_size_of_a_flattened_copy_is_counted_not_the_size_of_the_original() {
        let (workspace, mut library) = library();
        let png = import(&mut library, "red-dot.png");
        let shape = crate::annotations::Shape::new(
            "rectangle",
            crate::annotations::Geometry::Box(crate::annotations::Rect {
                x: 0.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            }),
            crate::annotations::Options::new(),
        );
        library
            .save_annotations(
                png,
                &crate::annotations::AnnotationDocument::new(
                    None,
                    vec![crate::annotations::AnnotationObject::new(1, shape)],
                ),
            )
            .expect("annotated");
        let flat = workspace.path().join("flat.png");
        fs::write(&flat, vec![0_u8; 5 * 1024 * 1024]).expect("large flattened copy");
        let mut flats = FlatFiles::new();
        flats.insert(png, flat);
        let tight = |_: &Path| {
            Some(VolumeSpace {
                free: GIB + 1024 * 1024,
                total: 10 * GIB,
            })
        };

        let refused = library.export_operation(&[png], &flats, NOW, &tight);

        assert!(matches!(refused, Err(LibraryError::StorageFull)));
        assert!(entries(&library).is_empty());
    }

    #[test]
    fn a_copy_that_fails_halfway_removes_only_the_new_operation() {
        let (_workspace, mut library) = library();
        let svg = import(&mut library, "dark-circle.svg");
        let png = import(&mut library, "red-dot.png");
        let previous = library
            .export_operation(&[svg], &FlatFiles::new(), NOW - HOUR, &roomy)
            .expect("previous");
        let before = entries(&library);
        let asset = library.asset(png).expect("read").expect("exists");
        let source = library.file_of(&asset);
        fs::set_permissions(&source, fs::Permissions::from_mode(0o000)).expect("locked");

        let result = library.export_operation(&[svg, png], &FlatFiles::new(), NOW, &roomy);

        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).expect("unlocked");
        assert!(result.is_err());
        assert!(previous[0].exists());
        assert_eq!(entries(&library), before);
    }

    #[test]
    fn publishing_is_complete_before_the_paths_are_given() {
        let (_workspace, mut library) = library();
        let svg = import(&mut library, "dark-circle.svg");
        let png = import(&mut library, "red-dot.png");

        let copies = library
            .export_operation(&[svg, png], &FlatFiles::new(), NOW, &roomy)
            .expect("copies");

        for copy in &copies {
            assert!(copy.is_file());
            assert!(!copy.to_string_lossy().contains(".partial"));
        }
        assert!(
            entries(&library)
                .iter()
                .all(|name| !name.ends_with(".partial"))
        );
    }
}
