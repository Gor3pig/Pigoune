use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pigoune_core::{
    AssetCommand, AssetId, CACHE_DIR_NAME, DATABASE_FILE_NAME, FlatFiles, ImportOutcome, Library,
    LibraryError,
};
use rusqlite::Connection;
use tempfile::TempDir;

const SOME_ID: &str = "01a10769-97bf-7319-b05f-99fc751d637f";
const KNOWN_FOLDERS: [&str; 6] = [
    "thumbnails",
    "annotated",
    "export",
    "clipboard",
    "open",
    "pasted",
];
const DETAILS_FILES: [&str; 4] = [
    "visual-details.db",
    "visual-details.db-journal",
    "visual-details.db-wal",
    "visual-details.db-shm",
];

fn refused(error: &LibraryError) -> bool {
    format!("{error:?}").starts_with("UnsafeCache")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn snapshot(folder: &Path) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .strip_prefix(folder)
                .expect("inside")
                .to_string_lossy()
                .into_owned();
            let metadata = fs::symlink_metadata(&path).expect("metadata");
            if metadata.file_type().is_symlink() {
                found.insert(
                    name,
                    format!("link {}", fs::read_link(&path).expect("link").display()),
                );
            } else if metadata.is_dir() {
                found.insert(name, "folder".to_owned());
                pending.push(path);
            } else if metadata.is_file() {
                let hash = blake3::hash(&fs::read(&path).expect("read")).to_hex();
                found.insert(name, format!("{} {}", metadata.len(), &hash[..16]));
            } else {
                found.insert(name, "special".to_owned());
            }
        }
    }
    found
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("folders");
    fs::write(path, content).expect("file");
}

fn witness_in(workspace: &Path) -> PathBuf {
    let witness = workspace.join("outside");
    write(&witness.join("pasted/important.txt"), "precious");
    write(&witness.join("pasted/deep/other.txt"), "precious too");
    write(&witness.join("annotated/foreign.png"), "not ours");
    write(
        &witness.join(format!(
            "annotated/{SOME_ID}-00000000000000000000000000000000.png"
        )),
        "old render",
    );
    write(
        &witness.join(format!("thumbnails/{SOME_ID}.png")),
        "loose thumbnail",
    );
    write(
        &witness.join(format!("thumbnails/256/{SOME_ID}.png")),
        "loose thumbnail 256",
    );
    write(&witness.join("thumbnails/256/ab/keep.txt"), "bucket");
    write(&witness.join("export/keep.txt"), "keep");
    write(&witness.join("clipboard/keep.txt"), "keep");
    write(&witness.join("open/keep.txt"), "keep");
    write(&witness.join(format!("open/{SOME_ID}/keep.txt")), "keep");
    write(&witness.join("plain.txt"), "plain");
    witness
}

fn witness_file(workspace: &Path) -> PathBuf {
    let file = workspace.join("outside-file.bin");
    fs::write(&file, "do not touch").expect("witness file");
    file
}

struct Fixture {
    workspace: TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            workspace: tempfile::tempdir().expect("workspace"),
        }
    }

    fn path(&self) -> &Path {
        self.workspace.path()
    }

    fn library_with_assets(&self) -> (PathBuf, Vec<AssetId>) {
        let mut library = Library::create(self.path(), "Essai").expect("library");
        let ids = ["red-dot.png", "blue-photo.jpg"]
            .iter()
            .map(
                |name| match library.import_file(&fixture(name), None).expect("import") {
                    ImportOutcome::Imported(id) => id,
                    other => panic!("unexpected {other:?}"),
                },
            )
            .collect();
        (library.root().to_path_buf(), ids)
    }

    fn replace_with_link(entry: &Path, target: &Path) {
        if fs::symlink_metadata(entry).is_ok() {
            let kind = fs::symlink_metadata(entry).expect("metadata");
            if kind.is_dir() && !kind.file_type().is_symlink() {
                fs::remove_dir_all(entry).expect("removed");
            } else {
                fs::remove_file(entry).expect("removed");
            }
        }
        fs::create_dir_all(entry.parent().expect("parent")).expect("parent");
        symlink(target, entry).expect("link");
    }
}

fn open(root: &Path) -> Result<Library, LibraryError> {
    Library::open(root)
}

fn refusal(root: &Path) -> LibraryError {
    match open(root) {
        Err(error) => error,
        Ok(_) => panic!("the library was accepted"),
    }
}

fn stays_a_link(entry: &Path, target: &Path) {
    let metadata = fs::symlink_metadata(entry).expect("the link is still there");
    assert!(
        metadata.file_type().is_symlink(),
        "{} was replaced",
        entry.display()
    );
    assert_eq!(fs::read_link(entry).expect("link"), target);
}

#[test]
fn a_plain_cache_works_as_before() {
    let fixture = Fixture::new();
    let (root, ids) = fixture.library_with_assets();
    let mut library = open(&root).expect("opens");
    let flats = FlatFiles::new();
    library
        .save_pasted_image("Pasted", "png", b"png")
        .expect("paste");
    assert!(
        !library
            .export_copies(&ids, &flats)
            .expect("export")
            .is_empty()
    );
    assert!(
        !library
            .clipboard_copies(&ids, &flats)
            .expect("clipboard")
            .is_empty()
    );
    assert!(
        library
            .opening_copy(ids[0], &flats)
            .expect("opening")
            .is_some()
    );
    library.clear_thumbnail_cache().expect("cleared");
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![ids[0]],
            trashed: true,
        })
        .expect("trashed");
    assert_eq!(library.empty_trash().expect("emptied"), 1);
    drop(library);
    assert!(open(&root).is_ok());
}

#[test]
fn a_cache_that_is_a_link_is_refused_and_nothing_outside_changes() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let cache = root.join(CACHE_DIR_NAME);
    Fixture::replace_with_link(&cache, &witness);
    let before = snapshot(&witness);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert_eq!(snapshot(&witness), before);
    stays_a_link(&cache, &witness);
}

#[test]
fn a_cache_that_is_a_broken_link_is_refused_with_the_same_error() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let missing = fixture.path().join("nowhere");
    let cache = root.join(CACHE_DIR_NAME);
    Fixture::replace_with_link(&cache, &missing);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert!(!missing.exists());
    stays_a_link(&cache, &missing);
}

#[test]
fn a_cache_that_is_a_file_is_refused_and_the_file_is_kept() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let cache = root.join(CACHE_DIR_NAME);
    fs::remove_dir_all(&cache).expect("cache removed");
    fs::write(&cache, "not a folder").expect("file");
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert_eq!(fs::read_to_string(&cache).expect("read"), "not a folder");
}

#[test]
fn a_cache_that_is_a_link_to_a_folder_inside_the_library_is_refused_too() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let inside = root.join("elsewhere-in-the-library");
    fs::create_dir_all(&inside).expect("folder");
    let cache = root.join(CACHE_DIR_NAME);
    Fixture::replace_with_link(&cache, &inside);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    stays_a_link(&cache, &inside);
}

#[test]
fn every_known_entry_of_the_cache_that_is_a_link_is_refused() {
    for entry in KNOWN_FOLDERS {
        let fixture = Fixture::new();
        let (root, _) = fixture.library_with_assets();
        let witness = witness_in(fixture.path());
        let link = root.join(CACHE_DIR_NAME).join(entry);
        Fixture::replace_with_link(&link, &witness);
        let before = snapshot(&witness);
        let error = open(&root)
            .err()
            .unwrap_or_else(|| panic!("{entry} was accepted"));
        assert!(refused(&error), "{entry}: {error:?}");
        assert!(format!("{error}").contains(entry), "{entry}: {error}");
        assert_eq!(snapshot(&witness), before, "{entry}");
        stays_a_link(&link, &witness);
    }
}

#[test]
fn the_files_of_the_details_database_must_be_plain_files() {
    for name in DETAILS_FILES {
        let fixture = Fixture::new();
        let (root, _) = fixture.library_with_assets();
        let witness = witness_file(fixture.path());
        let entry = root.join(CACHE_DIR_NAME).join(name);
        let _ = fs::remove_file(&entry);
        symlink(&witness, &entry).expect("link");
        let error = open(&root)
            .err()
            .unwrap_or_else(|| panic!("{name} link accepted"));
        assert!(refused(&error), "{name}: {error:?}");
        assert!(format!("{error}").contains(name), "{name}: {error}");
        assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
        stays_a_link(&entry, &witness);

        fs::remove_file(&entry).expect("link removed");
        fs::create_dir(&entry).expect("folder");
        let error = open(&root)
            .err()
            .unwrap_or_else(|| panic!("{name} folder accepted"));
        assert!(refused(&error), "{name}: {error:?}");
        fs::remove_dir(&entry).expect("folder removed");
    }
}

#[test]
fn a_pipe_in_place_of_a_details_file_is_refused_without_blocking() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let entry = root.join(CACHE_DIR_NAME).join("visual-details.db");
    let _ = fs::remove_file(&entry);
    let made = Command::new("mkfifo").arg(&entry).status();
    if !made.is_ok_and(|status| status.success()) {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    let root_for_thread = root.clone();
    thread::spawn(move || {
        let _ = sender.send(
            open(&root_for_thread)
                .err()
                .map(|error| format!("{error:?}")),
        );
    });
    let outcome = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the library opened or was refused without blocking");
    assert!(outcome.is_some_and(|text| text.starts_with("UnsafeCache")));
}

#[test]
fn an_ordinary_journal_left_by_a_crash_is_accepted() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    drop(open(&root).expect("opens"));
    let journal = root.join(CACHE_DIR_NAME).join("visual-details.db-journal");
    fs::write(&journal, b"").expect("journal");
    assert!(open(&root).is_ok());
}

#[test]
fn a_details_database_in_wal_mode_is_opened_and_kept() {
    let fixture = Fixture::new();
    let (root, ids) = fixture.library_with_assets();
    let known = ids[0].to_string();
    drop(open(&root).expect("opens"));
    let details = root.join(CACHE_DIR_NAME).join("visual-details.db");
    {
        let connection = Connection::open(&details).expect("details");
        let mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .expect("wal");
        assert_eq!(mode.to_lowercase(), "wal");
        connection
            .execute(
                "INSERT OR REPLACE INTO asset_details (asset_id, detail) VALUES (?1, x'01')",
                [&known],
            )
            .expect("row");
    }
    assert!(open(&root).is_ok());
    let connection = Connection::open(&details).expect("details");
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM asset_details WHERE asset_id = ?1",
            [&known],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn a_thumbnail_size_folder_that_is_a_link_is_refused_and_the_target_is_not_touched() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let link = root.join(CACHE_DIR_NAME).join("thumbnails/128");
    Fixture::replace_with_link(&link, &witness);
    let before = snapshot(&witness);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert!(format!("{error}").contains("thumbnails/128"), "{error}");
    assert_eq!(snapshot(&witness), before);
    stays_a_link(&link, &witness);
}

#[test]
fn a_thumbnail_bucket_that_is_a_link_is_refused_and_the_target_is_not_touched() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let link = root.join(CACHE_DIR_NAME).join("thumbnails/256/ab");
    Fixture::replace_with_link(&link, &witness);
    let before = snapshot(&witness);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert_eq!(snapshot(&witness), before);
    stays_a_link(&link, &witness);
}

#[test]
fn a_library_that_needs_a_migration_is_refused_before_anything_is_written() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let version = {
        let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database");
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("version")
    };
    {
        let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database");
        connection
            .pragma_update(None, "user_version", version - 1)
            .expect("older");
    }
    let witness = witness_in(fixture.path());
    let cache = root.join(CACHE_DIR_NAME);
    Fixture::replace_with_link(&cache, &witness);
    let before = snapshot(&witness);
    let error = refusal(&root);
    assert!(refused(&error), "{error:?}");
    assert_eq!(snapshot(&witness), before);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database");
    let after: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(after, version - 1, "the migration ran before the check");
}

#[test]
fn a_library_reached_through_a_link_is_opened_normally() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let door = fixture.path().join("door.pigoune");
    symlink(&root, &door).expect("link");
    assert!(open(&door).is_ok());
}

#[test]
fn a_missing_cache_is_made_again_as_a_plain_folder() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let cache = root.join(CACHE_DIR_NAME);
    fs::remove_dir_all(&cache).expect("cache removed");
    drop(open(&root).expect("opens"));
    let metadata = fs::symlink_metadata(&cache).expect("recreated");
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
}
