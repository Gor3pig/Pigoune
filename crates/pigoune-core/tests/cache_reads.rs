use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, Geometry, Options, Rect, Shape,
};
use pigoune_core::{AssetId, FlatFiles, ImportOutcome, Library, LibraryError};
use tempfile::TempDir;

const SECRET: &str = "CONTENT OUTSIDE THE LIBRARY";
const WAIT: Duration = Duration::from_secs(10);

struct Fixture {
    workspace: TempDir,
    library: Library,
    asset: AssetId,
    render: PathBuf,
    secret: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut library = Library::create(workspace.path(), "Essai").expect("library");
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png");
        let asset = match library.import_file(&sample, None).expect("import") {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        };
        let shape = Shape::new(
            "rectangle",
            Geometry::Box(Rect {
                x: 0.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            }),
            Options::new(),
        );
        library
            .save_annotations(
                asset,
                &AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)]),
            )
            .expect("annotated");
        let render = library
            .annotation_render(asset)
            .expect("render")
            .expect("annotated asset")
            .file;
        fs::create_dir_all(render.parent().expect("folder")).expect("annotated folder");
        let secret = workspace.path().join("outside-secret.txt");
        fs::write(&secret, SECRET).expect("secret");
        Self {
            workspace,
            library,
            asset,
            render,
            secret,
        }
    }

    fn flats(&self) -> FlatFiles {
        let mut flats = FlatFiles::new();
        flats.insert(self.asset, self.render.clone());
        flats
    }

    fn plain_render(&self) {
        let _ = fs::remove_file(&self.render);
        fs::write(&self.render, b"the real render").expect("render");
    }

    fn linked_render(&self) {
        let _ = fs::remove_file(&self.render);
        symlink(&self.secret, &self.render).expect("link");
    }

    fn broken_link(&self) {
        let _ = fs::remove_file(&self.render);
        symlink(self.workspace.path().join("nothing-here"), &self.render).expect("link");
    }

    fn folder_render(&self) {
        let _ = fs::remove_file(&self.render);
        fs::create_dir(&self.render).expect("folder");
    }

    fn pipe_render(&self) -> bool {
        let _ = fs::remove_file(&self.render);
        Command::new("mkfifo")
            .arg(&self.render)
            .status()
            .is_ok_and(|status| status.success())
    }

    fn destination(&self) -> PathBuf {
        let folder = self.workspace.path().join("destination");
        fs::create_dir_all(&folder).expect("destination");
        folder
    }

    fn secret_untouched(&self) {
        assert_eq!(fs::read_to_string(&self.secret).expect("secret"), SECRET);
    }

    fn render_is_still_a_link(&self) {
        assert!(
            fs::symlink_metadata(&self.render)
                .expect("render")
                .is_symlink()
        );
        assert_eq!(fs::read_link(&self.render).expect("link"), self.secret);
    }
}

fn contents_below(folder: &Path) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let kind = entry.file_type().expect("kind");
        if kind.is_dir() {
            found.extend(contents_below(&entry.path()));
        } else if kind.is_file() {
            found.push(fs::read(entry.path()).expect("read"));
        }
    }
    found
}

fn nothing_secret_below(folder: &Path) {
    for content in contents_below(folder) {
        assert_ne!(content, SECRET.as_bytes(), "the outside content was copied");
    }
}

fn refused(error: &LibraryError) -> bool {
    matches!(error, LibraryError::UnsafeCache(_))
}

#[test]
fn an_ordinary_render_is_still_exported_copied_and_opened() {
    let fixture = Fixture::new();
    fixture.plain_render();
    let flats = fixture.flats();

    let exported = fixture
        .library
        .export_copies(&[fixture.asset], &flats)
        .expect("export");
    let copied = fixture
        .library
        .clipboard_copies(&[fixture.asset], &flats)
        .expect("clipboard");
    let opened = fixture
        .library
        .opening_copy(fixture.asset, &flats)
        .expect("opening")
        .expect("copy");
    let report = fixture
        .library
        .export_to(&[fixture.asset], &fixture.destination(), &flats)
        .expect("export to");

    assert_eq!(fs::read(&exported[0]).expect("read"), b"the real render");
    assert_eq!(fs::read(&copied[0]).expect("read"), b"the real render");
    assert_eq!(fs::read(&opened).expect("read"), b"the real render");
    assert_eq!(
        fs::read(&report.copies[0]).expect("read"),
        b"the real render"
    );
}

#[test]
fn a_render_that_is_a_link_to_an_outside_file_is_refused_by_the_export() {
    let fixture = Fixture::new();
    fixture.linked_render();

    let error = fixture
        .library
        .export_copies(&[fixture.asset], &fixture.flats())
        .expect_err("refused");

    assert!(refused(&error), "{error:?}");
    nothing_secret_below(&fixture.library.root().join("cache"));
    fixture.secret_untouched();
    fixture.render_is_still_a_link();
}

#[test]
fn a_render_that_is_a_link_is_refused_by_the_clipboard_and_by_the_opening() {
    let fixture = Fixture::new();
    fixture.linked_render();

    let copy = fixture
        .library
        .clipboard_copies(&[fixture.asset], &fixture.flats())
        .expect_err("refused");
    let opening = fixture
        .library
        .opening_copy(fixture.asset, &fixture.flats())
        .expect_err("refused");

    assert!(refused(&copy), "{copy:?}");
    assert!(refused(&opening), "{opening:?}");
    nothing_secret_below(&fixture.library.root().join("cache"));
    fixture.secret_untouched();
    fixture.render_is_still_a_link();
}

#[test]
fn a_render_that_is_a_link_is_refused_by_the_export_to_a_folder() {
    let fixture = Fixture::new();
    fixture.linked_render();
    let destination = fixture.destination();

    let report = fixture
        .library
        .export_to(&[fixture.asset], &destination, &fixture.flats())
        .expect("report");

    assert!(report.copies.is_empty());
    assert_eq!(report.failures.len(), 1);
    nothing_secret_below(&destination);
    fixture.secret_untouched();
    fixture.render_is_still_a_link();
}

#[test]
fn a_render_that_is_a_link_is_refused_by_the_export_of_the_whole_library() {
    let fixture = Fixture::new();
    fixture.linked_render();
    let destination = fixture.destination();
    let plan = fixture
        .library
        .library_export_plan("Unclassified", "annotated")
        .expect("plan");

    let report = plan.run(&destination, &fixture.flats()).expect("run");

    assert!(report.failures.iter().any(|failure| failure.annotated_copy));
    nothing_secret_below(&destination);
    fixture.secret_untouched();
    fixture.render_is_still_a_link();
}

#[test]
fn a_broken_link_a_folder_or_a_missing_render_never_falls_back_to_the_original() {
    let fixture = Fixture::new();
    let original =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png"))
            .expect("original");
    for prepare in [Fixture::broken_link as fn(&Fixture), Fixture::folder_render] {
        prepare(&fixture);

        assert!(
            fixture
                .library
                .export_copies(&[fixture.asset], &fixture.flats())
                .is_err()
        );
        assert!(
            fixture
                .library
                .opening_copy(fixture.asset, &fixture.flats())
                .is_err()
        );
        assert!(
            fixture
                .library
                .clipboard_copies(&[fixture.asset], &fixture.flats())
                .is_err()
        );
        for content in contents_below(&fixture.library.root().join("cache")) {
            assert_ne!(content, original);
        }
    }
    assert!(
        fixture
            .library
            .export_copies(&[fixture.asset], &FlatFiles::new())
            .is_err()
    );
}

#[test]
fn a_pipe_in_place_of_the_render_is_refused_without_blocking() {
    let fixture = Fixture::new();
    if !fixture.pipe_render() {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    let asset = fixture.asset;
    let flats = fixture.flats();
    let Fixture {
        workspace, library, ..
    } = fixture;
    thread::spawn(move || {
        let results = (
            library.export_copies(&[asset], &flats).is_err(),
            library.clipboard_copies(&[asset], &flats).is_err(),
            library.opening_copy(asset, &flats).is_err(),
        );
        let _ = sender.send(results);
    });

    let results = receiver.recv_timeout(WAIT).expect("no blocking");

    assert_eq!(results, (true, true, true));
    drop(workspace);
}

#[test]
fn opening_a_plain_file_for_reading_refuses_links_and_special_entries() {
    let fixture = Fixture::new();
    fixture.plain_render();
    assert!(pigoune_core::open_plain_file(&fixture.render).is_ok());
    assert_eq!(
        pigoune_core::cache_file(&fixture.render),
        pigoune_core::CacheFile::Plain
    );

    fixture.linked_render();
    assert!(pigoune_core::open_plain_file(&fixture.render).is_err());
    assert_eq!(
        pigoune_core::cache_file(&fixture.render),
        pigoune_core::CacheFile::Unsafe
    );

    fixture.broken_link();
    assert!(pigoune_core::open_plain_file(&fixture.render).is_err());
    assert_eq!(
        pigoune_core::cache_file(&fixture.render),
        pigoune_core::CacheFile::Unsafe
    );

    fixture.folder_render();
    assert!(pigoune_core::open_plain_file(&fixture.render).is_err());

    let _ = fs::remove_dir(&fixture.render);
    assert_eq!(
        pigoune_core::cache_file(&fixture.render),
        pigoune_core::CacheFile::Missing
    );
    fixture.secret_untouched();
}
