use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{AssetCommand, AssetId, ImportOutcome, Library};
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

    fn export(&self, assets: &[AssetId]) -> Vec<PathBuf> {
        self.library
            .export_copies(assets)
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

#[test]
fn a_new_export_and_a_reopening_forget_the_previous_copies() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    let old = fixture.export(&[svg]).remove(0);

    let new = fixture.export(&[png]).remove(0);
    assert!(!old.exists());
    assert!(new.exists());

    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);
    let _reopened = Library::open(&root).expect("library reopens");
    assert!(!new.exists());
    drop(fixture.workspace);
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
        .export_to(&[svg], &folder)
        .expect("export succeeds");

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
        .export_to(&[svg], &folder)
        .expect("first export");
    fixture
        .library
        .export_to(&[svg], &folder)
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

    assert!(fixture.library.export_to(&[svg], &missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn clipboard_copies_survive_a_new_drag() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("dark-circle.svg");
    let png = fixture.import("red-dot.png");
    let copied = fixture
        .library
        .clipboard_copies(&[svg])
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
        .opening_copy(svg)
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
        .opening_copy(svg)
        .expect("first copy")
        .expect("asset exists");
    let second = fixture
        .library
        .opening_copy(png)
        .expect("second copy")
        .expect("asset exists");
    fixture.export(&[png]);
    fixture
        .library
        .clipboard_copies(&[svg])
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
        .opening_copy(svg)
        .expect("first copy")
        .expect("asset exists");
    fs::write(&first, b"edited elsewhere").expect("copy is changed");
    fixture.rename(svg, "Cercle");

    let second = fixture
        .library
        .opening_copy(svg)
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
        fixture.library.opening_copy(svg).expect("lookup works"),
        None
    );
}
