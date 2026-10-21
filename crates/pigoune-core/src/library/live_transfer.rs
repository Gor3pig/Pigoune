use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::export_lifetime::{self, Entry, EntryKind};
use super::{Library, LibraryError, layout};

struct Slots {
    clipboard_root: PathBuf,
    live: Option<PathBuf>,
    pending: Vec<PathBuf>,
}

static REGISTRY: Mutex<Vec<Slots>> = Mutex::new(Vec::new());

fn registry() -> MutexGuard<'static, Vec<Slots>> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn slots_of<'a>(all: &'a mut Vec<Slots>, clipboard_root: &Path) -> &'a mut Slots {
    let position = all
        .iter()
        .position(|slots| slots.clipboard_root == clipboard_root)
        .unwrap_or_else(|| {
            all.push(Slots {
                clipboard_root: clipboard_root.to_path_buf(),
                live: None,
                pending: Vec::new(),
            });
            all.len() - 1
        });
    &mut all[position]
}

pub(super) fn is_protected(path: &Path) -> bool {
    let wanted = canonical(path);
    registry().iter().any(|slots| {
        slots.live.as_deref() == Some(wanted.as_path()) || slots.pending.contains(&wanted)
    })
}

#[must_use = "an installation that is neither committed nor abandoned keeps its folders protected"]
#[derive(Debug)]
pub struct ClipboardInstall {
    clipboard_root: PathBuf,
    operation: PathBuf,
}

impl ClipboardInstall {
    pub fn commit(self) {
        let mut all = registry();
        let slots = slots_of(&mut all, &self.clipboard_root);
        slots.live = Some(self.operation);
        slots.pending.clear();
    }

    pub fn abandon(self) {
        {
            let mut all = registry();
            let slots = slots_of(&mut all, &self.clipboard_root);
            slots.pending.retain(|pending| *pending != self.operation);
        }
        if let Some(name) = self.operation.file_name() {
            let entry = Entry {
                name: name.to_string_lossy().into_owned(),
                kind: EntryKind::Operation,
                started_ms: 0,
                bytes: 0,
            };
            let _ = export_lifetime::remove_entry(&self.clipboard_root, &entry);
        }
    }
}

impl Library {
    pub fn begin_clipboard_install(
        &self,
        copies: &[PathBuf],
    ) -> Result<ClipboardInstall, LibraryError> {
        let refused = || LibraryError::NotFound(copies.first().cloned().unwrap_or_default());
        let clipboard_root = layout::clipboard_dir(&self.root);
        let operation = operation_folder(&clipboard_root, copies).ok_or_else(refused)?;
        let clipboard_root = canonical(&clipboard_root);
        let operation = clipboard_root.join(operation.file_name().ok_or_else(refused)?);
        let mut all = registry();
        slots_of(&mut all, &clipboard_root)
            .pending
            .push(operation.clone());
        Ok(ClipboardInstall {
            clipboard_root,
            operation,
        })
    }
}

fn operation_folder(clipboard_root: &Path, copies: &[PathBuf]) -> Option<PathBuf> {
    let folder = copies.first()?.parent()?;
    let shares_folder = copies.iter().all(|copy| copy.parent() == Some(folder));
    let is_operation = folder.parent() == Some(clipboard_root)
        && folder
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| export_lifetime::started_at(name).is_some());
    let is_real_folder = fs::symlink_metadata(folder).is_ok_and(|metadata| metadata.is_dir());
    (shares_folder && is_operation && is_real_folder).then(|| folder.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};

    use super::super::export_lifetime::{self, VolumeSpace};
    use super::super::{AssetId, FlatFiles, ImportOutcome, Library, LibraryError, layout};
    use super::{REGISTRY, is_protected};

    const HOUR: u64 = 60 * 60 * 1000;
    const GIB: u64 = 1024 * 1024 * 1024;
    const NOW: u64 = 5_000 * HOUR;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn library() -> (tempfile::TempDir, Library, AssetId) {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut library = Library::create(workspace.path(), "Essai").expect("library");
        let ImportOutcome::Imported(png) = library
            .import_file(&fixture("red-dot.png"), None)
            .expect("import")
        else {
            panic!("the image is new");
        };
        (workspace, library, png)
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

    #[expect(
        clippy::unnecessary_wraps,
        reason = "it stands in for a probe that may not know the free space"
    )]
    fn cramped(_: &Path) -> Option<VolumeSpace> {
        Some(VolumeSpace {
            free: GIB / 2,
            total: 100 * GIB,
        })
    }

    fn copy_at(library: &Library, asset: AssetId, now_ms: u64) -> Vec<PathBuf> {
        library
            .clipboard_operation(&[asset], &FlatFiles::new(), now_ms, &roomy)
            .expect("copy is prepared")
    }

    fn install(library: &Library, copies: &[PathBuf]) {
        library
            .begin_clipboard_install(copies)
            .expect("installation begins")
            .commit();
    }

    fn tidy_at(library: &Library, now_ms: u64) {
        export_lifetime::tidy_except(
            &layout::clipboard_dir(library.root()),
            now_ms,
            &is_protected,
        );
    }

    fn folder_of(copies: &[PathBuf]) -> PathBuf {
        copies[0].parent().expect("operation folder").to_path_buf()
    }

    fn annotate(library: &mut Library, asset: AssetId) {
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
                asset,
                &crate::annotations::AnnotationDocument::new(
                    None,
                    vec![crate::annotations::AnnotationObject::new(1, shape)],
                ),
            )
            .expect("annotated");
    }

    #[test]
    fn the_installed_copy_is_kept_at_any_age_and_released_only_by_the_next_installation() {
        let (_workspace, library, png) = library();
        let first = copy_at(&library, png, NOW);
        install(&library, &first);

        tidy_at(&library, NOW + 30 * HOUR);
        assert!(first[0].exists());

        let second = copy_at(&library, png, NOW + 30 * HOUR);
        assert!(first[0].exists());
        install(&library, &second);

        tidy_at(&library, NOW + 31 * HOUR);
        assert!(!first[0].exists());
        assert!(second[0].exists());
    }

    #[test]
    fn a_refused_copy_leaves_the_installed_one_whatever_its_age() {
        let (_workspace, mut library, png) = library();
        let installed = copy_at(&library, png, NOW);
        install(&library, &installed);
        let no_room =
            library.clipboard_operation(&[png], &FlatFiles::new(), NOW + 30 * HOUR, &cramped);
        annotate(&mut library, png);
        let missing_render =
            library.clipboard_operation(&[png], &FlatFiles::new(), NOW + 30 * HOUR, &roomy);

        assert!(missing_render.is_err());
        assert!(matches!(no_room, Err(LibraryError::StorageFull)));
        assert!(installed[0].exists());
        let operations = fs::read_dir(layout::clipboard_dir(library.root()))
            .expect("clipboard folder")
            .flatten()
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .count();
        assert_eq!(operations, 1);
    }

    #[test]
    fn a_copy_that_fails_halfway_leaves_the_installed_one() {
        let (_workspace, library, png) = library();
        let installed = copy_at(&library, png, NOW);
        install(&library, &installed);
        let asset = library.asset(png).expect("read").expect("asset");
        let stored = library.root().join(&asset.stored_path);
        fs::set_permissions(&stored, fs::Permissions::from_mode(0o000)).expect("locked");

        let result =
            library.clipboard_operation(&[png], &FlatFiles::new(), NOW + 30 * HOUR, &roomy);

        fs::set_permissions(&stored, fs::Permissions::from_mode(0o644)).expect("unlocked");
        assert!(result.is_err());
        assert!(installed[0].exists());
    }

    #[test]
    fn abandoning_an_installation_removes_the_new_folder_and_keeps_the_old_content() {
        let (_workspace, library, png) = library();
        let old = copy_at(&library, png, NOW);
        install(&library, &old);
        let new = copy_at(&library, png, NOW + HOUR);
        let pending = library.begin_clipboard_install(&new).expect("begins");
        assert!(is_protected(&folder_of(&new)));

        pending.abandon();

        assert!(!folder_of(&new).exists());
        assert!(
            !layout::clipboard_dir(library.root())
                .join(format!(
                    ".{}.bytes",
                    folder_of(&new).file_name().expect("name").to_string_lossy()
                ))
                .exists()
        );
        assert!(!is_protected(&folder_of(&new)));
        assert!(old[0].exists());
        tidy_at(&library, NOW + 30 * HOUR);
        assert!(old[0].exists());
    }

    #[test]
    fn an_unresolved_installation_keeps_both_folders_protected() {
        let (_workspace, library, png) = library();
        let old = copy_at(&library, png, NOW);
        install(&library, &old);
        let new = copy_at(&library, png, NOW + HOUR);
        let unresolved = library.begin_clipboard_install(&new).expect("begins");
        drop(unresolved);

        tidy_at(&library, NOW + 40 * HOUR);
        let pressured =
            library.export_operation(&[png], &FlatFiles::new(), NOW + 40 * HOUR, &cramped);

        assert!(old[0].exists());
        assert!(new[0].exists());
        assert!(matches!(pressured, Err(LibraryError::StorageFull)));
    }

    #[test]
    fn committing_releases_a_forgotten_installation_of_the_same_library() {
        let (_workspace, library, png) = library();
        let forgotten = copy_at(&library, png, NOW);
        drop(library.begin_clipboard_install(&forgotten).expect("begins"));
        let current = copy_at(&library, png, NOW + HOUR);
        install(&library, &current);

        tidy_at(&library, NOW + 40 * HOUR);

        assert!(!forgotten[0].exists());
        assert!(current[0].exists());
    }

    #[test]
    fn an_installation_is_refused_for_anything_that_is_not_a_copy_of_this_library() {
        let (workspace, library, png) = library();
        let copies = copy_at(&library, png, NOW);
        let root = layout::clipboard_dir(library.root());
        let elsewhere = workspace.path().join("elsewhere");
        fs::create_dir(&elsewhere).expect("folder");
        fs::write(elsewhere.join("a.png"), b"a").expect("file");
        let unnamed = root.join("not-an-operation");
        fs::create_dir(&unnamed).expect("folder");
        fs::write(unnamed.join("a.png"), b"a").expect("file");
        let linked = root.join(export_lifetime::operation_name(NOW, 7));
        symlink(&elsewhere, &linked).expect("link");
        let traversal = root
            .join("..")
            .join("clipboard")
            .join(
                copies[0]
                    .parent()
                    .expect("folder")
                    .file_name()
                    .expect("name"),
            )
            .join("a.png");
        let mixed = vec![copies[0].clone(), elsewhere.join("a.png")];

        for refused in [
            Vec::new(),
            vec![elsewhere.join("a.png")],
            vec![unnamed.join("a.png")],
            vec![linked.join("a.png")],
            vec![traversal],
            mixed,
        ] {
            assert!(library.begin_clipboard_install(&refused).is_err());
        }
        assert!(!is_protected(&folder_of(&copies)));
        assert!(!is_protected(&elsewhere));
        assert!(elsewhere.join("a.png").exists());
    }

    #[test]
    fn abandoning_never_follows_a_link_swapped_in_place_of_the_folder() {
        let (workspace, library, png) = library();
        let copies = copy_at(&library, png, NOW);
        let pending = library.begin_clipboard_install(&copies).expect("begins");
        let folder = folder_of(&copies);
        let elsewhere = workspace.path().join("elsewhere");
        fs::create_dir(&elsewhere).expect("folder");
        fs::write(elsewhere.join("keep.png"), b"keep").expect("file");
        fs::remove_dir_all(&folder).expect("folder removed");
        symlink(&elsewhere, &folder).expect("link");

        pending.abandon();

        assert!(elsewhere.join("keep.png").exists());
    }

    #[test]
    fn committing_cannot_fail_even_when_the_registry_was_poisoned() {
        let (_workspace, library, png) = library();
        let _ = std::thread::spawn(|| {
            let _guard = REGISTRY.lock();
            panic!("poisoning the registry");
        })
        .join();
        let copies = copy_at(&library, png, NOW);

        install(&library, &copies);

        assert!(is_protected(&folder_of(&copies)));
    }

    #[test]
    fn an_export_cannot_free_the_installed_clipboard_copy_even_when_it_would_fit_the_request() {
        let (_workspace, library, png) = library();
        let installed = copy_at(&library, png, NOW);
        install(&library, &installed);

        let result = library.export_operation(&[png], &FlatFiles::new(), NOW + 5 * HOUR, &cramped);

        assert!(matches!(result, Err(LibraryError::StorageFull)));
        assert!(installed[0].exists());
    }

    #[test]
    fn an_export_frees_an_old_clipboard_copy_that_is_not_installed() {
        let (_workspace, library, png) = library();
        let stale = copy_at(&library, png, NOW);
        let watched = stale[0].clone();
        let space = move |_: &Path| {
            Some(VolumeSpace {
                free: if watched.exists() { GIB / 2 } else { 500 * GIB },
                total: 100 * GIB,
            })
        };

        let export = library
            .export_operation(&[png], &FlatFiles::new(), NOW + 5 * HOUR, &space)
            .expect("there is room once the old copy is gone");

        assert!(!stale[0].exists());
        assert!(export[0].exists());
    }

    #[test]
    fn the_clipboard_and_the_exports_share_one_budget() {
        let (_workspace, library, png) = library();
        let export = library
            .export_operation(&[png], &FlatFiles::new(), NOW, &roomy)
            .expect("export");
        let export_folder = folder_of(&export);
        let name = export_folder
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        fs::write(
            layout::export_dir(library.root()).join(format!(".{name}.bytes")),
            (8 * GIB).to_string(),
        )
        .expect("size");

        let refused = library.clipboard_operation(&[png], &FlatFiles::new(), NOW + HOUR, &roomy);

        assert!(matches!(refused, Err(LibraryError::StorageFull)));
        assert!(export[0].exists());
    }

    fn published_in(root: &Path, hours_old: u64, bytes: u64) -> PathBuf {
        export_lifetime::secure_root(root).expect("root");
        let name = export_lifetime::operation_name(NOW - hours_old * HOUR, hours_old + 100);
        let staging = export_lifetime::create_staging(root, &name).expect("staging");
        fs::write(staging.join("a.png"), b"a").expect("file");
        export_lifetime::publish(root, &name, bytes).expect("published")
    }

    fn opening_root(library: &Library, asset: AssetId) -> PathBuf {
        layout::opening_dir(library.root()).join(asset.to_string())
    }

    #[test]
    fn the_opening_copies_count_in_the_budget_of_every_other_transfer() {
        let (_workspace, library, png) = library();
        let opened = published_in(&opening_root(&library, png), 1, 8 * GIB);

        let export = library.export_operation(&[png], &FlatFiles::new(), NOW, &roomy);
        let copy = library.clipboard_operation(&[png], &FlatFiles::new(), NOW, &roomy);

        assert!(matches!(export, Err(LibraryError::StorageFull)));
        assert!(matches!(copy, Err(LibraryError::StorageFull)));
        assert!(opened.exists());
    }

    #[test]
    fn a_transfer_frees_an_old_opening_copy_when_it_needs_the_room() {
        let (_workspace, library, png) = library();
        let opened = published_in(&opening_root(&library, png), 10, GIB);
        let watched = opened.clone();
        let space = move |_: &Path| {
            Some(VolumeSpace {
                free: if watched.exists() { GIB / 2 } else { 500 * GIB },
                total: 100 * GIB,
            })
        };

        let copy = library
            .clipboard_operation(&[png], &FlatFiles::new(), NOW, &space)
            .expect("there is room once the old opening copy is gone");

        assert!(!opened.exists());
        assert!(copy[0].exists());
    }

    #[test]
    fn a_recent_opening_copy_is_never_freed_by_another_transfer() {
        let (_workspace, library, png) = library();
        let opened = published_in(&opening_root(&library, png), 1, GIB);

        let result = library.export_operation(&[png], &FlatFiles::new(), NOW, &cramped);

        assert!(matches!(result, Err(LibraryError::StorageFull)));
        assert!(opened.exists());
    }

    #[test]
    fn folders_that_are_not_real_asset_folders_are_left_out_of_the_budget() {
        let (workspace, library, png) = library();
        let opening = layout::opening_dir(library.root());
        let stranger = published_in(&opening.join("not-an-asset"), 1, 8 * GIB);
        let elsewhere = workspace.path().join("elsewhere");
        let outside = published_in(&elsewhere, 1, 8 * GIB);
        let linked = opening.join("01a15389-0f99-7438-9ab3-9d143a65c533");
        symlink(&elsewhere, &linked).expect("link");

        let result = library.export_operation(&[png], &FlatFiles::new(), NOW, &roomy);

        assert!(result.is_ok());
        assert!(stranger.exists());
        assert!(outside.exists());
    }

    fn open_at(library: &Library, asset: AssetId, now_ms: u64) -> PathBuf {
        library
            .opening_operation(asset, &FlatFiles::new(), now_ms, &roomy)
            .expect("opening copy is prepared")
            .expect("asset exists")
    }

    #[test]
    fn an_opening_never_frees_the_installed_clipboard_copy_even_after_a_day() {
        let (_workspace, library, png) = library();
        let installed = copy_at(&library, png, NOW);
        install(&library, &installed);

        let refused = library.opening_operation(png, &FlatFiles::new(), NOW + 30 * HOUR, &cramped);

        assert!(matches!(refused, Err(LibraryError::StorageFull)));
        assert!(installed[0].exists());
    }

    #[test]
    fn an_opening_is_refused_when_recent_transfers_hold_the_shared_budget() {
        let (_workspace, library, png) = library();
        let export = library
            .export_operation(&[png], &FlatFiles::new(), NOW, &roomy)
            .expect("export");
        let name = folder_of(&export)
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        fs::write(
            layout::export_dir(library.root()).join(format!(".{name}.bytes")),
            (8 * GIB).to_string(),
        )
        .expect("size");

        let refused = library.opening_operation(png, &FlatFiles::new(), NOW + HOUR, &roomy);

        assert!(matches!(refused, Err(LibraryError::StorageFull)));
        assert!(export[0].exists());
        assert!(
            !layout::opening_dir(library.root())
                .join(png.to_string())
                .exists()
                || fs::read_dir(layout::opening_dir(library.root()).join(png.to_string()))
                    .expect("asset folder")
                    .flatten()
                    .all(|entry| entry.file_name().to_string_lossy().starts_with('.'))
        );
    }

    #[test]
    fn an_opening_frees_the_oldest_normal_transfer_of_any_kind_when_it_needs_the_room() {
        let (_workspace, library, png) = library();
        let stale = copy_at(&library, png, NOW - 10 * HOUR);
        let recent = copy_at(&library, png, NOW - HOUR);
        let watched = stale[0].clone();
        let space = move |_: &Path| {
            Some(VolumeSpace {
                free: if watched.exists() { GIB / 2 } else { 500 * GIB },
                total: 100 * GIB,
            })
        };

        let opened = library
            .opening_operation(png, &FlatFiles::new(), NOW, &space)
            .expect("there is room once the old copy is gone")
            .expect("asset exists");

        assert!(!stale[0].exists());
        assert!(recent[0].exists() && opened.exists());
    }

    #[test]
    fn an_opening_removes_nothing_that_is_younger_than_two_hours() {
        let (_workspace, library, png) = library();
        let young = open_at(&library, png, NOW - HOUR);

        let refused = library.opening_operation(png, &FlatFiles::new(), NOW, &cramped);

        assert!(matches!(refused, Err(LibraryError::StorageFull)));
        assert!(young.exists());
    }

    #[test]
    fn an_opening_removes_what_is_older_than_a_day_when_there_is_room() {
        let (_workspace, library, png) = library();
        let old = open_at(&library, png, NOW - 30 * HOUR);
        let middle = open_at(&library, png, NOW - 5 * HOUR);

        let current = open_at(&library, png, NOW);

        assert!(!old.exists());
        assert!(middle.exists() && current.exists());
    }

    #[test]
    fn two_openings_at_the_same_instant_never_share_a_path() {
        let (_workspace, library, png) = library();

        let first = open_at(&library, png, NOW);
        let second = open_at(&library, png, NOW);

        assert_ne!(first, second);
        assert!(first.exists() && second.exists());
    }
}
