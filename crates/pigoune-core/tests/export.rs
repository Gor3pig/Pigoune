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
    let svg = fixture.import("github-mark.svg");
    fixture.rename(svg, "Logo GitHub");

    let copies = fixture.export(&[svg]);

    assert_eq!(copies.len(), 1);
    assert_eq!(file_name(&copies[0]), "Logo GitHub.svg");
    assert_eq!(
        fs::read(&copies[0]).expect("copy is read"),
        fs::read(sample_file("github-mark.svg")).expect("original is read")
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
    let svg = fixture.import("github-mark.svg");
    let copy = fixture.export(&[svg]).remove(0);

    fs::write(&copy, b"modified").expect("copy is changed");

    let asset = fixture
        .library
        .asset(svg)
        .expect("asset is read")
        .expect("asset exists");
    assert_eq!(
        fs::read(fixture.library.file_of(&asset)).expect("original is read"),
        fs::read(sample_file("github-mark.svg")).expect("fixture is read")
    );
}

#[test]
fn a_new_export_and_a_reopening_forget_the_previous_copies() {
    let mut fixture = Fixture::new();
    let svg = fixture.import("github-mark.svg");
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
