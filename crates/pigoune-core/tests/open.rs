use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pigoune_core::{AssetCommand, AssetId, FlatFiles, ImportOutcome, Library};
use tempfile::TempDir;

struct Fixture {
    workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self { workspace, library }
    }

    fn import(&mut self, name: &str) -> AssetId {
        match self
            .library
            .import_file(&sample_file(name), None)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn rename(&mut self, asset: AssetId, name: &str) {
        self.library
            .apply_asset_command(&AssetCommand::Rename {
                asset,
                name: name.to_owned(),
            })
            .expect("resource is renamed");
    }

    fn open(&self, asset: AssetId) -> PathBuf {
        self.library
            .opening_copy(asset, &FlatFiles::new())
            .expect("opening copy is prepared")
            .expect("asset exists")
    }

    fn open_with(&self, asset: AssetId, flat: &Path) -> PathBuf {
        let mut flats = FlatFiles::new();
        flats.insert(asset, flat.to_path_buf());
        self.library
            .opening_copy(asset, &flats)
            .expect("opening copy is prepared")
            .expect("asset exists")
    }

    fn opening_folder(&self) -> PathBuf {
        self.library.root().join("cache").join("open")
    }

    fn annotate(&mut self, id: AssetId) {
        let shape = pigoune_core::annotations::Shape::new(
            "rectangle",
            pigoune_core::annotations::Geometry::Box(pigoune_core::annotations::Rect {
                x: 0.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            }),
            pigoune_core::annotations::Options::new(),
        );
        self.library
            .save_annotations(
                id,
                &pigoune_core::annotations::AnnotationDocument::new(
                    None,
                    vec![pigoune_core::annotations::AnnotationObject::new(1, shape)],
                ),
            )
            .expect("annotated");
    }

    fn remove_annotations(&mut self, id: AssetId) {
        self.library
            .save_annotations(
                id,
                &pigoune_core::annotations::AnnotationDocument::new(None, Vec::new()),
            )
            .expect("annotations removed");
    }

    fn write_flat(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.workspace.path().join(name);
        fs::write(&path, contents).expect("flat file is written");
        path
    }
}

fn sample_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("copy has a name")
        .to_string_lossy()
        .into_owned()
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn set_age(path: &Path, hours: u64) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("file opens");
    file.set_modified(SystemTime::now() - Duration::from_secs(hours * 3600))
        .expect("age is set");
}

#[test]
fn an_annotated_image_opened_first_never_turns_into_the_original_afterwards() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.rename(id, "pair");
    fixture.annotate(id);
    let flat = fixture.write_flat("flat.png", b"flattened and safe");
    let promised = fixture.open_with(id, &flat);

    fixture.remove_annotations(id);
    let later = fixture.open(id);

    assert_ne!(promised, later);
    assert_eq!(file_name(&promised), file_name(&later));
    assert_eq!(
        fs::read(&promised).expect("promised is read"),
        b"flattened and safe"
    );
    assert_eq!(
        fs::read(&later).expect("later is read"),
        fs::read(sample_file("red-dot.png")).expect("fixture")
    );
}

#[test]
fn an_original_opened_first_keeps_its_content_when_the_image_is_annotated_later() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.rename(id, "pair");
    let promised = fixture.open(id);

    fixture.annotate(id);
    let flat = fixture.write_flat("flat.png", b"flattened and safe");
    let later = fixture.open_with(id, &flat);

    assert_ne!(promised, later);
    assert_eq!(
        fs::read(&promised).expect("promised is read"),
        fs::read(sample_file("red-dot.png")).expect("fixture")
    );
    assert_eq!(
        fs::read(&later).expect("later is read"),
        b"flattened and safe"
    );
}

#[test]
fn two_images_with_the_same_name_never_share_a_path_or_a_content() {
    let mut fixture = Fixture::new();
    let annotated = fixture.import("red-dot.png");
    let plain = fixture.import("blinking.png");
    fixture.rename(annotated, "pair");
    fixture.rename(plain, "pair");
    fixture.annotate(annotated);
    let flat = fixture.write_flat("flat.png", b"flattened and safe");

    let first = fixture.open_with(annotated, &flat);
    let second = fixture.open(plain);

    assert_eq!(file_name(&first), file_name(&second));
    assert_ne!(first.parent(), second.parent());
    assert_eq!(fs::read(&first).expect("first"), b"flattened and safe");
    assert_eq!(
        fs::read(&second).expect("second"),
        fs::read(sample_file("blinking.png")).expect("fixture")
    );
}

#[test]
fn the_path_follows_the_asset_folder_then_a_folder_of_its_own() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");

    let copy = fixture.open(png);

    let operation = copy.parent().expect("operation folder");
    let asset_folder = operation.parent().expect("asset folder");
    assert_eq!(file_name(asset_folder), png.to_string());
    assert_eq!(
        asset_folder.parent(),
        Some(fixture.opening_folder().as_path())
    );
    assert_ne!(file_name(operation), png.to_string());
}

#[test]
fn many_quick_openings_of_one_asset_all_stay_readable_at_distinct_paths() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let copies: Vec<PathBuf> = (0..8).map(|_| fixture.open(png)).collect();

    let distinct: std::collections::HashSet<&PathBuf> = copies.iter().collect();
    assert_eq!(distinct.len(), copies.len());
    for path in &copies {
        assert_eq!(
            fs::read(path).expect("still readable"),
            fs::read(sample_file("red-dot.png")).expect("fixture")
        );
    }
}

#[test]
fn a_refused_opening_changes_nothing_that_the_previous_copy_needs() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    let previous = fixture.open(id);
    fixture.annotate(id);
    let asset_folder = previous
        .parent()
        .and_then(Path::parent)
        .expect("asset folder")
        .to_path_buf();
    let before = names_in(&asset_folder);

    let refused = fixture.library.opening_copy(id, &FlatFiles::new());

    assert!(refused.is_err());
    assert_eq!(
        fs::read(&previous).expect("previous is read"),
        fs::read(sample_file("red-dot.png")).expect("fixture")
    );
    assert_eq!(names_in(&asset_folder), before);
}

#[test]
fn an_opening_that_fails_halfway_changes_nothing_that_the_previous_copy_needs() {
    let mut fixture = Fixture::new();
    let id = fixture.import("blue-photo.jpg");
    let previous = fixture.open(id);
    let asset = fixture.library.asset(id).expect("read").expect("asset");
    let stored = fixture.library.file_of(&asset);
    let asset_folder = previous
        .parent()
        .and_then(Path::parent)
        .expect("asset folder")
        .to_path_buf();
    let before = names_in(&asset_folder);
    fs::set_permissions(&stored, fs::Permissions::from_mode(0o000)).expect("locked");

    let failed = fixture.library.opening_copy(id, &FlatFiles::new());

    fs::set_permissions(&stored, fs::Permissions::from_mode(0o644)).expect("unlocked");
    assert!(failed.is_err());
    assert!(previous.exists());
    assert_eq!(names_in(&asset_folder), before);
}

#[test]
fn an_annotated_image_without_its_render_is_refused_and_never_replaced_by_its_original() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.annotate(id);

    let refused = fixture.library.opening_copy(id, &FlatFiles::new());

    assert!(refused.is_err());
    assert!(names_in(&fixture.opening_folder()).is_empty());
}

#[test]
fn a_reopening_keeps_the_copies_and_the_next_opening_takes_a_new_path() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    let before = fixture.open(id);
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let reopened = Library::open(&root).expect("library reopens");
    let after = reopened
        .opening_copy(id, &FlatFiles::new())
        .expect("opening copy")
        .expect("asset exists");

    assert!(before.exists());
    assert_ne!(before, after);
    drop(fixture.workspace);
}

#[test]
fn a_reopening_removes_what_is_older_than_a_day_and_keeps_the_rest() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    let recent = fixture.open(id);
    let old = fixture.open(id);
    let asset_folder = recent
        .parent()
        .and_then(Path::parent)
        .expect("asset folder")
        .to_path_buf();
    let old_name = file_name(old.parent().expect("operation"));
    let stale_name = format!("{:013x}-{:016x}", 1_000_u64, 9_u64);
    fs::rename(asset_folder.join(&old_name), asset_folder.join(&stale_name)).expect("renamed");
    fs::rename(
        asset_folder.join(format!(".{old_name}.bytes")),
        asset_folder.join(format!(".{stale_name}.bytes")),
    )
    .expect("size renamed");
    fs::create_dir(asset_folder.join(format!(".{:013x}-{:016x}.partial", 2_u64, 3_u64)))
        .expect("unfinished work");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let _reopened = Library::open(&root).expect("library reopens");

    assert!(recent.exists());
    assert!(!asset_folder.join(&stale_name).exists());
    assert!(
        !names_in(&asset_folder)
            .iter()
            .any(|name| name.ends_with(".partial"))
    );
}

#[test]
fn copies_of_the_previous_layout_are_kept_while_recent_and_removed_when_old() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    let asset_folder = fixture.opening_folder().join(id.to_string());
    fs::create_dir_all(&asset_folder).expect("folder");
    let recent = asset_folder.join("recent.png");
    let old = asset_folder.join("old.png");
    fs::write(&recent, b"recent").expect("recent");
    fs::write(&old, b"old").expect("old");
    set_age(&old, 30);
    let empty_folder = fixture.opening_folder().join(
        AssetId::parse("01a15389-0f99-7438-9ab3-9d143a65c533")
            .expect("id")
            .to_string(),
    );
    fs::create_dir_all(&empty_folder).expect("empty folder");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let _reopened = Library::open(&root).expect("library reopens");

    assert!(recent.exists());
    assert!(!old.exists());
    assert!(!empty_folder.exists());
}

#[test]
fn the_opening_folders_are_private_to_the_user() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let copy = fixture.open(png);

    let operation = copy.parent().expect("operation");
    let asset_folder = operation.parent().expect("asset folder");
    for folder in [
        operation,
        asset_folder,
        asset_folder.parent().expect("opening"),
    ] {
        let mode = fs::metadata(folder).expect("folder").permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
    }
}

#[test]
fn a_link_in_place_of_the_asset_folder_is_refused_and_left_alone() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let elsewhere = fixture.workspace.path().join("elsewhere");
    fs::create_dir(&elsewhere).expect("folder");
    fs::write(elsewhere.join("keep.txt"), b"keep").expect("file");
    fs::create_dir_all(fixture.opening_folder()).expect("opening folder");
    let link = fixture.opening_folder().join(png.to_string());
    symlink(&elsewhere, &link).expect("link");

    let refused = fixture
        .library
        .opening_copy(png, &FlatFiles::new())
        .expect_err("a link in place of the asset folder is refused");

    assert!(matches!(
        refused,
        pigoune_core::LibraryError::UnsafeCache(_)
    ));
    assert!(link.is_symlink());
    assert_eq!(names_in(&elsewhere), ["keep.txt"]);
}

#[test]
fn a_cleanup_never_follows_a_link_to_a_folder_outside_the_library() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let elsewhere = fixture.workspace.path().join("elsewhere");
    fs::create_dir(&elsewhere).expect("folder");
    let outside = elsewhere.join("old.png");
    fs::write(&outside, b"outside").expect("file");
    set_age(&outside, 40);
    fs::create_dir_all(fixture.opening_folder()).expect("opening folder");
    symlink(&elsewhere, fixture.opening_folder().join(png.to_string())).expect("link");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let _reopened = Library::open(&root).expect("library reopens");

    assert!(outside.exists());
}

#[test]
fn an_application_changing_its_copy_touches_neither_the_original_nor_the_other_copies() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let asset = fixture.library.asset(png).expect("read").expect("asset");
    let stored = fixture.library.file_of(&asset);
    let original = fs::read(&stored).expect("original is read");
    let first = fixture.open(png);
    let second = fixture.open(png);

    fs::write(&first, b"changed elsewhere").expect("copy is changed");

    assert_eq!(fs::read(&stored).expect("original"), original);
    assert_eq!(fs::read(&second).expect("second"), original);
}
