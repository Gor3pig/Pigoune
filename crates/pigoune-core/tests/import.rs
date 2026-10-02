use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pigoune_core::{
    Asset, AssetFormat, AssetId, DATABASE_FILE_NAME, Dimensions, FILES_DIR_NAME, ImportError,
    ImportOutcome, Library, LibraryError,
};
use rusqlite::Connection;
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
    sources: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        let sources = workspace.path().join("sources");
        fs::create_dir(&sources).expect("sources folder created");
        Self {
            _workspace: workspace,
            library,
            sources,
        }
    }

    fn source(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.sources.join(name);
        fs::write(&path, content).expect("source written");
        path
    }

    fn sample(&self, fixture_name: &str) -> PathBuf {
        self.source(fixture_name, &fixture_bytes(fixture_name))
    }

    fn import(&mut self, source: &Path) -> Result<ImportOutcome, ImportError> {
        self.library.import_file(source)
    }

    fn import_new(&mut self, source: &Path) -> Asset {
        match self.import(source).expect("import succeeds") {
            ImportOutcome::Imported(id) => self.asset(id),
            ImportOutcome::AlreadyPresent(_) => panic!("the file was unexpectedly a duplicate"),
        }
    }

    fn asset(&self, id: AssetId) -> Asset {
        self.library
            .asset(id)
            .expect("asset is read")
            .expect("asset exists")
    }

    fn files_dir(&self) -> PathBuf {
        self.library.root().join(FILES_DIR_NAME)
    }

    fn stored_folders(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.files_dir())
            .expect("files folder readable")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn asset_count(&self) -> u32 {
        Connection::open(self.library.root().join(DATABASE_FILE_NAME))
            .expect("database opens")
            .query_row("SELECT count(*) FROM assets", [], |row| row.get(0))
            .expect("assets counted")
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read(path).expect("fixture exists")
}

fn dimensions(width: u32, height: u32) -> Dimensions {
    Dimensions::new(width, height).expect("valid dimensions")
}

fn icon_with_sides(sides: &[u8]) -> Vec<u8> {
    let count = u16::try_from(sides.len()).expect("few entries");
    let mut bytes = vec![0, 0, 1, 0];
    bytes.extend(count.to_le_bytes());
    for &side in sides {
        bytes.extend([side, side]);
        bytes.extend([0; 14]);
    }
    bytes
}

fn now_unix_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970");
    i64::try_from(elapsed.as_millis()).expect("realistic date")
}

#[test]
fn an_imported_file_is_copied_and_described() {
    let mut fixture = Fixture::new();
    let source = fixture.sample("github-mark.svg");
    let original = fs::read(&source).expect("source readable");
    let before = now_unix_ms();

    let asset = fixture.import_new(&source);

    assert_eq!(asset.display_name, "github-mark");
    assert_eq!(asset.original_file_name, "github-mark.svg");
    assert_eq!(asset.format, AssetFormat::Svg);
    assert_eq!(asset.dimensions, Some(dimensions(98, 96)));
    assert_eq!(asset.byte_size, original.len() as u64);
    assert_eq!(
        asset.content_hash,
        blake3::hash(&original).to_hex().as_str()
    );
    assert!(!asset.is_animated);
    assert!(asset.embedded_sizes.is_empty());
    assert!((before..=now_unix_ms()).contains(&asset.added_at_unix_ms));
    assert_eq!(
        asset.stored_path,
        Path::new(FILES_DIR_NAME)
            .join(asset.id.to_string())
            .join("github-mark.svg")
    );

    let copy = fixture.library.root().join(&asset.stored_path);
    assert_eq!(fs::read(copy).expect("copy readable"), original);
    assert_eq!(fs::read(&source).expect("source still readable"), original);
}

#[test]
fn every_raster_format_reports_its_dimensions() {
    let cases = [
        ("red-dot.png", AssetFormat::Png, dimensions(3, 2)),
        ("blue-photo.jpg", AssetFormat::Jpeg, dimensions(4, 3)),
        ("green-square.webp", AssetFormat::Webp, dimensions(5, 4)),
        ("still.gif", AssetFormat::Gif, dimensions(2, 2)),
    ];
    let mut fixture = Fixture::new();

    for (name, format, size) in cases {
        let asset = fixture.import_new(&fixture.sample(name));
        assert_eq!(asset.format, format, "{name}");
        assert_eq!(asset.dimensions, Some(size), "{name}");
        assert!(!asset.is_animated, "{name}");
    }
}

#[test]
fn an_animated_gif_is_recognized() {
    let mut fixture = Fixture::new();

    let asset = fixture.import_new(&fixture.sample("spinner.gif"));

    assert_eq!(asset.format, AssetFormat::Gif);
    assert!(asset.is_animated);
}

#[test]
fn an_icon_lists_its_sizes_and_shows_the_largest() {
    let mut fixture = Fixture::new();
    let source = fixture.source("app.ico", &icon_with_sides(&[32, 0, 16, 48]));

    let asset = fixture.import_new(&source);

    assert_eq!(asset.format, AssetFormat::Ico);
    assert_eq!(
        asset.embedded_sizes,
        [
            dimensions(16, 16),
            dimensions(32, 32),
            dimensions(48, 48),
            dimensions(256, 256)
        ]
    );
    assert_eq!(asset.dimensions, Some(dimensions(256, 256)));
}

#[test]
fn the_format_comes_from_the_content_not_the_extension() {
    let mut fixture = Fixture::new();
    let source = fixture.source("misnamed.jpg", &fixture_bytes("red-dot.png"));

    let asset = fixture.import_new(&source);

    assert_eq!(asset.format, AssetFormat::Png);
    assert_eq!(asset.original_file_name, "misnamed.jpg");
}

#[test]
fn an_unsupported_file_is_refused_without_leaving_anything() {
    let mut fixture = Fixture::new();
    let cases = [
        fixture.source("notes.png", b"just some text"),
        fixture.source("page.svg", b"<html><body>hello</body></html>"),
        fixture.source("empty.png", b""),
        fixture.sources.clone(),
    ];

    for source in cases {
        let result = fixture.import(&source);
        assert!(
            matches!(result, Err(ImportError::UnsupportedFormat(ref path)) if *path == source),
            "{}: {result:?}",
            source.display()
        );
    }
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn an_unreadable_file_is_refused_without_leaving_anything() {
    let mut fixture = Fixture::new();
    let png_signature_only = &fixture_bytes("red-dot.png")[..8];
    let cases = [
        fixture.source("truncated.png", png_signature_only),
        fixture.source(
            "broken.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><g></svg>",
        ),
        fixture.sources.join("missing.png"),
    ];

    for source in cases {
        let result = fixture.import(&source);
        assert!(
            matches!(result, Err(ImportError::Unreadable(ref path)) if *path == source),
            "{}: {result:?}",
            source.display()
        );
    }
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn a_duplicate_is_reported_and_not_copied_again() {
    let mut fixture = Fixture::new();
    let first = fixture.import_new(&fixture.sample("red-dot.png"));
    let same_content = fixture.source("renamed copy.png", &fixture_bytes("red-dot.png"));

    let outcome = fixture.import(&same_content).expect("import succeeds");

    assert_eq!(outcome, ImportOutcome::AlreadyPresent(first.id));
    assert_eq!(fixture.stored_folders(), [first.id.to_string()]);
    assert_eq!(fixture.asset_count(), 1);
}

#[test]
fn a_failed_copy_leaves_the_library_unchanged() {
    let mut fixture = Fixture::new();
    let source = fixture.sample("red-dot.png");
    let files_dir = fixture.files_dir();
    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o555)).expect("locked");

    let result = fixture.import(&source);

    fs::set_permissions(&files_dir, fs::Permissions::from_mode(0o755)).expect("unlocked");
    assert!(
        matches!(
            result,
            Err(ImportError::Library(LibraryError::PermissionDenied))
        ),
        "{result:?}"
    );
    assert!(fixture.stored_folders().is_empty());
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn imported_assets_are_still_there_after_reopening() {
    let mut fixture = Fixture::new();
    let asset = fixture.import_new(&fixture.sample("spinner.gif"));
    let root = fixture.library.root().to_path_buf();

    drop(fixture.library);
    let reopened = Library::open(&root).expect("library reopens");

    assert_eq!(reopened.asset(asset.id).expect("asset read"), Some(asset));
}

#[test]
fn leftovers_of_an_interrupted_import_are_removed_on_opening() {
    let fixture = Fixture::new();
    let root = fixture.library.root().to_path_buf();
    let leftover = fixture
        .files_dir()
        .join(".0199a8f2-0000-7000-8000-000000000000.partial");
    fs::create_dir(&leftover).expect("leftover created");
    fs::write(leftover.join("half.png"), b"\x89PNG").expect("half file written");

    drop(fixture.library);
    let _reopened = Library::open(&root).expect("library reopens");

    assert!(!leftover.exists());
}
