use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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

    fn copy(&self, assets: &[AssetId]) -> Vec<PathBuf> {
        self.library
            .clipboard_copies(assets, &FlatFiles::new())
            .expect("copies are prepared")
    }

    fn clipboard_folder(&self) -> PathBuf {
        self.library.root().join("cache").join("clipboard")
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

fn visible_children(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn hidden_children(folder: &Path) -> Vec<String> {
    fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with('.') && name.ends_with(".partial"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn an_annotated_image_copied_first_never_turns_into_the_original_afterwards() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.rename(id, "pair");
    fixture.annotate(id);
    let mut flats = FlatFiles::new();
    flats.insert(id, fixture.write_flat("flat.png", b"flattened and safe"));
    let promised = fixture
        .library
        .clipboard_copies(&[id], &flats)
        .expect("the flattened copy is prepared")
        .remove(0);

    fixture.remove_annotations(id);
    let later = fixture.copy(&[id]).remove(0);

    assert_ne!(promised, later);
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
fn two_images_with_the_same_name_stay_independent_from_one_copy_to_the_next() {
    let mut fixture = Fixture::new();
    let annotated = fixture.import("red-dot.png");
    let plain = fixture.import("blinking.png");
    fixture.rename(annotated, "pair");
    fixture.rename(plain, "pair");
    fixture.annotate(annotated);
    let mut flats = FlatFiles::new();
    flats.insert(
        annotated,
        fixture.write_flat("flat.png", b"flattened and safe"),
    );
    let promised = fixture
        .library
        .clipboard_copies(&[annotated], &flats)
        .expect("the flattened copy is prepared")
        .remove(0);

    let other = fixture.copy(&[plain]).remove(0);

    assert_eq!(file_name(&promised), file_name(&other));
    assert_ne!(promised, other);
    assert_eq!(
        fs::read(&promised).expect("promised is read"),
        b"flattened and safe"
    );
    assert_eq!(
        fs::read(&other).expect("other is read"),
        fs::read(sample_file("blinking.png")).expect("fixture")
    );
}

#[test]
fn a_selection_of_identical_names_is_never_swapped_by_a_later_copy() {
    let mut fixture = Fixture::new();
    let first = fixture.import("dark-circle.svg");
    let second = fixture.import("blue-photo.jpg");
    let third = fixture.import("red-dot.png");
    let fourth = fixture.import("blinking.png");
    for id in [first, second, third, fourth] {
        fixture.rename(id, "dup");
    }
    let before = fixture.copy(&[first, second, third, fourth]);
    let contents: Vec<Vec<u8>> = before
        .iter()
        .map(|path| fs::read(path).expect("read"))
        .collect();

    fixture.copy(&[fourth]);
    fixture.copy(&[third, fourth]);

    for (path, expected) in before.iter().zip(&contents) {
        assert_eq!(&fs::read(path).expect("still readable"), expected);
    }
}

#[test]
fn a_smaller_copy_after_a_larger_one_leaves_every_earlier_path_readable() {
    let mut fixture = Fixture::new();
    let ids: Vec<AssetId> = [
        "dark-circle.svg",
        "blue-photo.jpg",
        "red-dot.png",
        "blinking.png",
    ]
    .into_iter()
    .map(|name| fixture.import(name))
    .collect();
    let large = fixture.copy(&ids);

    fixture.copy(&ids[..1]);

    for path in &large {
        assert!(path.exists());
    }
}

#[test]
fn a_refused_copy_changes_nothing_that_the_previous_copy_needs() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    let previous = fixture.copy(&[id]);
    fixture.annotate(id);
    let before = visible_children(&fixture.clipboard_folder());

    let refused = fixture.library.clipboard_copies(&[id], &FlatFiles::new());

    assert!(refused.is_err());
    assert_eq!(
        fs::read(&previous[0]).expect("previous is read"),
        fs::read(sample_file("red-dot.png")).expect("fixture")
    );
    assert_eq!(visible_children(&fixture.clipboard_folder()), before);
    assert!(hidden_children(&fixture.clipboard_folder()).is_empty());
}

#[test]
fn a_copy_that_fails_halfway_changes_nothing_that_the_previous_copy_needs() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("blue-photo.jpg");
    let previous = fixture.copy(&[first]);
    let before = visible_children(&fixture.clipboard_folder());
    let asset = fixture.library.asset(second).expect("read").expect("asset");
    let stored = fixture.library.file_of(&asset);
    fs::set_permissions(&stored, fs::Permissions::from_mode(0o000)).expect("locked");

    let failed = fixture
        .library
        .clipboard_copies(&[first, second], &FlatFiles::new());

    fs::set_permissions(&stored, fs::Permissions::from_mode(0o644)).expect("unlocked");
    assert!(failed.is_err());
    assert!(previous[0].exists());
    assert_eq!(visible_children(&fixture.clipboard_folder()), before);
    assert!(hidden_children(&fixture.clipboard_folder()).is_empty());
}

#[test]
fn an_annotated_image_without_its_render_is_refused_and_never_replaced_by_its_original() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.annotate(id);

    let refused = fixture.library.clipboard_copies(&[id], &FlatFiles::new());

    assert!(refused.is_err());
    assert!(visible_children(&fixture.clipboard_folder()).is_empty());
}

#[test]
fn the_names_inside_a_copy_are_the_same_as_before() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let jpg = fixture.import("blue-photo.jpg");
    fixture.rename(svg, "Logo");
    fixture.rename(jpg, "Logo");

    let names: Vec<String> = fixture
        .copy(&[svg, jpg])
        .iter()
        .map(|path| file_name(path))
        .collect();

    assert_eq!(names, ["Logo.svg", "Logo.jpg"]);
}

#[test]
fn many_successive_copies_all_stay_readable() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let copies: Vec<PathBuf> = (0..6).map(|_| fixture.copy(&[png]).remove(0)).collect();

    for path in &copies {
        assert!(path.exists());
    }
    let distinct: std::collections::HashSet<&PathBuf> = copies.iter().collect();
    assert_eq!(distinct.len(), copies.len());
}

#[test]
fn the_clipboard_folders_are_private_to_the_user() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let copy = fixture.copy(&[png]).remove(0);

    let root_mode = fs::metadata(fixture.clipboard_folder())
        .expect("root")
        .permissions()
        .mode();
    let operation_mode = fs::metadata(copy.parent().expect("operation"))
        .expect("operation")
        .permissions()
        .mode();

    assert_eq!(root_mode & 0o777, 0o700);
    assert_eq!(operation_mode & 0o777, 0o700);
}

#[test]
fn a_link_in_place_of_the_clipboard_folder_is_replaced_and_never_followed() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let elsewhere = fixture.workspace.path().join("elsewhere");
    fs::create_dir(&elsewhere).expect("folder");
    fs::write(elsewhere.join("keep.txt"), b"keep").expect("file");
    let folder = fixture.clipboard_folder();
    let _ = fs::remove_dir_all(&folder);
    std::os::unix::fs::symlink(&elsewhere, &folder).expect("link");

    let copy = fixture.copy(&[png]).remove(0);

    assert!(copy.exists());
    assert!(elsewhere.join("keep.txt").exists());
    assert_eq!(visible_children(&elsewhere), ["keep.txt"]);
}

#[test]
fn copies_never_touch_the_library_files_even_after_many_operations() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let asset = fixture.library.asset(png).expect("read").expect("asset");
    let stored = fixture.library.file_of(&asset);
    let before = fs::read(&stored).expect("original is read");

    for _ in 0..4 {
        let copy = fixture.copy(&[png]).remove(0);
        fs::write(&copy, b"changed elsewhere").expect("copy is changed");
    }

    assert_eq!(fs::read(&stored).expect("original is read"), before);
}

#[test]
fn a_reopening_keeps_a_recent_copy_and_removes_the_ones_older_than_a_day() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let recent = fixture.copy(&[png]).remove(0);
    let old = fixture.copy(&[png]).remove(0);
    let folder = fixture.clipboard_folder();
    let old_name = file_name(old.parent().expect("operation"));
    let stale_name = format!("{:013x}-{:016x}", 1_000_u64, 9_u64);
    fs::rename(folder.join(&old_name), folder.join(&stale_name)).expect("renamed");
    fs::rename(
        folder.join(format!(".{old_name}.bytes")),
        folder.join(format!(".{stale_name}.bytes")),
    )
    .expect("size renamed");
    fs::create_dir(folder.join(format!(".{:013x}-{:016x}.partial", 2_u64, 3_u64)))
        .expect("unfinished work");
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let _reopened = Library::open(&root).expect("library reopens");

    assert!(recent.exists());
    assert!(!folder.join(&stale_name).exists());
    assert!(hidden_children(&folder).is_empty());
}
