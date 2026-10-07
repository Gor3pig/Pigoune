use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetView, FILES_DIR_NAME, ImportControl, ImportEnding, ImportOutcome, Library,
};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
    sources: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self {
            _workspace: workspace,
            library,
            sources: tempfile::tempdir().expect("temporary directory"),
        }
    }

    fn source(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.sources.path().join(name);
        fs::write(&path, contents).expect("source is written");
        path
    }

    fn asset_count(&self) -> usize {
        self.library
            .visible_assets_in(AssetView::All)
            .expect("assets are listed")
            .len()
    }

    fn stored_files(&self) -> usize {
        count_files(&self.library.root().join(FILES_DIR_NAME))
    }
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copy_sample(name: &str, destination: &Path) {
    fs::copy(sample(name), destination).expect("sample is copied");
}

fn count_files(folder: &Path) -> usize {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            if entry.path().is_dir() {
                count_files(&entry.path())
            } else {
                1
            }
        })
        .sum()
}

fn distinct_png(number: usize) -> Vec<u8> {
    let mut bytes = fs::read(sample("red-dot.png")).expect("sample is read");
    bytes.extend_from_slice(number.to_string().as_bytes());
    bytes
}

#[test]
fn refused_files_leave_nothing_behind() {
    let mut fixture = Fixture::new();
    let fake = fixture.source("fake.png", b"this is not an image");
    let empty = fixture.source("empty.png", b"");
    let text = fixture.source("notes.txt", b"hello");
    let absent = fixture.sources.path().join("absent.png");

    for path in [fake, empty, text, absent] {
        let result = fixture.library.import_file(&path, None);
        assert!(result.is_err(), "{path:?} should be refused: {result:?}");
    }

    assert_eq!(fixture.asset_count(), 0);
    assert_eq!(fixture.stored_files(), 0);
}

#[test]
fn an_unreadable_file_is_refused_without_a_trace() {
    let mut fixture = Fixture::new();
    let locked = fixture.source("locked.png", &distinct_png(1));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("permissions change");
    if fs::File::open(&locked).is_ok() {
        return;
    }

    let result = fixture.library.import_file(&locked, None);

    assert!(result.is_err(), "{result:?}");
    assert_eq!(fixture.asset_count(), 0);
    assert_eq!(fixture.stored_files(), 0);
}

#[test]
fn unusual_file_names_are_imported() {
    let mut fixture = Fixture::new();
    let long = format!("{}.png", "x".repeat(250));
    let names = [
        "été 2024 - logo ✓.png",
        "a b  c.png",
        "UPPER.PNG",
        "🙂 émoji.png",
        long.as_str(),
    ];
    for (number, name) in names.iter().enumerate() {
        let path = fixture.source(name, &distinct_png(number));

        let outcome = fixture
            .library
            .import_file(&path, None)
            .unwrap_or_else(|error| panic!("{name:?}: {error}"));

        assert!(matches!(outcome, ImportOutcome::Imported(_)), "{name:?}");
    }
    assert_eq!(fixture.asset_count(), names.len());
}

#[test]
fn a_folder_keeps_its_structure_and_ignores_links() {
    let mut fixture = Fixture::new();
    let root = fixture.sources.path().join("pack");
    fs::create_dir_all(root.join("icons/round")).expect("folders are created");
    fs::create_dir_all(root.join("photos")).expect("folders are created");
    fs::create_dir_all(root.join("empty")).expect("folders are created");
    copy_sample("red-dot.png", &root.join("icons/round/a.png"));
    copy_sample("dark-circle.svg", &root.join("icons/b.svg"));
    copy_sample("blue-photo.jpg", &root.join("photos/c.jpg"));
    copy_sample("red-dot.png", &root.join("photos/copy.png"));
    std::os::unix::fs::symlink(root.join("photos/c.jpg"), root.join("link.jpg"))
        .expect("link is created");

    let summary = fixture
        .library
        .import_paths(
            std::slice::from_ref(&root),
            None,
            |_| true,
            |_| ImportControl::Continue,
        )
        .expect("import runs");

    let paths: Vec<String> = fixture
        .library
        .collection_paths()
        .expect("paths are read")
        .iter()
        .map(|path| path.names.join("/"))
        .collect();
    assert!(
        paths.iter().any(|path| path.ends_with("icons/round")),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.ends_with("photos")),
        "{paths:?}"
    );
    assert_eq!(summary.imported.len(), 3);
    assert_eq!(summary.already_present + summary.added_to_collection, 1);
    assert_eq!(summary.ignored_links, 1);
    assert_eq!(
        fs::read(root.join("photos/c.jpg")).expect("original is read"),
        fs::read(sample("blue-photo.jpg")).expect("sample is read")
    );
}

#[test]
fn a_cancelled_import_keeps_what_came_in_and_can_be_resumed() {
    let mut fixture = Fixture::new();
    let folder = fixture.sources.path().join("many");
    fs::create_dir_all(&folder).expect("folder is created");
    for number in 0..120 {
        fs::write(folder.join(format!("f{number}.png")), distinct_png(number))
            .expect("source is written");
    }
    let mut seen = 0;

    let cancelled = fixture
        .library
        .import_paths(
            std::slice::from_ref(&folder),
            None,
            |_| true,
            |_| {
                seen += 1;
                if seen > 40 {
                    ImportControl::Cancel
                } else {
                    ImportControl::Continue
                }
            },
        )
        .expect("import runs");

    assert!(matches!(cancelled.ending, ImportEnding::Cancelled));
    assert_eq!(fixture.asset_count(), cancelled.imported.len());
    assert_eq!(fixture.stored_files(), cancelled.imported.len());
    assert!(!cancelled.imported.is_empty() && cancelled.imported.len() < 120);

    fixture
        .library
        .import_paths(
            std::slice::from_ref(&folder),
            None,
            |_| true,
            |_| ImportControl::Continue,
        )
        .expect("import resumes");

    assert_eq!(fixture.asset_count(), 120);
    assert_eq!(fixture.stored_files(), 120);
}
