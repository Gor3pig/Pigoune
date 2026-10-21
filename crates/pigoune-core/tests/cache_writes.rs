use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use pigoune_core::{LibraryError, create_cache_folder, write_render_file, write_thumbnail_file};
use tempfile::TempDir;

const ID: &str = "01a10769-97bf-7319-b05f-99fc751d637f";
const KEY: &str = "0123456789abcdef0123456789abcdef";
const RANDOM: &str = "0123456789abcdef";

struct Workspace {
    folder: TempDir,
}

impl Workspace {
    fn new() -> Self {
        Self {
            folder: tempfile::tempdir().expect("workspace"),
        }
    }

    fn thumbnails(&self) -> PathBuf {
        let folder = self
            .folder
            .path()
            .join("Lib.pigoune/cache/thumbnails/256/7f");
        fs::create_dir_all(&folder).expect("folders");
        folder
    }

    fn annotated(&self) -> PathBuf {
        let folder = self.folder.path().join("Lib.pigoune/cache/annotated");
        fs::create_dir_all(&folder).expect("folders");
        folder
    }

    fn witness(&self) -> PathBuf {
        let witness = self.folder.path().join("witness.bin");
        fs::write(&witness, "do not touch").expect("witness");
        witness
    }

    fn witness_folder(&self) -> PathBuf {
        let folder = self.folder.path().join("witness-folder");
        fs::create_dir_all(&folder).expect("folder");
        fs::write(folder.join("keep.txt"), "keep").expect("file");
        folder
    }
}

fn refused(result: &Result<(), LibraryError>) -> bool {
    matches!(result, Err(LibraryError::UnsafeCache(_)))
}

fn age(path: &Path, minutes: u64) {
    let file = fs::OpenOptions::new().write(true).open(path).expect("file");
    file.set_modified(SystemTime::now() - Duration::from_secs(minutes * 60))
        .expect("age");
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .expect("folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn untouched(witness: &Path) {
    assert_eq!(
        fs::read_to_string(witness).expect("witness"),
        "do not touch"
    );
}

#[test]
fn a_thumbnail_is_written_whole_in_its_folders_and_replaces_an_ordinary_older_one() {
    let workspace = Workspace::new();
    let destination = workspace.thumbnails().join(format!("{ID}.png"));

    write_thumbnail_file(&destination, b"first").expect("written");
    write_thumbnail_file(&destination, b"second").expect("replaced");

    assert_eq!(fs::read(&destination).expect("read"), b"second");
    assert_eq!(
        names_in(destination.parent().expect("bucket")),
        [format!("{ID}.png")]
    );
}

#[test]
fn a_thumbnail_creates_its_missing_folders_but_never_outside_a_cache_folder() {
    let workspace = Workspace::new();
    fs::create_dir(workspace.folder.path().join("Lib.pigoune")).expect("library");
    let inside = workspace
        .folder
        .path()
        .join("Lib.pigoune/cache/thumbnails/512/aa/x.png");
    write_thumbnail_file(&inside, b"content").expect("written");
    assert_eq!(fs::read(&inside).expect("read"), b"content");

    let outside = workspace.folder.path().join("elsewhere/thumbnails/x.png");
    assert!(write_thumbnail_file(&outside, b"content").is_err());
    assert!(!workspace.folder.path().join("elsewhere").exists());
}

#[test]
fn a_destination_that_is_a_link_is_refused_and_neither_the_link_nor_its_target_changes() {
    let workspace = Workspace::new();
    let witness = workspace.witness();
    let destination = workspace.thumbnails().join(format!("{ID}.png"));
    symlink(&witness, &destination).expect("link");

    let result = write_thumbnail_file(&destination, b"new content");

    assert!(refused(&result), "{result:?}");
    assert_eq!(fs::read_link(&destination).expect("link"), witness);
    untouched(&witness);
    assert_eq!(
        names_in(destination.parent().expect("bucket")),
        [format!("{ID}.png")]
    );
}

#[test]
fn a_render_destination_that_is_a_link_is_refused_the_same_way() {
    let workspace = Workspace::new();
    let witness = workspace.witness();
    let destination = workspace.annotated().join(format!("{ID}-{KEY}.png"));
    symlink(&witness, &destination).expect("link");

    let result = write_render_file(&destination, b"new content");

    assert!(refused(&result), "{result:?}");
    assert_eq!(fs::read_link(&destination).expect("link"), witness);
    untouched(&witness);
    assert_eq!(
        names_in(&workspace.annotated()),
        [format!("{ID}-{KEY}.png")]
    );
}

#[test]
fn a_historic_temporary_name_that_is_a_link_is_refused_and_left_alone() {
    let workspace = Workspace::new();
    let witness = workspace.witness();
    let folder = workspace.thumbnails();
    let destination = folder.join(format!("{ID}.png"));
    let temporary = folder.join(format!(".{ID}.png.partial"));
    symlink(&witness, &temporary).expect("link");

    let result = write_thumbnail_file(&destination, b"new content");

    assert!(refused(&result), "{result:?}");
    assert_eq!(fs::read_link(&temporary).expect("link"), witness);
    untouched(&witness);
    assert!(!destination.exists());
}

#[test]
fn a_unique_temporary_name_that_is_a_link_is_refused_and_left_alone() {
    let workspace = Workspace::new();
    let witness = workspace.witness();
    let folder = workspace.thumbnails();
    let destination = folder.join(format!("{ID}.png"));
    let temporary = folder.join(format!(".{ID}.png.{RANDOM}.partial"));
    symlink(&witness, &temporary).expect("link");

    let result = write_thumbnail_file(&destination, b"new content");

    assert!(refused(&result), "{result:?}");
    assert_eq!(fs::read_link(&temporary).expect("link"), witness);
    untouched(&witness);
    assert!(!destination.exists());
}

#[test]
fn a_render_temporary_that_is_a_link_is_refused_and_left_alone() {
    let workspace = Workspace::new();
    let witness = workspace.witness();
    let folder = workspace.annotated();
    let destination = folder.join(format!("{ID}-{KEY}.png"));
    for name in [
        format!("{ID}-{KEY}.partial"),
        format!("{ID}-{KEY}.{RANDOM}.partial"),
    ] {
        let temporary = folder.join(&name);
        symlink(&witness, &temporary).expect("link");

        let result = write_render_file(&destination, b"new content");

        assert!(refused(&result), "{name}: {result:?}");
        assert_eq!(fs::read_link(&temporary).expect("link"), witness);
        untouched(&witness);
        fs::remove_file(&temporary).expect("link removed by the test");
    }
}

#[test]
fn an_old_ordinary_temporary_left_by_an_interruption_is_cleaned_and_the_write_succeeds() {
    let workspace = Workspace::new();
    let folder = workspace.thumbnails();
    let destination = folder.join(format!("{ID}.png"));
    let historic = folder.join(format!(".{ID}.png.partial"));
    let unique = folder.join(format!(".{ID}.png.{RANDOM}.partial"));
    for file in [&historic, &unique] {
        fs::write(file, b"half").expect("leftover");
        age(file, 30);
    }

    write_thumbnail_file(&destination, b"content").expect("written");

    assert_eq!(fs::read(&destination).expect("read"), b"content");
    assert_eq!(names_in(&folder), [format!("{ID}.png")]);
}

#[test]
fn an_old_ordinary_render_temporary_is_cleaned_too() {
    let workspace = Workspace::new();
    let folder = workspace.annotated();
    let destination = folder.join(format!("{ID}-{KEY}.png"));
    let historic = folder.join(format!("{ID}-{KEY}.partial"));
    let unique = folder.join(format!("{ID}-{KEY}.{RANDOM}.partial"));
    for file in [&historic, &unique] {
        fs::write(file, b"half").expect("leftover");
        age(file, 30);
    }

    write_render_file(&destination, b"content").expect("written");

    assert_eq!(names_in(&folder), [format!("{ID}-{KEY}.png")]);
}

#[test]
fn a_recent_temporary_and_the_temporary_of_another_asset_are_left_and_the_write_still_succeeds() {
    let workspace = Workspace::new();
    let folder = workspace.thumbnails();
    let destination = folder.join(format!("{ID}.png"));
    let recent = folder.join(format!(".{ID}.png.{RANDOM}.partial"));
    let other_id = "01a10769-97bf-7319-b05f-99fc751d6380";
    let other = folder.join(format!(".{other_id}.png.partial"));
    fs::write(&recent, b"being written").expect("recent");
    fs::write(&other, b"someone else").expect("other");
    age(&other, 600);

    write_thumbnail_file(&destination, b"content").expect("written");

    assert_eq!(fs::read(&destination).expect("read"), b"content");
    assert_eq!(fs::read(&recent).expect("recent kept"), b"being written");
    assert_eq!(fs::read(&other).expect("other kept"), b"someone else");
}

#[test]
fn a_folder_that_is_a_link_on_the_way_is_refused_and_the_target_is_not_touched() {
    let workspace = Workspace::new();
    let target = workspace.witness_folder();
    let cache = workspace.folder.path().join("Lib.pigoune/cache");
    fs::create_dir_all(cache.join("thumbnails")).expect("thumbnails");
    symlink(&target, cache.join("thumbnails/256")).expect("link");
    let destination = cache.join(format!("thumbnails/256/7f/{ID}.png"));

    let result = write_thumbnail_file(&destination, b"content");

    assert!(refused(&result), "{result:?}");
    assert_eq!(names_in(&target), ["keep.txt"]);

    fs::remove_file(cache.join("thumbnails/256")).expect("link removed by the test");
    fs::create_dir_all(cache.join("thumbnails/256")).expect("size");
    symlink(&target, cache.join("thumbnails/256/7f")).expect("link");
    let result = write_thumbnail_file(&destination, b"content");
    assert!(refused(&result), "{result:?}");
    assert_eq!(names_in(&target), ["keep.txt"]);

    let linked_cache = workspace.folder.path().join("Other.pigoune/cache");
    fs::create_dir_all(linked_cache.parent().expect("library")).expect("library");
    symlink(&target, &linked_cache).expect("link");
    let result = create_cache_folder(&linked_cache.join("thumbnails/256"));
    assert!(refused(&result), "{result:?}");
    assert_eq!(names_in(&target), ["keep.txt"]);
}

#[test]
fn several_writers_of_the_same_thumbnail_never_leave_a_partial_file() {
    let workspace = Workspace::new();
    let destination = Arc::new(workspace.thumbnails().join(format!("{ID}.png")));
    let writers: Vec<_> = (0..8)
        .map(|number| {
            let destination = Arc::clone(&destination);
            thread::spawn(move || {
                let content = vec![u8::try_from(number).expect("small"); 4096];
                write_thumbnail_file(&destination, &content).expect("written");
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer");
    }

    let content = fs::read(&*destination).expect("read");
    assert_eq!(content.len(), 4096);
    assert!(content.iter().all(|byte| *byte == content[0]));
    assert_eq!(
        names_in(destination.parent().expect("bucket")),
        [format!("{ID}.png")]
    );
}
