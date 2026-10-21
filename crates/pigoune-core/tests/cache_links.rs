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
    let opened = open(&door);
    assert!(opened.is_ok(), "{:?}", opened.err());
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

#[test]
fn a_cache_swapped_for_a_link_after_the_opening_stops_every_write_and_removal() {
    let fixture = Fixture::new();
    let (root, ids) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let mut library = open(&root).expect("opens");
    let cache = root.join(CACHE_DIR_NAME);
    let moved = fixture.path().join("cache-moved");
    fs::rename(&cache, &moved).expect("moved");
    symlink(&witness, &cache).expect("link");
    let before = snapshot(&witness);
    let flats = FlatFiles::new();

    let pasted = library.save_pasted_image("Pasted", "png", b"png").err();
    assert!(pasted.as_ref().is_some_and(refused), "{pasted:?}");
    let exported = library.export_copies(&ids, &flats).err();
    assert!(exported.as_ref().is_some_and(refused), "{exported:?}");
    let copied = library.clipboard_copies(&ids, &flats).err();
    assert!(copied.as_ref().is_some_and(refused), "{copied:?}");
    let opened = library.opening_copy(ids[0], &flats).err();
    assert!(opened.as_ref().is_some_and(refused), "{opened:?}");
    let cleared = library.clear_thumbnail_cache().err();
    assert!(cleared.as_ref().is_some_and(refused), "{cleared:?}");
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![ids[0]],
            trashed: true,
        })
        .expect("trashed");
    library.empty_trash().expect("emptied");

    assert_eq!(snapshot(&witness), before);
    stays_a_link(&cache, &witness);
}

#[test]
fn an_entry_swapped_for_a_link_after_the_opening_is_never_followed_or_replaced() {
    for entry in ["export", "clipboard", "open", "pasted", "thumbnails"] {
        let fixture = Fixture::new();
        let (root, ids) = fixture.library_with_assets();
        let witness = witness_in(fixture.path());
        let library = open(&root).expect("opens");
        let link = root.join(CACHE_DIR_NAME).join(entry);
        Fixture::replace_with_link(&link, &witness);
        let before = snapshot(&witness);
        let flats = FlatFiles::new();
        let outcome = match entry {
            "export" => library.export_copies(&ids, &flats).err(),
            "clipboard" => library.clipboard_copies(&ids, &flats).err(),
            "open" => library.opening_copy(ids[0], &flats).err(),
            "pasted" => library.save_pasted_image("Pasted", "png", b"png").err(),
            _ => library.clear_thumbnail_cache().err(),
        };
        assert!(
            outcome.as_ref().is_some_and(refused),
            "{entry}: {outcome:?}"
        );
        assert_eq!(snapshot(&witness), before, "{entry}");
        stays_a_link(&link, &witness);
    }
}

#[test]
fn an_opening_folder_that_is_a_link_is_refused_and_kept() {
    let fixture = Fixture::new();
    let (root, ids) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let library = open(&root).expect("opens");
    let link = root
        .join(CACHE_DIR_NAME)
        .join("open")
        .join(ids[0].to_string());
    Fixture::replace_with_link(&link, &witness);
    let before = snapshot(&witness);
    let outcome = library.opening_copy(ids[0], &FlatFiles::new()).err();
    assert!(outcome.as_ref().is_some_and(refused), "{outcome:?}");
    assert_eq!(snapshot(&witness), before);
    stays_a_link(&link, &witness);
}

#[test]
fn links_left_in_the_working_folders_survive_the_cleanups() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_in(fixture.path());
    let witness_file = witness_file(fixture.path());
    let cache = root.join(CACHE_DIR_NAME);
    let loose = cache.join("export/loose-link.png");
    let in_pasted = cache.join("pasted/inner-link.png");
    fs::create_dir_all(cache.join("export")).expect("export");
    fs::create_dir_all(cache.join("pasted")).expect("pasted");
    symlink(&witness_file, &loose).expect("link");
    symlink(&witness, &in_pasted).expect("link");
    let old_operation = cache.join("export/0000000000000-0000000000000000");
    fs::create_dir_all(&old_operation).expect("operation");
    fs::write(old_operation.join("a.png"), b"a").expect("file");
    symlink(&witness_file, old_operation.join("escape")).expect("link");
    let before = snapshot(&witness);
    drop(open(&root).expect("opens"));
    assert_eq!(snapshot(&witness), before);
    assert_eq!(
        fs::read_to_string(&witness_file).expect("read"),
        "do not touch"
    );
    stays_a_link(&loose, &witness_file);
    stays_a_link(&in_pasted, &witness);
    stays_a_link(&old_operation.join("escape"), &witness_file);
}

const RENDER_KEY: &str = "0123456789abcdef0123456789abcdef";

fn old_file(path: &Path, minutes: u64) {
    let file = fs::OpenOptions::new().write(true).open(path).expect("file");
    file.set_modified(std::time::SystemTime::now() - Duration::from_secs(minutes * 60))
        .expect("age");
}

#[test]
fn the_stale_renders_cleanup_removes_only_plain_files_of_pigoune_and_never_links() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_file(fixture.path());
    let annotated = root.join(CACHE_DIR_NAME).join("annotated");
    fs::create_dir_all(&annotated).expect("annotated");
    let stale = annotated.join(format!("{SOME_ID}-{RENDER_KEY}.png"));
    let stale_partial = annotated.join(format!("{SOME_ID}-{RENDER_KEY}.partial"));
    let stale_unique = annotated.join(format!("{SOME_ID}-{RENDER_KEY}.0123456789abcdef.partial"));
    let foreign = annotated.join("foreign.png");
    let linked = annotated.join(format!("{SOME_ID}-fedcba9876543210fedcba9876543210.png"));
    let linked_partial = annotated.join(format!(
        "{SOME_ID}-fedcba9876543210fedcba9876543210.partial"
    ));
    for file in [&stale, &stale_partial, &stale_unique, &foreign] {
        fs::write(file, b"x").expect("file");
    }
    symlink(&witness, &linked).expect("link");
    symlink(&witness, &linked_partial).expect("link");

    drop(open(&root).expect("opens"));

    assert!(!stale.exists());
    assert!(!stale_partial.exists());
    assert!(!stale_unique.exists());
    assert!(foreign.exists());
    stays_a_link(&linked, &witness);
    stays_a_link(&linked_partial, &witness);
    assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
}

#[test]
fn the_thumbnail_regrouping_moves_plain_files_and_leaves_links_where_they_are() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_file(fixture.path());
    let size = root.join(CACHE_DIR_NAME).join("thumbnails/256");
    fs::create_dir_all(&size).expect("size folder");
    let plain = size.join(format!("{SOME_ID}.png"));
    let other_id = "01a10769-97bf-7319-b05f-99fc751d6380";
    let linked = size.join(format!("{other_id}.png"));
    fs::write(&plain, b"thumbnail").expect("file");
    symlink(&witness, &linked).expect("link");

    drop(open(&root).expect("opens"));

    assert!(!plain.exists());
    assert_eq!(
        fs::read(size.join("7f").join(format!("{SOME_ID}.png"))).expect("moved"),
        b"thumbnail"
    );
    stays_a_link(&linked, &witness);
    assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
}

#[test]
fn deleting_an_asset_for_good_removes_its_plain_cache_files_and_never_a_link() {
    let fixture = Fixture::new();
    let (root, ids) = fixture.library_with_assets();
    let witness = witness_file(fixture.path());
    let mut library = open(&root).expect("opens");
    let thumbnail = library.thumbnail_file(ids[0], 256);
    let linked_thumbnail = library.thumbnail_file(ids[1], 256);
    for file in [&thumbnail, &linked_thumbnail] {
        fs::create_dir_all(file.parent().expect("bucket")).expect("bucket");
    }
    fs::write(&thumbnail, b"thumbnail").expect("thumbnail");
    symlink(&witness, &linked_thumbnail).expect("link");
    let annotated = root.join(CACHE_DIR_NAME).join("annotated");
    fs::create_dir_all(&annotated).expect("annotated");
    let render = annotated.join(format!("{}-{RENDER_KEY}.png", ids[0]));
    let linked_render = annotated.join(format!("{}-{RENDER_KEY}.png", ids[1]));
    let render_partial =
        annotated.join(format!("{}-{RENDER_KEY}.0123456789abcdef.partial", ids[0]));
    fs::write(&render, b"render").expect("render");
    fs::write(&render_partial, b"partial").expect("partial");
    symlink(&witness, &linked_render).expect("link");
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: ids.clone(),
            trashed: true,
        })
        .expect("trashed");

    assert_eq!(library.empty_trash().expect("emptied"), 2);

    assert!(!thumbnail.exists());
    assert!(!render.exists());
    assert!(!render_partial.exists());
    stays_a_link(&linked_thumbnail, &witness);
    stays_a_link(&linked_render, &witness);
    assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
}

#[test]
fn clearing_the_thumbnails_removes_plain_files_keeps_links_and_names_the_first_one() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_file(fixture.path());
    let library = open(&root).expect("opens");
    let size = root.join(CACHE_DIR_NAME).join("thumbnails/256/ab");
    fs::create_dir_all(&size).expect("bucket");
    let plain = size.join("plain.png");
    let linked = size.join("linked.png");
    fs::write(&plain, b"thumbnail").expect("file");
    symlink(&witness, &linked).expect("link");

    let error = library
        .clear_thumbnail_cache()
        .expect_err("a link is reported");

    assert!(refused(&error), "{error:?}");
    assert!(format!("{error}").contains("linked.png"), "{error}");
    assert!(!plain.exists());
    stays_a_link(&linked, &witness);
    assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
}

#[test]
fn a_paste_works_while_a_link_stays_in_the_pasted_folder() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let witness = witness_file(fixture.path());
    let library = open(&root).expect("opens");
    let pasted = root.join(CACHE_DIR_NAME).join("pasted");
    fs::create_dir_all(&pasted).expect("pasted");
    let linked = pasted.join("Pasted image.png");
    symlink(&witness, &linked).expect("link");

    let path = library
        .save_pasted_image("Pasted image", "png", b"png")
        .expect("paste");

    assert_ne!(path, linked);
    assert_eq!(fs::read(&path).expect("read"), b"png");
    stays_a_link(&linked, &witness);
    assert_eq!(fs::read_to_string(&witness).expect("read"), "do not touch");
}

#[test]
fn an_old_ordinary_working_folder_is_still_cleaned_after_an_interruption() {
    let fixture = Fixture::new();
    let (root, _) = fixture.library_with_assets();
    let cache = root.join(CACHE_DIR_NAME);
    let export = cache.join("export");
    fs::create_dir_all(&export).expect("export");
    let unfinished = export.join(".0000000000000-0000000000000000.partial");
    fs::create_dir_all(&unfinished).expect("staging");
    fs::write(unfinished.join("a.png"), b"a").expect("file");
    fs::write(export.join(".0000000000000-0000000000000000.bytes"), b"1").expect("size");
    let expired = export.join("0000000000000-1111111111111111");
    fs::create_dir_all(&expired).expect("operation");
    fs::write(expired.join("a.png"), b"a").expect("file");
    old_file(&expired.join("a.png"), 3);

    drop(open(&root).expect("opens"));

    assert!(!unfinished.exists());
    assert!(!expired.exists());
    assert!(
        !export
            .join(".0000000000000-0000000000000000.bytes")
            .exists()
    );
}
