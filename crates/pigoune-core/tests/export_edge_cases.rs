use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CACHE_DIR_NAME, ImportOutcome, Library, LibraryError,
};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self {
            _workspace: workspace,
            library,
        }
    }

    fn import(&mut self, name: &str) -> AssetId {
        match self
            .library
            .import_file(&sample(name), None)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn rename(&mut self, asset: AssetId, name: &str) -> bool {
        self.library
            .apply_asset_command(&AssetCommand::Rename {
                asset,
                name: name.to_owned(),
            })
            .is_ok()
    }

    fn trash(&mut self, asset: AssetId) {
        self.library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: vec![asset],
                trashed: true,
            })
            .expect("asset is trashed");
    }
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn file_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .expect("folder is read")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn unusual_display_names_give_a_sane_copy_inside_the_library() {
    let long = "x".repeat(400);
    let names = [
        "a/b",
        "..",
        " . ",
        long.as_str(),
        "Logo.PNG",
        "..hidden",
        "line\nbreak",
        "🙂 émoji",
    ];
    for name in names {
        let mut fixture = Fixture::new();
        let asset = fixture.import("red-dot.png");
        if !fixture.rename(asset, name) {
            continue;
        }

        let copies = fixture
            .library
            .clipboard_copies(&[asset])
            .unwrap_or_else(|error| panic!("{name:?}: {error}"));

        assert_eq!(copies.len(), 1, "{name:?}");
        assert!(copies[0].is_file(), "{name:?}");
        assert!(copies[0].starts_with(fixture.library.root()), "{name:?}");
    }
}

#[test]
fn two_resources_with_the_same_name_never_share_a_copy() {
    let mut fixture = Fixture::new();
    let png = fixture.import("red-dot.png");
    let bmp = fixture.import("navy-tile.bmp");
    let webp = fixture.import("green-square.webp");
    for asset in [png, bmp, webp] {
        assert!(fixture.rename(asset, "Logo"));
    }

    let copies = fixture
        .library
        .clipboard_copies(&[png, bmp, webp])
        .expect("copies are made");

    let names: Vec<String> = copies
        .iter()
        .map(|path| {
            path.file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["Logo.png", "Logo.bmp", "Logo.webp"]);
}

#[test]
fn exporting_never_overwrites_a_file_of_the_user() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    assert!(fixture.rename(asset, "Logo"));
    let destination = tempfile::tempdir().expect("temporary directory");
    fixture
        .library
        .export_to(&[asset], destination.path())
        .expect("first export");
    fs::write(destination.path().join("Logo.png"), b"changed by the user")
        .expect("file is changed");

    let second = fixture
        .library
        .export_to(&[asset], destination.path())
        .expect("second export");

    assert_eq!(
        fs::read(destination.path().join("Logo.png")).expect("file is read"),
        b"changed by the user"
    );
    assert_eq!(second.len(), 1);
    assert_ne!(second[0], destination.path().join("Logo.png"));
    assert_eq!(
        fs::read(&second[0]).expect("copy is read"),
        fs::read(sample("red-dot.png")).expect("sample is read")
    );
}

#[test]
fn exporting_into_an_absent_folder_is_an_error() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let destination = tempfile::tempdir().expect("temporary directory");

    let result = fixture
        .library
        .export_to(&[asset], &destination.path().join("absent"));

    assert!(
        matches!(result, Err(LibraryError::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn exporting_into_a_read_only_folder_is_an_error() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let destination = tempfile::tempdir().expect("temporary directory");
    fs::set_permissions(destination.path(), fs::Permissions::from_mode(0o555))
        .expect("permissions change");
    let enforced = fs::write(destination.path().join("probe"), b"x").is_err();

    let result = fixture.library.export_to(&[asset], destination.path());

    fs::set_permissions(destination.path(), fs::Permissions::from_mode(0o755))
        .expect("permissions change");
    if enforced {
        assert!(result.is_err(), "{result:?}");
    }
}

#[test]
fn a_trashed_resource_can_still_be_copied_exported_and_opened() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("dark-circle.svg");
    fixture.trash(asset);
    let destination = tempfile::tempdir().expect("temporary directory");

    let copies = fixture.library.clipboard_copies(&[asset]).expect("copy");
    let exported = fixture
        .library
        .export_to(&[asset], destination.path())
        .expect("export");
    let opening = fixture.library.opening_copy(asset).expect("opening copy");

    assert_eq!(copies.len(), 1);
    assert_eq!(exported.len(), 1);
    assert!(opening.is_some());
    assert_eq!(
        fixture
            .library
            .visible_assets_in(AssetView::Trash)
            .expect("assets are listed")
            .len(),
        1
    );
}

#[test]
fn changing_the_copy_given_to_another_application_leaves_the_library_alone() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("dark-circle.svg");
    let opening = fixture
        .library
        .opening_copy(asset)
        .expect("opening copy")
        .expect("the resource exists");

    fs::write(&opening, b"<svg/>").expect("copy is changed");

    let stored = fixture
        .library
        .file_of(&fixture.library.asset(asset).expect("read").expect("asset"));
    assert_eq!(
        fs::read(stored).expect("stored file is read"),
        fs::read(sample("dark-circle.svg")).expect("sample is read")
    );
}

#[test]
fn temporary_copies_are_forgotten_when_the_library_is_opened_again() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("red-dot.png");
    let copies = fixture.library.clipboard_copies(&[asset]).expect("copy");
    assert!(copies[0].exists());
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let reopened = Library::open(&root).expect("library opens again");

    assert!(!copies[0].exists());
    let cache = root.join(CACHE_DIR_NAME);
    assert!(!cache.join("clipboard").exists() || file_names(&cache.join("clipboard")).is_empty());
    drop(reopened);
}

#[test]
fn a_converted_export_never_overwrites_the_previous_one() {
    let mut fixture = Fixture::new();
    let asset = fixture.import("dark-circle.svg");
    let stored = fixture.library.asset(asset).expect("read").expect("asset");
    let destination = tempfile::tempdir().expect("temporary directory");

    let first = Library::save_converted(&stored, destination.path(), Some("128px"), "png", b"one")
        .expect("first conversion");
    let second = Library::save_converted(&stored, destination.path(), Some("128px"), "png", b"two")
        .expect("second conversion");

    assert_ne!(first, second);
    assert_eq!(fs::read(first).expect("file is read"), b"one");
    assert_eq!(fs::read(second).expect("file is read"), b"two");
}
