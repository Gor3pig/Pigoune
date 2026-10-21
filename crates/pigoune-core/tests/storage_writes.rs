use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use pigoune_core::{AssetCommand, AssetId, ImportOutcome, Library, LibraryError, ReplaceError};
use tempfile::TempDir;

struct Trap {
    workspace: TempDir,
    root: PathBuf,
    red: AssetId,
    photo: AssetId,
}

impl Trap {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut library = Library::create(workspace.path(), "Piege").expect("library");
        let mut import = |name: &str| match library.import_file(&sample(name), None) {
            Ok(ImportOutcome::Imported(id)) => id,
            other => panic!("unexpected {other:?}"),
        };
        let red = import("red-dot.png");
        let photo = import("blue-photo.jpg");
        let root = library.root().to_path_buf();
        drop(library);
        Self {
            workspace,
            root,
            red,
            photo,
        }
    }

    fn open(&self) -> Library {
        for _ in 0..100 {
            match Library::open(&self.root) {
                Err(LibraryError::InUse) => thread::sleep(Duration::from_millis(20)),
                other => return other.expect("opens"),
            }
        }
        Library::open(&self.root).expect("opens")
    }

    fn files(&self) -> PathBuf {
        self.root.join("files")
    }

    fn bucket_of(&self, id: AssetId) -> PathBuf {
        self.files().join(&id.to_string()[34..])
    }

    fn folder_of(&self, id: AssetId) -> PathBuf {
        self.bucket_of(id).join(id.to_string())
    }

    fn outside(&self, name: &str) -> PathBuf {
        let folder = self.workspace.path().join(name);
        fs::create_dir_all(&folder).expect("outside folder");
        folder
    }

    fn swap_bucket_for_a_link_to_a_copy(&self, id: AssetId) -> PathBuf {
        let bucket = self.bucket_of(id);
        let copy = self.outside("outside-copy").join("bucket");
        fs::rename(&bucket, &copy).expect("moved");
        symlink(&copy, &bucket).expect("link");
        copy
    }
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn snapshot(folder: &Path) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    collect(folder, folder, &mut found);
    found
}

fn collect(base: &Path, folder: &Path, found: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .expect("inside")
            .display()
            .to_string();
        let kind = entry.file_type().expect("kind");
        if kind.is_symlink() {
            found.insert(
                relative,
                format!("link:{}", fs::read_link(&path).expect("link").display()),
            );
        } else if kind.is_dir() {
            found.insert(relative, "dir".to_owned());
            collect(base, &path, found);
        } else {
            found.insert(
                relative,
                format!("file:{}", fs::metadata(&path).map_or(0, |m| m.len())),
            );
        }
    }
}

fn trash(library: &mut Library, id: AssetId) {
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
}

#[test]
fn only_the_unfinished_imports_of_pigoune_are_cleaned_at_the_opening() {
    let trap = Trap::new();
    let unrelated = trap.files().join(".some-download.partial");
    fs::create_dir_all(&unrelated).expect("unrelated");
    fs::write(unrelated.join("data.txt"), b"keep").expect("file");
    let own_id = "01a15388-86c4-73c4-bcba-5e2c72ef0257";
    let own = trap.files().join(format!(".{own_id}.partial"));
    fs::create_dir_all(&own).expect("own");
    fs::write(own.join("half.bin"), b"half").expect("file");
    let own_file = trap
        .files()
        .join(".01a15388-86c4-73c4-bcba-5e2c72ef0258.partial");
    fs::write(&own_file, b"a file, not an import folder").expect("file");

    drop(trap.open());

    assert_eq!(fs::read(unrelated.join("data.txt")).expect("kept"), b"keep");
    assert!(!own.exists());
    assert!(own_file.is_file());
}

#[test]
fn an_unfinished_import_that_holds_a_link_keeps_the_link_and_its_target() {
    let trap = Trap::new();
    let outside = trap.outside("outside");
    fs::write(outside.join("keep.txt"), b"keep").expect("witness");
    let own = trap
        .files()
        .join(".01a15388-86c4-73c4-bcba-5e2c72ef0257.partial");
    fs::create_dir_all(&own).expect("own");
    fs::write(own.join("half.bin"), b"half").expect("file");
    symlink(&outside, own.join("escape")).expect("link");
    let before = snapshot(&outside);

    drop(trap.open());

    assert!(own.join("escape").is_symlink());
    assert!(!own.join("half.bin").exists());
    assert_eq!(snapshot(&outside), before);
}

#[test]
fn deleting_for_good_through_a_bucket_swapped_for_a_link_removes_nothing_outside() {
    let trap = Trap::new();
    let mut library = trap.open();
    trash(&mut library, trap.photo);
    let copy = trap.swap_bucket_for_a_link_to_a_copy(trap.photo);
    let before = snapshot(copy.parent().expect("outside"));

    let removed = library.empty_trash();

    assert!(removed.is_ok());
    assert_eq!(snapshot(copy.parent().expect("outside")), before);
    assert!(trap.bucket_of(trap.photo).is_symlink());
}

#[test]
fn deleting_for_good_removes_the_plain_files_of_the_resource_and_keeps_a_link_inside() {
    let trap = Trap::new();
    let outside = trap.outside("outside");
    fs::write(outside.join("keep.txt"), b"keep").expect("witness");
    let mut library = trap.open();
    trash(&mut library, trap.photo);
    let folder = trap.folder_of(trap.photo);
    symlink(&outside, folder.join("escape")).expect("link");
    let before = snapshot(&outside);

    let removed = library.empty_trash().expect("emptied");

    assert_eq!(removed, 1);
    assert!(folder.join("escape").is_symlink());
    assert!(!folder.join("blue-photo.jpg").exists());
    assert_eq!(snapshot(&outside), before);
}

#[test]
fn deleting_for_good_an_ordinary_resource_removes_its_whole_folder_as_before() {
    let trap = Trap::new();
    let mut library = trap.open();
    trash(&mut library, trap.photo);

    let removed = library.empty_trash().expect("emptied");

    assert_eq!(removed, 1);
    assert!(!trap.folder_of(trap.photo).exists());
    assert!(trap.folder_of(trap.red).join("red-dot.png").is_file());
}

#[test]
fn removing_the_record_of_a_missing_file_never_removes_a_folder_through_a_link() {
    let trap = Trap::new();
    let mut library = trap.open();
    let copy = trap.swap_bucket_for_a_link_to_a_copy(trap.photo);
    let outside_folder = copy.join(trap.photo.to_string());
    fs::remove_file(outside_folder.join("blue-photo.jpg")).expect("file gone");
    assert!(outside_folder.is_dir());

    let removed = library.remove_record_of_missing_file(trap.photo);

    assert!(removed.is_ok(), "{:?}", removed.err());
    assert!(outside_folder.is_dir());
    assert!(trap.bucket_of(trap.photo).is_symlink());
}

#[test]
fn replacing_an_original_never_replaces_a_link_and_never_writes_through_a_folder_link() {
    let trap = Trap::new();
    let library = trap.open();
    let stored = trap.folder_of(trap.photo).join("blue-photo.jpg");
    let witness = trap.outside("outside").join("witness.bin");
    fs::write(&witness, b"WITNESS").expect("witness");
    fs::remove_file(&stored).expect("removed");
    symlink(&witness, &stored).expect("link");

    let refused = library.replace_stored_file(trap.photo, &sample("blue-photo.jpg"));

    assert!(
        matches!(
            refused,
            Err(ReplaceError::Library(LibraryError::UnsafeStorage(_)))
        ),
        "{refused:?}"
    );
    assert_eq!(fs::read_link(&stored).expect("link kept"), witness);
    assert_eq!(fs::read(&witness).expect("witness"), b"WITNESS");

    fs::remove_file(&stored).expect("removed");
    let copy = trap.swap_bucket_for_a_link_to_a_copy(trap.photo);
    let before = snapshot(copy.parent().expect("outside"));
    let refused = library.replace_stored_file(trap.photo, &sample("blue-photo.jpg"));
    assert!(refused.is_err());
    assert_eq!(snapshot(copy.parent().expect("outside")), before);
}

#[test]
fn replacing_an_original_keeps_working_and_refuses_a_temporary_that_is_a_link() {
    let trap = Trap::new();
    let library = trap.open();
    let stored = trap.folder_of(trap.photo).join("blue-photo.jpg");
    fs::remove_file(&stored).expect("removed");

    library
        .replace_stored_file(trap.photo, &sample("blue-photo.jpg"))
        .expect("replaced");
    assert!(stored.is_file());

    let witness = trap.outside("outside").join("witness.bin");
    fs::write(&witness, b"WITNESS").expect("witness");
    let temporary = trap
        .folder_of(trap.photo)
        .join(format!(".{}.partial", trap.photo));
    symlink(&witness, &temporary).expect("link");
    fs::remove_file(&stored).expect("removed");

    let refused = library.replace_stored_file(trap.photo, &sample("blue-photo.jpg"));

    assert!(refused.is_err());
    assert_eq!(fs::read_link(&temporary).expect("link kept"), witness);
    assert_eq!(fs::read(&witness).expect("witness"), b"WITNESS");
}

#[test]
fn importing_while_the_originals_folder_became_a_link_writes_nothing_outside() {
    let trap = Trap::new();
    let mut library = trap.open();
    let outside = trap.outside("import-target");
    fs::remove_dir_all(trap.files()).expect("removed");
    symlink(&outside, trap.files()).expect("link");

    let result = library.import_file(&sample("green-square.webp"), None);

    assert!(result.is_err(), "{result:?}");
    assert!(snapshot(&outside).is_empty());
    assert!(trap.files().is_symlink());
}

#[test]
fn importing_into_a_healthy_library_still_works_and_leaves_no_unfinished_folder() {
    let trap = Trap::new();
    let mut library = trap.open();

    let outcome = library.import_file(&sample("green-square.webp"), None);

    let Ok(ImportOutcome::Imported(id)) = outcome else {
        panic!("unexpected {outcome:?}");
    };
    assert!(trap.folder_of(id).join("green-square.webp").is_file());
    assert!(
        fs::read_dir(trap.files())
            .expect("files")
            .flatten()
            .all(|entry| !entry.file_name().to_string_lossy().ends_with(".partial"))
    );
}

#[test]
fn the_health_check_skips_only_the_unfinished_imports_of_pigoune() {
    let trap = Trap::new();
    let library = trap.open();
    let own = trap
        .files()
        .join(".01a15388-86c4-73c4-bcba-5e2c72ef0257.partial");
    fs::create_dir_all(&own).expect("own");
    fs::write(own.join("half.bin"), b"half").expect("file");
    let other = trap.files().join(".other.partial");
    fs::create_dir_all(&other).expect("other");
    fs::write(other.join("data.txt"), b"data").expect("file");

    let report = library
        .health_plan()
        .expect("plan")
        .run(|_| std::ops::ControlFlow::Continue(()))
        .expect("not cancelled");

    assert_eq!(report.unrecorded.len(), 1);
    assert!(report.unrecorded[0].ends_with("data.txt"));
}
