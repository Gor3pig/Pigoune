use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{Asset, AssetCommand, AssetId, FlatFiles, ImportOutcome, Library};
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

    fn asset(&self, id: AssetId) -> Asset {
        self.library
            .asset(id)
            .expect("asset readable")
            .expect("asset exists")
    }

    fn export(&self, assets: &[AssetId]) -> Vec<PathBuf> {
        self.library
            .export_copies(assets, &FlatFiles::new())
            .expect("copies are prepared")
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

#[test]
fn a_copy_takes_the_display_name_and_the_original_extension() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    fixture.rename(svg, "Logo GitHub");

    let copies = fixture.export(&[svg]);

    assert_eq!(copies.len(), 1);
    assert_eq!(file_name(&copies[0]), "Logo GitHub.svg");
    assert_eq!(
        fs::read(&copies[0]).expect("copy is read"),
        fs::read(sample_file("dark-circle.svg")).expect("original is read")
    );
}

#[test]
fn different_types_keep_the_same_name() {
    let mut fixture = Fixture::new();
    let first = fixture.import("red-dot.png");
    let second = fixture.import("green-square.webp");
    let third = fixture.import("blue-photo.jpg");
    for asset in [first, second, third] {
        fixture.rename(asset, "Image");
    }
    let gif = fixture.import("still.gif");
    fixture.rename(gif, "Image");

    let names: Vec<String> = fixture
        .export(&[first, second, third, gif])
        .iter()
        .map(|copy| file_name(copy))
        .collect();

    assert_eq!(names, ["Image.png", "Image.webp", "Image.jpg", "Image.gif"]);
}

#[test]
fn the_same_name_and_extension_get_a_number() {
    let mut fixture = Fixture::new();
    let first = fixture.import("still.gif");
    let second = fixture.import("spinner.gif");
    fixture.rename(first, "Animation");
    fixture.rename(second, "animation");

    let names: Vec<String> = fixture
        .export(&[first, second])
        .iter()
        .map(|copy| file_name(copy))
        .collect();

    assert_eq!(names, ["Animation.gif", "animation (2).gif"]);
}

#[test]
fn copies_never_touch_the_library_files() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let copy = fixture.export(&[svg]).remove(0);

    fs::write(&copy, b"modified").expect("copy is changed");

    let asset = fixture
        .library
        .asset(svg)
        .expect("asset is read")
        .expect("asset exists");
    assert_eq!(
        fs::read(fixture.library.file_of(&asset)).expect("original is read"),
        fs::read(sample_file("dark-circle.svg")).expect("fixture is read")
    );
}

const HOUR_MS: u64 = 60 * 60 * 1000;

fn export_folder(fixture: &Fixture) -> PathBuf {
    fixture.library.root().join("cache").join("export")
}

fn write_flat(fixture: &Fixture, name: &str, contents: &[u8]) -> PathBuf {
    let path = fixture.workspace.path().join(name);
    fs::write(&path, contents).expect("flat file is written");
    path
}

fn annotate(fixture: &mut Fixture, id: AssetId) {
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
    fixture
        .library
        .save_annotations(
            id,
            &pigoune_core::annotations::AnnotationDocument::new(
                None,
                vec![pigoune_core::annotations::AnnotationObject::new(1, shape)],
            ),
        )
        .expect("annotated");
}

fn remove_annotations(fixture: &mut Fixture, id: AssetId) {
    fixture
        .library
        .save_annotations(
            id,
            &pigoune_core::annotations::AnnotationDocument::new(None, Vec::new()),
        )
        .expect("annotations removed");
}

fn operation_name(started_ms: u64, random: u64) -> String {
    format!("{started_ms:013x}-{random:016x}")
}

fn now_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    )
    .expect("fits")
}

#[test]
fn two_exports_never_share_a_path_and_the_first_keeps_its_files() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    fixture.rename(svg, "same");
    fixture.rename(png, "same");

    let first = fixture.export(&[svg]);
    let second = fixture.export(&[png]);

    assert_ne!(first, second);
    assert!(first[0].exists() && second[0].exists());
    assert_eq!(
        fs::read(&first[0]).expect("first is read"),
        fs::read(sample_file("dark-circle.svg")).expect("fixture")
    );
    assert_eq!(
        fs::read(&second[0]).expect("second is read"),
        fs::read(sample_file("red-dot.png")).expect("fixture")
    );
}

#[test]
fn the_names_inside_an_export_are_the_same_as_before() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    fixture.rename(svg, "Logo");
    fixture.rename(png, "Logo");

    let copies = fixture.export(&[svg, png]);

    assert_eq!(file_name(&copies[0]), "Logo.svg");
    assert_eq!(file_name(&copies[1]), "Logo.png");
    assert_eq!(copies[0].parent(), copies[1].parent());
}

#[test]
fn an_annotated_image_exported_first_never_turns_into_the_original_afterwards() {
    let mut fixture = Fixture::new();
    let id = fixture.import("red-dot.png");
    fixture.rename(id, "pair");
    annotate(&mut fixture, id);
    let flat = write_flat(&fixture, "flat.png", b"flattened and safe");
    let mut flats = FlatFiles::new();
    flats.insert(id, flat);
    let promised = fixture
        .library
        .export_copies(&[id], &flats)
        .expect("the flattened copy is prepared")
        .remove(0);

    remove_annotations(&mut fixture, id);
    let later = fixture.export(&[id]).remove(0);

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
fn two_items_with_the_same_name_never_swap_their_contents_between_exports() {
    let mut fixture = Fixture::new();
    let first = fixture.import("dark-circle.svg");
    let second = fixture.import("blue-photo.jpg");
    let third = fixture.import("red-dot.png");
    for id in [first, second, third] {
        fixture.rename(id, "dup");
    }
    let before = fixture.export(&[first, third, second]);
    let contents: Vec<Vec<u8>> = before
        .iter()
        .map(|path| fs::read(path).expect("read"))
        .collect();

    fixture.export(&[second, first, third]);
    fixture.export(&[third, second, first]);

    for (path, expected) in before.iter().zip(&contents) {
        assert_eq!(&fs::read(path).expect("still readable"), expected);
    }
}

#[test]
fn many_successive_exports_all_stay_readable() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let exports: Vec<PathBuf> = (0..6).map(|_| fixture.export(&[png]).remove(0)).collect();

    for path in &exports {
        assert!(path.exists());
    }
    let distinct: std::collections::HashSet<&PathBuf> = exports.iter().collect();
    assert_eq!(distinct.len(), exports.len());
}

#[test]
fn a_reopening_keeps_recent_exports_and_removes_the_ones_older_than_a_day() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let recent = fixture.export(&[png]).remove(0);
    let folder = export_folder(&fixture);
    let old = folder.join(operation_name(now_ms() - 30 * HOUR_MS, 7));
    let middle = folder.join(operation_name(now_ms() - 5 * HOUR_MS, 8));
    for path in [&old, &middle] {
        fs::create_dir(path).expect("operation");
        fs::write(path.join("a.png"), b"a").expect("file");
    }
    let unfinished = folder.join(format!(".{}.partial", operation_name(now_ms(), 9)));
    fs::create_dir(&unfinished).expect("unfinished work");

    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let _reopened = Library::open(&root).expect("library reopens");

    assert!(recent.exists());
    assert!(middle.join("a.png").exists());
    assert!(!old.exists());
    assert!(!unfinished.exists());
    drop(fixture.workspace);
}

#[test]
fn a_failed_export_leaves_no_trace_and_the_previous_one_intact() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    let previous = fixture.export(&[svg]).remove(0);
    let before: Vec<String> = fs::read_dir(export_folder(&fixture))
        .expect("export folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let asset = fixture.asset(png);
    fs::remove_file(fixture.library.file_of(&asset)).expect("source is lost");

    assert!(
        fixture
            .library
            .export_copies(&[svg, png], &FlatFiles::new())
            .is_err()
    );

    let mut after: Vec<String> = fs::read_dir(export_folder(&fixture))
        .expect("export folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let mut expected = before;
    expected.sort();
    after.sort();
    assert_eq!(after, expected);
    assert_eq!(
        fs::read(&previous).expect("previous is read"),
        fs::read(sample_file("dark-circle.svg")).expect("fixture")
    );
}

#[test]
fn the_export_folders_are_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;

    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let copy = fixture.export(&[png]).remove(0);

    let operation = copy.parent().expect("operation folder");
    for folder in [operation, operation.parent().expect("export folder")] {
        assert_eq!(
            fs::metadata(folder).expect("metadata").permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn exports_never_touch_the_library_files_even_after_many_operations() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let asset = fixture.asset(png);
    let original = fs::read(fixture.library.file_of(&asset)).expect("original");
    for _ in 0..4 {
        let copy = fixture.export(&[png]).remove(0);
        fs::write(&copy, b"modified by the destination").expect("copy is changed");
    }
    assert_eq!(
        fs::read(fixture.library.file_of(&asset)).expect("original"),
        original
    );
}

#[test]
fn exporting_to_a_folder_never_overwrites_a_file() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    fixture.rename(svg, "Logo GitHub");
    let folder = fixture.workspace.path().join("Bureau");
    fs::create_dir(&folder).expect("folder is created");
    fs::write(folder.join("Logo GitHub.svg"), b"mine").expect("file is written");
    fs::write(folder.join("Logo GitHub (2).svg"), b"mine too").expect("file is written");

    let copies = fixture
        .library
        .export_to(&[svg], &folder, &FlatFiles::new())
        .expect("export succeeds")
        .copies;

    assert_eq!(copies, [folder.join("Logo GitHub (3).svg")]);
    assert_eq!(
        fs::read(folder.join("Logo GitHub.svg")).expect("file is read"),
        b"mine"
    );
    assert_eq!(
        fs::read(folder.join("Logo GitHub (2).svg")).expect("file is read"),
        b"mine too"
    );
    assert_eq!(
        fs::read(&copies[0]).expect("copy is read"),
        fs::read(sample_file("dark-circle.svg")).expect("original is read")
    );
}

#[test]
fn several_exports_to_the_same_folder_add_up() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let folder = fixture.workspace.path().join("Exports");
    fs::create_dir(&folder).expect("folder is created");

    fixture
        .library
        .export_to(&[svg], &folder, &FlatFiles::new())
        .expect("first export");
    fixture
        .library
        .export_to(&[svg], &folder, &FlatFiles::new())
        .expect("second export");

    let mut names: Vec<String> = fs::read_dir(&folder)
        .expect("folder is listed")
        .map(|entry| {
            entry
                .expect("entry is read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["dark-circle (2).svg", "dark-circle.svg"]);
}

#[test]
fn exporting_to_a_missing_folder_is_refused() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let missing = fixture.workspace.path().join("absent");

    assert!(
        fixture
            .library
            .export_to(&[svg], &missing, &FlatFiles::new())
            .is_err()
    );
    assert!(!missing.exists());
}

#[test]
fn clipboard_copies_survive_a_new_drag() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    let copied = fixture
        .library
        .clipboard_copies(&[svg], &FlatFiles::new())
        .expect("clipboard copies are prepared")
        .remove(0);

    fixture.export(&[png]);

    assert!(copied.exists());
    assert_eq!(file_name(&copied), "dark-circle.svg");
}

#[test]
fn an_opening_copy_takes_the_display_name() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    fixture.rename(svg, "Logo GitHub");

    let copy = fixture
        .library
        .opening_copy(svg, &FlatFiles::new())
        .expect("opening copy is prepared")
        .expect("asset exists");

    assert_eq!(file_name(&copy), "Logo GitHub.svg");
    assert_eq!(
        fs::read(&copy).expect("copy is read"),
        fs::read(sample_file("dark-circle.svg")).expect("original is read")
    );
}

#[test]
fn opening_copies_stay_until_the_library_is_reopened() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    let first = fixture
        .library
        .opening_copy(svg, &FlatFiles::new())
        .expect("first copy")
        .expect("asset exists");
    let second = fixture
        .library
        .opening_copy(png, &FlatFiles::new())
        .expect("second copy")
        .expect("asset exists");
    fixture.export(&[png]);
    fixture
        .library
        .clipboard_copies(&[svg], &FlatFiles::new())
        .expect("clipboard copies are prepared");

    assert!(first.exists());
    assert!(second.exists());

    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let _reopened = Library::open(&root).expect("library reopens");
    assert!(!first.exists());
    assert!(!second.exists());
    drop(fixture.workspace);
}

#[test]
fn opening_the_same_asset_twice_replaces_its_copy() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let first = fixture
        .library
        .opening_copy(svg, &FlatFiles::new())
        .expect("first copy")
        .expect("asset exists");
    fs::write(&first, b"edited elsewhere").expect("copy is changed");
    fixture.rename(svg, "Cercle");

    let second = fixture
        .library
        .opening_copy(svg, &FlatFiles::new())
        .expect("second copy")
        .expect("asset exists");

    assert!(!first.exists());
    assert_eq!(file_name(&second), "Cercle.svg");
    assert_eq!(
        fs::read(&second).expect("copy is read"),
        fs::read(sample_file("dark-circle.svg")).expect("original is read")
    );
}

#[test]
fn a_missing_asset_has_no_opening_copy() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![svg],
            trashed: true,
        })
        .expect("asset is trashed");
    fixture.library.empty_trash().expect("trash is emptied");

    assert_eq!(
        fixture
            .library
            .opening_copy(svg, &FlatFiles::new())
            .expect("lookup works"),
        None
    );
}

#[test]
fn a_conversion_takes_the_display_name_and_the_new_extension() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    fixture.rename(png, "Point rouge");
    let folder = fixture.workspace.path().join("Exports");
    fs::create_dir(&folder).expect("folder is created");
    fs::write(folder.join("Point rouge.jpg"), b"mine").expect("file is written");

    let saved = Library::save_converted(&fixture.asset(png), &folder, None, "jpg", b"converted")
        .expect("conversion is saved");

    assert_eq!(saved, folder.join("Point rouge (2).jpg"));
    assert_eq!(fs::read(&saved).expect("file is read"), b"converted");
    assert_eq!(
        fs::read(folder.join("Point rouge.jpg")).expect("file is read"),
        b"mine"
    );
}

#[test]
fn the_original_extension_is_not_kept_in_a_conversion() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    fixture.rename(png, "red-dot.png");
    let folder = fixture.workspace.path().join("Exports");
    fs::create_dir(&folder).expect("folder is created");

    let saved = Library::save_converted(&fixture.asset(png), &folder, None, "webp", b"converted")
        .expect("conversion is saved");

    assert_eq!(saved, folder.join("red-dot.webp"));
}

#[test]
fn a_conversion_to_a_missing_folder_is_refused() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let missing = fixture.workspace.path().join("absent");

    assert!(
        Library::save_converted(&fixture.asset(png), &missing, None, "jpg", b"converted").is_err()
    );
    assert!(!missing.exists());
}

#[test]
fn a_conversion_can_carry_its_size_in_its_name() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    fixture.rename(png, "Logo");
    let folder = fixture.workspace.path().join("Exports");
    fs::create_dir(&folder).expect("folder is created");

    let saved = Library::save_converted(&fixture.asset(png), &folder, Some("512"), "png", b"x")
        .expect("conversion is saved");

    assert_eq!(saved, folder.join("Logo-512.png"));
}

#[test]
fn a_pasted_image_is_saved_under_its_name() {
    let fixture = Fixture::new();

    let saved = fixture
        .library
        .save_pasted_image("Pasted image 2026-10-07 14.32.05", "png", b"png bytes")
        .expect("pasted image is saved");

    assert_eq!(file_name(&saved), "Pasted image 2026-10-07 14.32.05.png");
    assert_eq!(fs::read(&saved).expect("file is read"), b"png bytes");
}

#[test]
fn a_new_pasted_image_replaces_the_previous_one() {
    let fixture = Fixture::new();
    let first = fixture
        .library
        .save_pasted_image("Pasted image", "png", b"first")
        .expect("first image is saved");

    let second = fixture
        .library
        .save_pasted_image("Pasted image", "png", b"second")
        .expect("second image is saved");

    assert_eq!(first, second);
    assert_eq!(fs::read(&second).expect("file is read"), b"second");
}

#[test]
fn a_pasted_image_name_cannot_leave_its_folder() {
    let fixture = Fixture::new();

    let saved = fixture
        .library
        .save_pasted_image("../escape", "png", b"png bytes")
        .expect("pasted image is saved");

    assert_eq!(file_name(&saved), "-escape.png");
}

#[test]
fn a_dropped_image_keeps_its_own_extension() {
    let fixture = Fixture::new();

    let saved = fixture
        .library
        .save_pasted_image("apple", "WebP", b"webp bytes")
        .expect("dropped image is saved");

    assert_eq!(file_name(&saved), "apple.webp");
}

#[test]
fn an_unsafe_extension_is_cleaned_or_replaced() {
    let fixture = Fixture::new();

    let cleaned = fixture
        .library
        .save_pasted_image("one", "j/p\\g", b"x")
        .expect("saved");
    let replaced = fixture
        .library
        .save_pasted_image("two", "../..", b"x")
        .expect("saved");

    assert_eq!(file_name(&cleaned), "one.jpg");
    assert_eq!(file_name(&replaced), "two.png");
    assert!(replaced.starts_with(fixture.library.root()));
}

#[test]
fn an_annotated_image_is_exported_only_as_its_flattened_copy_and_never_as_its_original() {
    let mut fixture = Fixture::new();
    let id = fixture.import("blue-photo.jpg");
    let shape = pigoune_core::annotations::Shape::new(
        "blur",
        pigoune_core::annotations::Geometry::Box(pigoune_core::annotations::Rect {
            x: 0.0,
            y: 0.0,
            width: 2.0,
            height: 2.0,
        }),
        pigoune_core::annotations::Options::new(),
    );
    fixture
        .library
        .save_annotations(
            id,
            &pigoune_core::annotations::AnnotationDocument::new(
                None,
                vec![pigoune_core::annotations::AnnotationObject::new(1, shape)],
            ),
        )
        .expect("annotated");
    let destination = tempfile::tempdir().expect("destination");

    let refused = fixture
        .library
        .export_to(&[id], destination.path(), &FlatFiles::new())
        .expect("a report");
    assert!(refused.copies.is_empty());
    assert_eq!(refused.failures.len(), 1);
    assert_eq!(
        std::fs::read_dir(destination.path()).expect("dir").count(),
        0
    );
    assert!(
        fixture
            .library
            .export_copies(&[id], &FlatFiles::new())
            .is_err()
    );
    assert!(fixture.library.opening_copy(id, &FlatFiles::new()).is_err());

    let flat = destination.path().join("rendered.png");
    std::fs::write(&flat, b"flattened").expect("flat");
    let mut flats = FlatFiles::new();
    flats.insert(id, flat);
    let exported = fixture
        .library
        .export_to(&[id], destination.path(), &flats)
        .expect("exported");
    assert_eq!(exported.copies.len(), 1);
    let name = exported.copies[0]
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert!(name == "blue-photo.png", "{name}");
    assert_eq!(
        std::fs::read(&exported.copies[0]).expect("copy"),
        b"flattened"
    );
}
