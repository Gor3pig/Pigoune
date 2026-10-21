use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pigoune_core::{AssetId, ImportOutcome, Library, LibraryError};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(10);

struct Workspace {
    folder: TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            folder: tempfile::tempdir().expect("workspace"),
        }
    }

    fn path(&self) -> &Path {
        self.folder.path()
    }

    fn library(&self, name: &str) -> (PathBuf, Vec<AssetId>) {
        let parent = self.path().join(format!("parent-{name}"));
        fs::create_dir_all(&parent).expect("parent");
        let mut library = Library::create(&parent, name).expect("library");
        let ids = ["red-dot.png", "blue-photo.jpg"]
            .iter()
            .map(|file| {
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(file);
                match library.import_file(&source, None).expect("import") {
                    ImportOutcome::Imported(id) => id,
                    other => panic!("unexpected {other:?}"),
                }
            })
            .collect();
        (library.root().to_path_buf(), ids)
    }
}

fn open(root: &Path) -> Result<Library, LibraryError> {
    for _ in 0..100 {
        match Library::open(root) {
            Err(LibraryError::InUse) => thread::sleep(Duration::from_millis(20)),
            other => return other,
        }
    }
    Library::open(root)
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
        } else if kind.is_file() {
            found.insert(
                relative,
                format!("file:{:?}", fs::read(&path).map(|bytes| blake(&bytes))),
            );
        } else {
            found.insert(relative, "special".to_owned());
        }
    }
}

fn blake(bytes: &[u8]) -> (usize, u64) {
    let sum = bytes.iter().fold(0_u64, |sum, byte| {
        sum.wrapping_mul(131).wrapping_add(u64::from(*byte))
    });
    (bytes.len(), sum)
}

fn refused(result: &Result<Library, LibraryError>) -> bool {
    matches!(result, Err(LibraryError::UnsafeStorage(_)))
}

fn mkfifo(path: &Path) -> bool {
    Command::new("mkfifo")
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn a_healthy_library_opens_closes_and_opens_again() {
    let workspace = Workspace::new();
    let (root, _) = workspace.library("Sain");

    let first = open(&root);
    assert!(first.is_ok(), "{:?}", first.err());
    drop(first);
    let second = open(&root);
    assert!(second.is_ok(), "{:?}", second.err());
}

#[test]
fn a_library_reached_through_a_link_to_its_folder_still_opens() {
    let workspace = Workspace::new();
    let (root, _) = workspace.library("Sain");
    let door = workspace.path().join("door.pigoune");
    symlink(&root, &door).expect("link");

    let opened = open(&door);

    assert!(opened.is_ok(), "{:?}", opened.err());
}

#[test]
fn a_database_that_is_a_link_to_another_library_is_refused_and_that_library_is_untouched() {
    let workspace = Workspace::new();
    let (trap, _) = workspace.library("Piege");
    let (neighbour, _) = workspace.library("Voisine");
    let before = snapshot(&neighbour);
    fs::remove_file(trap.join("library.db")).expect("removed");
    symlink(neighbour.join("library.db"), trap.join("library.db")).expect("link");

    let result = open(&trap);

    assert!(refused(&result), "{:?}", result.err());
    assert_eq!(snapshot(&neighbour), before);
    assert!(trap.join("library.db").is_symlink());
}

#[test]
fn a_database_with_several_hard_links_is_refused_and_both_names_are_untouched() {
    let workspace = Workspace::new();
    let (trap, _) = workspace.library("Piege");
    let (neighbour, _) = workspace.library("Voisine");
    fs::remove_file(trap.join("library.db")).expect("removed");
    fs::hard_link(neighbour.join("library.db"), trap.join("library.db")).expect("hard link");
    let before = snapshot(&neighbour);

    let result = open(&trap);

    assert!(
        matches!(result, Err(LibraryError::SharedDatabase)),
        "{:?}",
        result.err()
    );
    assert_eq!(snapshot(&neighbour), before);
    assert!(trap.join("library.db").is_file());
}

#[test]
fn an_independent_copy_of_a_library_opens_normally() {
    let workspace = Workspace::new();
    let (root, ids) = workspace.library("Sain");
    let copy = workspace.path().join("copy.pigoune");
    assert!(
        Command::new("cp")
            .arg("-a")
            .arg(&root)
            .arg(&copy)
            .status()
            .expect("cp")
            .success()
    );

    let opened = open(&copy).expect("opens");

    assert!(opened.asset(ids[0]).expect("query").is_some());
}

#[test]
fn a_database_that_is_a_broken_link_a_folder_or_a_pipe_is_not_a_library_and_nothing_blocks() {
    let workspace = Workspace::new();
    let (root, _) = workspace.library("Sain");
    let database = root.join("library.db");
    fs::remove_file(&database).expect("removed");
    let (sender, receiver) = mpsc::channel();

    symlink(workspace.path().join("nothing"), &database).expect("link");
    let broken = open(&root).err();
    fs::remove_file(&database).expect("removed");
    fs::create_dir(&database).expect("folder");
    let folder = open(&root).err();
    fs::remove_dir(&database).expect("removed");
    let piped = mkfifo(&database);
    let root_for_thread = root.clone();
    thread::spawn(move || {
        let _ = sender.send(open(&root_for_thread).err().map(|e| e.to_string()));
    });
    let pipe = receiver.recv_timeout(WAIT).expect("no blocking");

    assert!(
        matches!(broken, Some(LibraryError::UnsafeStorage(_))),
        "{broken:?}"
    );
    assert!(
        matches!(folder, Some(LibraryError::NotALibrary(_))),
        "{folder:?}"
    );
    if piped {
        assert!(pipe.is_some());
    }
}

#[test]
fn a_database_companion_that_is_a_link_or_a_pipe_is_refused_and_its_target_is_untouched() {
    for suffix in ["-journal", "-wal", "-shm"] {
        let workspace = Workspace::new();
        let (root, _) = workspace.library("Sain");
        let witness = workspace.path().join("witness.bin");
        fs::write(&witness, b"WITNESS").expect("witness");
        let companion = root.join(format!("library.db{suffix}"));
        symlink(&witness, &companion).expect("link");

        let result = open(&root);

        assert!(refused(&result), "{suffix}: {:?}", result.err());
        assert_eq!(fs::read(&witness).expect("witness"), b"WITNESS");
        assert!(companion.is_symlink());

        fs::remove_file(&companion).expect("removed");
        if mkfifo(&companion) {
            let (sender, receiver) = mpsc::channel();
            let root_for_thread = root.clone();
            thread::spawn(move || {
                let _ = sender.send(open(&root_for_thread).is_err());
            });
            assert!(
                receiver.recv_timeout(WAIT).expect("no blocking"),
                "{suffix}"
            );
        }
    }
}

#[test]
fn an_ordinary_journal_left_by_a_crash_is_accepted() {
    let workspace = Workspace::new();
    let (root, ids) = workspace.library("Sain");
    fs::write(root.join("library.db-journal"), b"").expect("journal");

    let opened = open(&root).expect("opens");

    assert!(opened.asset(ids[0]).expect("query").is_some());
}

#[test]
fn an_originals_folder_that_is_a_link_is_refused_and_nothing_outside_changes() {
    let workspace = Workspace::new();
    let (root, _) = workspace.library("Piege");
    let outside = workspace.path().join("outside");
    fs::create_dir_all(outside.join("01a15388-86c4-73c4-bcba-5e2c72ef0257")).expect("legacy");
    fs::write(
        outside.join("01a15388-86c4-73c4-bcba-5e2c72ef0257/old.txt"),
        b"legacy",
    )
    .expect("file");
    fs::create_dir_all(outside.join(".download.partial")).expect("partial");
    fs::write(outside.join(".download.partial/data.txt"), b"keep").expect("file");
    fs::remove_dir_all(root.join("files")).expect("removed");
    symlink(&outside, root.join("files")).expect("link");
    let before = snapshot(&outside);

    let result = open(&root);

    assert!(refused(&result), "{:?}", result.err());
    assert_eq!(snapshot(&outside), before);
    assert!(root.join("files").is_symlink());
}

#[test]
fn a_bucket_that_is_a_link_to_a_folder_a_file_or_nothing_is_refused_at_the_opening() {
    for kind in ["folder", "file", "broken"] {
        let workspace = Workspace::new();
        let (root, ids) = workspace.library("Piege");
        let bucket = root.join("files").join(&ids[1].to_string()[34..]);
        let target = workspace.path().join("target");
        match kind {
            "folder" => {
                fs::rename(&bucket, &target).expect("moved");
            }
            "file" => {
                fs::remove_dir_all(&bucket).expect("removed");
                fs::write(&target, b"file").expect("file");
            }
            _ => {
                fs::remove_dir_all(&bucket).expect("removed");
            }
        }
        symlink(&target, &bucket).expect("link");
        let before = snapshot(workspace.path());

        let result = open(&root);

        assert!(refused(&result), "{kind}: {:?}", result.err());
        assert_eq!(snapshot(workspace.path()), before, "{kind}");
    }
}
