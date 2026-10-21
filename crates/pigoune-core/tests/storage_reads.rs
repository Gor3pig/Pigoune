use std::fs;
use std::ops::ControlFlow;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pigoune_core::{AssetId, FlatFiles, ImportOutcome, Library, LibraryError};
use rusqlite::Connection;
use tempfile::TempDir;

const SECRET: &str = "CONTENT OUTSIDE THE LIBRARY";
const WAIT: Duration = Duration::from_secs(10);

struct Trap {
    workspace: TempDir,
    root: PathBuf,
    photo: AssetId,
    secret: PathBuf,
}

impl Trap {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut library = Library::create(workspace.path(), "Piege").expect("library");
        let sample = |name: &str| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name)
        };
        let mut import = |name: &str| match library.import_file(&sample(name), None) {
            Ok(ImportOutcome::Imported(id)) => id,
            other => panic!("unexpected {other:?}"),
        };
        import("red-dot.png");
        let photo = import("blue-photo.jpg");
        let root = library.root().to_path_buf();
        drop(library);
        let secret = workspace.path().join("outside-secret.txt");
        fs::write(&secret, SECRET).expect("secret");
        Self {
            workspace,
            root,
            photo,
            secret,
        }
    }

    fn sql(&self, statement: &str) {
        Connection::open(self.root.join("library.db"))
            .expect("database")
            .execute(statement, [])
            .expect("statement");
    }

    fn record(&self, stored_path: &str) {
        self.sql(&format!(
            "UPDATE assets SET stored_path = '{stored_path}' WHERE id = '{}'",
            self.photo
        ));
    }

    fn open(&self) -> Library {
        Library::open(&self.root).expect("opens")
    }

    fn stored(&self) -> PathBuf {
        let library = self.open();
        let asset = library.asset(self.photo).expect("query").expect("asset");
        self.root.join(asset.stored_path)
    }

    fn bucket(&self) -> PathBuf {
        self.stored()
            .parent()
            .and_then(Path::parent)
            .expect("bucket")
            .to_path_buf()
    }

    fn outside(&self, name: &str) -> PathBuf {
        let folder = self.workspace.path().join(name);
        fs::create_dir_all(&folder).expect("outside folder");
        folder
    }

    fn secret_untouched(&self) {
        assert_eq!(fs::read_to_string(&self.secret).expect("secret"), SECRET);
    }
}

fn contents_below(folder: &Path) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let kind = entry.file_type().expect("kind");
        if kind.is_dir() {
            found.extend(contents_below(&entry.path()));
        } else if kind.is_file() {
            found.push(fs::read(entry.path()).expect("read"));
        }
    }
    found
}

fn nothing_secret_below(folder: &Path) {
    for content in contents_below(folder) {
        assert_ne!(content, SECRET.as_bytes(), "the outside content was copied");
    }
}

fn refused(error: &LibraryError) -> bool {
    matches!(error, LibraryError::UnsafeStorage(_))
}

fn every_transfer_is_refused_and_nothing_is_copied(trap: &Trap) {
    let library = trap.open();
    let flats = FlatFiles::new();
    let destination = trap.outside("destination");
    let library_destination = trap.outside("library-destination");

    let export = library.export_copies(&[trap.photo], &flats);
    let clipboard = library.clipboard_copies(&[trap.photo], &flats);
    let opening = library.opening_copy(trap.photo, &flats);
    let folder = library
        .export_to(&[trap.photo], &destination, &flats)
        .expect("report");
    let plan = library
        .library_export_plan("Unclassified", "annotated")
        .expect("plan");
    let whole = plan.run(&library_destination, &flats).expect("run");

    assert!(export.as_ref().is_err_and(refused), "{export:?}");
    assert!(clipboard.as_ref().is_err_and(refused), "{clipboard:?}");
    assert!(opening.as_ref().is_err_and(refused), "{opening:?}");
    assert!(folder.copies.is_empty());
    assert_eq!(folder.failures.len(), 1);
    assert_eq!(whole.failures.len(), 1);
    nothing_secret_below(&trap.root.join("cache"));
    nothing_secret_below(&destination);
    nothing_secret_below(&library_destination);
    trap.secret_untouched();
}

#[test]
fn a_healthy_library_still_transfers_all_its_originals() {
    let trap = Trap::new();
    let library = trap.open();
    let flats = FlatFiles::new();
    let destination = trap.outside("destination");

    let export = library
        .export_copies(&[trap.photo], &flats)
        .expect("export");
    let clipboard = library
        .clipboard_copies(&[trap.photo], &flats)
        .expect("clipboard");
    let opening = library
        .opening_copy(trap.photo, &flats)
        .expect("opening")
        .expect("copy");
    let folder = library
        .export_to(&[trap.photo], &destination, &flats)
        .expect("folder");
    let whole = library
        .library_export_plan("Unclassified", "annotated")
        .expect("plan")
        .run(&trap.outside("library-destination"), &flats)
        .expect("run");

    let asset = library.asset(trap.photo).expect("query").expect("asset");
    let original = fs::read(trap.root.join(asset.stored_path)).expect("original");
    assert_eq!(fs::read(&export[0]).expect("read"), original);
    assert_eq!(fs::read(&clipboard[0]).expect("read"), original);
    assert_eq!(fs::read(&opening).expect("read"), original);
    assert_eq!(fs::read(&folder.copies[0]).expect("read"), original);
    assert_eq!(whole.exported, 2);
    assert!(whole.failures.is_empty());
}

#[test]
fn a_recorded_absolute_path_never_reads_an_outside_file() {
    let trap = Trap::new();
    trap.record(&trap.secret.display().to_string());

    every_transfer_is_refused_and_nothing_is_copied(&trap);
}

#[test]
fn a_recorded_path_with_parent_components_never_reads_an_outside_file() {
    let trap = Trap::new();
    let relative = Path::new("..").join(trap.secret.file_name().expect("name"));
    trap.record(&relative.display().to_string());

    every_transfer_is_refused_and_nothing_is_copied(&trap);
}

#[test]
fn a_recorded_path_that_belongs_to_another_resource_is_refused() {
    let trap = Trap::new();
    let other = trap.bucket().join("01a15388-86c4-73c4-bcba-5e2c72ef0257");
    fs::create_dir_all(&other).expect("other folder");
    fs::write(other.join("blue-photo.jpg"), SECRET).expect("planted");
    let recorded = format!(
        "files/{}/01a15388-86c4-73c4-bcba-5e2c72ef0257/blue-photo.jpg",
        trap.bucket().file_name().expect("bucket").to_string_lossy()
    );
    trap.record(&recorded);

    every_transfer_is_refused_and_nothing_is_copied(&trap);
}

#[test]
fn an_original_replaced_by_a_link_is_refused_and_left_alone() {
    let trap = Trap::new();
    let stored = trap.stored();
    fs::remove_file(&stored).expect("removed");
    symlink(&trap.secret, &stored).expect("link");

    every_transfer_is_refused_and_nothing_is_copied(&trap);

    assert_eq!(fs::read_link(&stored).expect("link kept"), trap.secret);
}

#[test]
fn a_bucket_folder_that_is_a_link_is_refused_and_left_alone() {
    let trap = Trap::new();
    let bucket = trap.bucket();
    let moved = trap.outside("moved-bucket").join("bucket");
    fs::rename(&bucket, &moved).expect("moved");
    symlink(&moved, &bucket).expect("link");

    every_transfer_is_refused_and_nothing_is_copied(&trap);

    assert_eq!(fs::read_link(&bucket).expect("link kept"), moved);
    assert!(moved.is_dir());
}

#[test]
fn an_asset_folder_that_is_a_link_is_refused_and_left_alone() {
    let trap = Trap::new();
    let folder = trap.stored().parent().expect("folder").to_path_buf();
    let moved = trap.outside("moved-asset").join("asset");
    fs::rename(&folder, &moved).expect("moved");
    symlink(&moved, &folder).expect("link");

    every_transfer_is_refused_and_nothing_is_copied(&trap);

    assert_eq!(fs::read_link(&folder).expect("link kept"), moved);
}

#[test]
fn a_pipe_in_place_of_an_original_is_refused_without_blocking() {
    let trap = Trap::new();
    let stored = trap.stored();
    fs::remove_file(&stored).expect("removed");
    if !Command::new("mkfifo")
        .arg(&stored)
        .status()
        .is_ok_and(|status| status.success())
    {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    let photo = trap.photo;
    let library = trap.open();
    thread::spawn(move || {
        let flats = FlatFiles::new();
        let results = (
            library.export_copies(&[photo], &flats).is_err(),
            library.clipboard_copies(&[photo], &flats).is_err(),
            library.opening_copy(photo, &flats).is_err(),
        );
        let _ = sender.send(results);
    });

    let results = receiver.recv_timeout(WAIT).expect("no blocking");

    assert_eq!(results, (true, true, true));
}

#[test]
fn the_health_check_reports_an_unsafe_original_as_missing_and_reads_nothing_outside() {
    let trap = Trap::new();
    trap.record(&trap.secret.display().to_string());
    let library = trap.open();

    let report = library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("not cancelled");

    assert_eq!(report.missing.len(), 1);
    assert!(report.damaged.is_empty());
    assert!(
        library
            .asset(trap.photo)
            .expect("query")
            .is_some_and(|asset| asset.stored_path == trap.secret)
    );
}

#[test]
fn an_unsafe_record_is_never_removed_or_repaired_by_a_refusal() {
    let trap = Trap::new();
    trap.record(&trap.secret.display().to_string());
    let library = trap.open();

    let refusal = library.export_copies(&[trap.photo], &FlatFiles::new());

    assert!(refusal.is_err());
    assert!(
        library
            .asset(trap.photo)
            .expect("query")
            .is_some_and(|asset| asset.stored_path == trap.secret)
    );
    trap.secret_untouched();
}
