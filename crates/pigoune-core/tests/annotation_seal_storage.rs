use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, Geometry, Options, Rect, SealRecord, Shape,
};
use pigoune_core::{
    AnnotatedError, AnnotatedImage, AssetId, FinalizeReport, Library, SealProblem, SealStatus,
    SealStep,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png")
}

fn blur() -> Shape {
    Shape::new(
        "blur",
        Geometry::Box(Rect {
            x: 1.0,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(BLUR.options),
    )
}

fn unsealed() -> AnnotationDocument {
    AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur())])
}

fn hash_of(path: &Path) -> String {
    blake3::hash(&fs::read(path).expect("readable"))
        .to_hex()
        .to_string()
}

fn sealed_document(base: &Path, generation: u32, keep_marker: bool) -> AnnotationDocument {
    let mut object = AnnotationObject::new(1, blur());
    object.state.sealed = true;
    let mut document = AnnotationDocument::new(
        None,
        if keep_marker {
            vec![object]
        } else {
            Vec::new()
        },
    );
    document.seal = Some(SealRecord {
        generation,
        base: hash_of(base),
    });
    document
}

struct Bench {
    _workspace: TempDir,
    sources: TempDir,
    library: Library,
    id: AssetId,
    parking: Vec<TempDir>,
}

impl Bench {
    fn png(&self, name: &str, number: usize) -> PathBuf {
        let mut bytes = fs::read(sample()).expect("sample");
        bytes.extend_from_slice(number.to_string().as_bytes());
        let path = self.sources.path().join(name);
        fs::write(&path, bytes).expect("written");
        path
    }

    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut library = Library::create(workspace.path(), "Essai").expect("library");
        let sources = tempfile::tempdir().expect("sources");
        let bench_png = |name: &str, number: usize| {
            let mut bytes = fs::read(sample()).expect("sample");
            bytes.extend_from_slice(number.to_string().as_bytes());
            let path = sources.path().join(name);
            fs::write(&path, bytes).expect("written");
            path
        };
        let rendered = bench_png("rendered.png", 1);
        let original = bench_png("original.png", 2);
        let id = library
            .import_annotated(AnnotatedImage {
                rendered: &rendered,
                original: &original,
                document: &unsealed(),
            })
            .expect("imported");
        Self {
            _workspace: workspace,
            sources,
            library,
            id,
            parking: Vec::new(),
        }
    }

    fn folder(&self) -> PathBuf {
        let asset = self.library.asset(self.id).expect("q").expect("asset");
        self.library
            .root()
            .join(asset.stored_path)
            .parent()
            .expect("folder")
            .to_path_buf()
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.folder())
            .expect("folder")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn reopen(&mut self) {
        let root = self.library.root().to_path_buf();
        let spare = tempfile::tempdir().expect("spare folder");
        let parked = Library::create(spare.path(), "Parking").expect("parking library");
        self.parking.push(spare);
        drop(std::mem::replace(&mut self.library, parked));
        self.library = Library::open(&root).expect("reopened");
    }

    fn rendered_file(&self) -> PathBuf {
        let asset = self.library.asset(self.id).expect("q").expect("asset");
        self.library.root().join(asset.stored_path)
    }

    fn database(&self) -> Connection {
        Connection::open(self.library.root().join("library.db")).expect("database")
    }

    fn seal(&mut self, generation: u32, number: usize) -> (PathBuf, PathBuf) {
        let rendered = self.png(&format!("sealed-rendered-{number}.png"), 100 + number);
        let baked = self.png(&format!("baked-{number}.png"), 200 + number);
        let document = sealed_document(&baked, generation, true);
        self.library
            .seal_annotated(self.id, &rendered, &baked, &document)
            .expect("sealed");
        (rendered, baked)
    }

    fn problems(&self) -> usize {
        self.library
            .health_plan()
            .expect("plan")
            .run(|_| ControlFlow::Continue(()))
            .expect("not cancelled")
            .problems()
    }
}

#[test]
fn a_new_annotated_image_is_not_sealed_and_may_leave() {
    let bench = Bench::new();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unsealed
    );
    assert!(bench.library.ensure_may_leave(bench.id).is_ok());
    assert!(bench.files().contains(&"original.png".to_owned()));
}

#[test]
fn sealing_installs_a_new_generation_and_removes_the_net_original() {
    let mut bench = Bench::new();
    let before = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    let net = fs::read(&before).expect("net original");
    let (rendered, baked) = bench.seal(1, 1);

    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
    let after = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    assert_ne!(after, before);
    assert_eq!(
        fs::read(&after).expect("sealed original"),
        fs::read(&baked).expect("baked")
    );
    assert!(!before.exists(), "the net original is gone");
    assert_eq!(bench.files().len(), 2, "{:?}", bench.files());
    for name in bench.files() {
        let content = fs::read(bench.folder().join(&name)).expect("file");
        assert_ne!(content, net, "{name} still holds the net original");
    }
    assert_eq!(
        fs::read(bench.rendered_file()).expect("rendered"),
        fs::read(&rendered).expect("src")
    );
    assert_eq!(bench.problems(), 0);
    let stored = bench
        .library
        .annotations(bench.id)
        .expect("q")
        .expect("some");
    assert_eq!(stored.seal_generation, 1);
}

#[test]
fn a_sealed_image_survives_a_reopening_verified() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
    assert!(
        bench
            .library
            .seal_attention()
            .expect("attention")
            .is_empty()
    );
}

#[test]
fn a_second_sealing_replaces_the_first_generation() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    bench.seal(2, 2);
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 2 }
    );
    let names = bench.files();
    assert!(names.contains(&"original.2.png".to_owned()), "{names:?}");
    assert!(!names.contains(&"original.1.png".to_owned()), "{names:?}");
    assert_eq!(names.len(), 2);
}

#[test]
fn the_sealing_record_must_follow_the_stored_generation() {
    let mut bench = Bench::new();
    let rendered = bench.png("r.png", 11);
    let baked = bench.png("b.png", 12);
    let skipped = sealed_document(&baked, 2, true);
    assert!(matches!(
        bench
            .library
            .seal_annotated(bench.id, &rendered, &baked, &skipped),
        Err(AnnotatedError::InvalidSeal(_))
    ));
    let wrong_base = sealed_document(&rendered, 1, true);
    assert!(matches!(
        bench
            .library
            .seal_annotated(bench.id, &rendered, &baked, &wrong_base),
        Err(AnnotatedError::InvalidSeal(_))
    ));
    let without_record = unsealed();
    assert!(matches!(
        bench
            .library
            .seal_annotated(bench.id, &rendered, &baked, &without_record),
        Err(AnnotatedError::InvalidSeal(_))
    ));
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unsealed
    );
    assert_eq!(bench.files().len(), 2);
}

#[test]
fn an_ordinary_save_of_a_sealed_image_keeps_the_seal_and_the_original() {
    let mut bench = Bench::new();
    let (_, baked) = bench.seal(1, 1);
    let rendered = bench.png("again.png", 300);
    let mut document = sealed_document(&baked, 1, true);
    document.objects.push(AnnotationObject::new(5, blur()));
    bench
        .library
        .update_annotated(bench.id, &rendered, &document)
        .expect("saved");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
    assert_eq!(
        fs::read(bench.rendered_file()).expect("rendered"),
        fs::read(&rendered).expect("src")
    );
    assert_eq!(bench.files().len(), 2);
}

#[test]
fn a_save_that_drops_the_sealing_record_is_refused() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let rendered = bench.png("again.png", 301);
    assert!(matches!(
        bench
            .library
            .update_annotated(bench.id, &rendered, &unsealed()),
        Err(AnnotatedError::InvalidSeal(_))
    ));
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
}

#[test]
fn removing_every_marker_keeps_the_sealing_verifiable() {
    let mut bench = Bench::new();
    let (_, baked) = bench.seal(1, 1);
    let rendered = bench.png("nomarker.png", 302);
    let document = sealed_document(&baked, 1, false);
    bench
        .library
        .update_annotated(bench.id, &rendered, &document)
        .expect("saved");
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
}

fn set_document(bench: &Bench, json: &str) {
    bench
        .database()
        .execute("UPDATE asset_annotations SET document = ?1", [json])
        .expect("updated");
}

#[test]
fn erasing_the_markers_and_the_record_from_the_document_fails_closed() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    set_document(&bench, r#"{"version":1,"objects":[]}"#);
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::Contradiction)
    );
    assert!(matches!(
        bench.library.ensure_may_leave(bench.id),
        Err(AnnotatedError::SealBlocked(_))
    ));
}

#[test]
fn a_record_that_disagrees_with_the_library_fails_closed() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let other = "f".repeat(64);
    set_document(
        &bench,
        &format!(r#"{{"version":1,"objects":[],"seal":{{"generation":1,"base":"{other}"}}}}"#),
    );
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::Contradiction)
    );
}

#[test]
fn a_marker_without_any_recorded_sealing_fails_closed() {
    let bench = Bench::new();
    let json = {
        let mut document = unsealed();
        document.objects[0].state.sealed = true;
        document.to_json().expect("json")
    };
    set_document(&bench, &json);
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::Contradiction)
    );
}

#[test]
fn a_library_claiming_a_sealing_the_document_does_not_know_fails_closed() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    bench
        .database()
        .execute("UPDATE asset_annotations SET seal_base_hash = NULL", [])
        .expect("tampered");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::Contradiction)
    );
    bench
        .database()
        .execute(
            "UPDATE asset_annotations SET seal_base_hash = ?1, seal_generation = 3",
            [hash_of(&sample())],
        )
        .expect("tampered");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::Contradiction)
    );
}

#[test]
fn a_changed_sealed_original_fails_closed() {
    let bench = {
        let mut bench = Bench::new();
        bench.seal(1, 1);
        bench
    };
    let original = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    fs::write(&original, b"changed").expect("tampered");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::OriginalChanged)
    );
}

#[test]
fn a_missing_sealed_original_fails_closed() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let original = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    fs::remove_file(&original).expect("removed");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::OriginalMissing)
    );
    bench.reopen();
    assert!(!bench.library.seal_attention().expect("a").is_empty());
}

#[test]
fn a_truncated_or_swapped_rendering_of_a_sealed_image_fails_closed() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let rendered = bench.rendered_file();
    let intact = fs::read(&rendered).expect("rendering");
    fs::write(&rendered, &intact[..intact.len() / 2]).expect("truncated");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::RenderedChanged)
    );
    fs::remove_file(&rendered).expect("removed");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(SealProblem::RenderedMissing)
    );
}

#[test]
fn a_symbolic_link_in_place_of_the_sealed_original_fails_closed() {
    let mut bench = Bench::new();
    let (_, baked) = bench.seal(1, 1);
    let original = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    fs::remove_file(&original).expect("removed");
    std::os::unix::fs::symlink(&baked, &original).expect("linked");
    assert!(matches!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unverified(_)
    ));
}

#[test]
fn the_health_report_lists_the_images_whose_sealing_cannot_be_verified() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    assert_eq!(bench.problems(), 0);
    let original = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    fs::write(&original, b"changed").expect("tampered");
    let report = bench
        .library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("done");
    assert_eq!(report.sealing.len(), 1);
    assert_eq!(
        report.sealing[0].1,
        SealStatus::Unverified(SealProblem::OriginalChanged)
    );
}

#[test]
fn an_image_imported_already_sealed_is_stored_under_its_generation() {
    let mut bench = Bench::new();
    let rendered = bench.png("cap.png", 400);
    let baked = bench.png("baked.png", 401);
    let document = sealed_document(&baked, 1, true);
    let id = bench
        .library
        .import_annotated(AnnotatedImage {
            rendered: &rendered,
            original: &baked,
            document: &document,
        })
        .expect("imported");
    assert_eq!(
        bench.library.seal_status(id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
    let original = bench.library.original_path(id).expect("q").expect("some");
    assert!(original.ends_with("original.1.png"));
    let wrong = sealed_document(&rendered, 1, true);
    let other = bench.png("cap2.png", 402);
    assert!(matches!(
        bench.library.import_annotated(AnnotatedImage {
            rendered: &other,
            original: &baked,
            document: &wrong,
        }),
        Err(AnnotatedError::InvalidSeal(_))
    ));
}

fn interrupted(step: SealStep) -> (Bench, PathBuf) {
    let mut bench = Bench::new();
    let net = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    let rendered = bench.png("rr.png", 500);
    let baked = bench.png("bb.png", 501);
    let document = sealed_document(&baked, 1, true);
    bench.library.fail_at(Some(step));
    let result = bench
        .library
        .seal_annotated(bench.id, &rendered, &baked, &document);
    assert!(
        matches!(result, Err(AnnotatedError::Interrupted(found)) if found == step),
        "{result:?}"
    );
    (bench, net)
}

fn no_unfinished_file(bench: &Bench) {
    assert!(
        bench.files().iter().all(|name| !name.ends_with(".partial")),
        "{:?}",
        bench.files()
    );
}

#[test]
fn an_interruption_before_the_commit_leaves_the_previous_state_whole() {
    for step in [
        SealStep::AfterPrepare,
        SealStep::AfterOriginalPartial,
        SealStep::AfterRenderedPartial,
    ] {
        let (mut bench, net) = interrupted(step);
        let net_content = fs::read(&net).expect("net original");
        bench.reopen();
        assert_eq!(
            bench.library.seal_status(bench.id).expect("status"),
            SealStatus::Unsealed,
            "{step:?}"
        );
        assert_eq!(
            fs::read(&net).expect("still there"),
            net_content,
            "{step:?}"
        );
        no_unfinished_file(&bench);
        assert_eq!(bench.problems(), 0, "{step:?}");
        bench.library.ensure_may_leave(bench.id).expect("may leave");
    }
}

#[test]
fn after_the_commit_the_sealing_is_not_complete_until_the_cleanup_is_done() {
    for step in [
        SealStep::AfterCommit,
        SealStep::AfterInstallRendered,
        SealStep::AfterInstallOriginal,
        SealStep::AfterPurge,
        SealStep::BeforeClear,
    ] {
        let (bench, net) = interrupted(step);
        let net_exists_now = net.exists();
        let status_now = bench.library.seal_status(bench.id).expect("status");
        assert_eq!(status_now, SealStatus::Finalizing, "{step:?}");
        assert!(
            bench.library.ensure_may_leave(bench.id).is_err(),
            "{step:?}: nothing may leave while the cleanup is incomplete"
        );
        if matches!(step, SealStep::AfterCommit) {
            assert!(net_exists_now, "the net copy is still there at this point");
        }
    }
}

#[test]
fn reopening_after_an_interruption_after_the_commit_finishes_the_sealing() {
    for step in [
        SealStep::AfterCommit,
        SealStep::AfterInstallRendered,
        SealStep::AfterInstallOriginal,
        SealStep::AfterPurge,
        SealStep::BeforeClear,
    ] {
        let (mut bench, net) = interrupted(step);
        bench.reopen();
        assert_eq!(
            bench.library.seal_status(bench.id).expect("status"),
            SealStatus::Verified { generation: 1 },
            "{step:?}"
        );
        assert!(!net.exists(), "{step:?}: the net original is removed");
        no_unfinished_file(&bench);
        assert_eq!(bench.files().len(), 2, "{step:?}: {:?}", bench.files());
        assert_eq!(bench.problems(), 0, "{step:?}");
        assert!(bench.library.seal_attention().expect("a").is_empty());
    }
}

#[test]
fn a_file_that_cannot_be_proven_ours_is_kept_and_the_sealing_stays_incomplete() {
    let (mut bench, net) = interrupted(SealStep::AfterCommit);
    fs::remove_file(&net).expect("removed");
    let elsewhere = bench.png("other-data.png", 600);
    std::os::unix::fs::symlink(&elsewhere, &net).expect("a link takes its place");
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Finalizing
    );
    assert!(
        fs::symlink_metadata(&net).is_ok(),
        "the ambiguous file is kept"
    );
    assert!(elsewhere.exists());
    let report: FinalizeReport = bench.library.finalize_save(bench.id).expect("report");
    assert!(!report.complete);
    assert_eq!(report.kept, vec![net.clone()]);
    assert!(bench.library.ensure_may_leave(bench.id).is_err());
    fs::remove_file(&net).expect("the user resolves it");
    let report = bench.library.finalize_save(bench.id).expect("report");
    assert!(report.complete);
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
}

#[test]
fn a_user_file_that_looks_like_a_generation_is_never_overwritten_or_removed() {
    let mut bench = Bench::new();
    let stray = bench.folder().join("original.1.png");
    fs::write(&stray, b"precious").expect("stray");
    let rendered = bench.png("r2.png", 700);
    let baked = bench.png("b2.png", 701);
    let document = sealed_document(&baked, 1, true);
    let result = bench
        .library
        .seal_annotated(bench.id, &rendered, &baked, &document);
    assert!(
        matches!(result, Err(AnnotatedError::InvalidSeal(_))),
        "{result:?}"
    );
    assert_eq!(fs::read(&stray).expect("stray"), b"precious");
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unsealed
    );
    assert!(bench.files().iter().all(|name| !name.ends_with(".partial")));
    bench.reopen();
    assert_eq!(fs::read(&stray).expect("stray"), b"precious");
}

#[test]
fn an_unreferenced_old_generation_is_reported_not_deleted() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let stray = bench.folder().join("original.7.png");
    fs::write(&stray, b"unknown").expect("stray");
    bench.reopen();
    let report = bench
        .library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("done");
    assert!(stray.exists());
    assert!(
        report
            .unrecorded
            .iter()
            .any(|path| path.ends_with("original.7.png"))
    );
}

#[test]
fn an_unfinished_file_of_another_image_is_never_touched() {
    let (mut bench, _) = interrupted(SealStep::AfterRenderedPartial);
    let foreign = bench.folder().join(".other-seal-1.partial");
    fs::write(&foreign, b"not ours").expect("written");
    bench.reopen();
    assert!(foreign.exists());
}

#[test]
fn deleting_for_good_removes_every_generation() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let folder = bench.folder();
    bench
        .library
        .remove_record_of_missing_file(bench.id)
        .expect_err("the rendering is still there");
    assert!(folder.exists());
}

#[test]
fn a_damaged_unfinished_file_stops_the_cleanup_and_the_net_original_is_kept() {
    let (mut bench, net) = interrupted(SealStep::AfterCommit);
    let partial = bench
        .files()
        .into_iter()
        .find(|name| name.contains("-seal-"))
        .expect("the sealed image is still unfinished");
    let path = bench.folder().join(&partial);
    let bytes = fs::read(&path).expect("partial");
    fs::write(&path, &bytes[..bytes.len() / 2]).expect("truncated");
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Finalizing
    );
    assert!(
        net.exists(),
        "nothing is purged while a new file is doubtful"
    );
    assert!(
        path.exists(),
        "the doubtful file is kept for the diagnostic"
    );
    assert!(bench.library.ensure_may_leave(bench.id).is_err());
    assert!(bench.library.finalize_save(bench.id).is_err());
    assert!(net.exists());
}

#[test]
fn a_vanished_unfinished_file_stops_the_cleanup_too() {
    let (mut bench, net) = interrupted(SealStep::AfterCommit);
    let partial = bench
        .files()
        .into_iter()
        .find(|name| name.contains("-seal-"))
        .expect("unfinished");
    fs::remove_file(bench.folder().join(partial)).expect("removed");
    bench.reopen();
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Finalizing
    );
    assert!(net.exists());
    assert!(bench.library.ensure_may_leave(bench.id).is_err());
}

#[test]
fn nothing_ever_restores_the_net_original_once_a_sealing_is_committed() {
    let (mut bench, net) = interrupted(SealStep::AfterCommit);
    let recorded = bench
        .library
        .annotations(bench.id)
        .expect("q")
        .expect("some");
    assert_eq!(recorded.seal_generation, 1);
    bench.reopen();
    assert!(!net.exists());
    let recorded = bench
        .library
        .annotations(bench.id)
        .expect("q")
        .expect("some");
    assert_eq!(recorded.seal_generation, 1);
    let rendered = bench.png("late.png", 800);
    let again = bench
        .library
        .update_annotated(bench.id, &rendered, &unsealed());
    assert!(matches!(again, Err(AnnotatedError::InvalidSeal(_))));
}

fn drain(plan: &pigoune_core::LibraryExportPlan, into: &Path) -> pigoune_core::LibraryExportReport {
    plan.run(into).expect("exported")
}

#[test]
fn every_way_out_of_an_image_is_closed_while_the_sealing_is_not_verified() {
    let mut bench = Bench::new();
    bench.seal(1, 1);
    let ids = [bench.id];
    let destination = tempfile::tempdir().expect("destination");
    assert!(
        bench.library.export_copies(&ids).is_ok(),
        "open while verified"
    );
    assert!(bench.library.clipboard_copies(&ids).is_ok());
    assert!(bench.library.opening_copy(bench.id).is_ok());
    assert_eq!(
        bench
            .library
            .export_to(&ids, destination.path())
            .expect("r")
            .failures
            .len(),
        0
    );
    let plan = bench.library.library_export_plan("None").expect("plan");
    assert_eq!(drain(&plan, destination.path()).exported, 1);

    let original = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    fs::write(&original, b"tampered").expect("tampered");
    let outside = tempfile::tempdir().expect("outside");
    let leaked = |folder: &Path| -> usize {
        fs::read_dir(folder).map_or(0, |entries| entries.flatten().count())
    };

    assert!(bench.library.export_copies(&ids).is_err());
    assert!(bench.library.clipboard_copies(&ids).is_err());
    assert!(bench.library.opening_copy(bench.id).is_err());
    let report = bench
        .library
        .export_to(&ids, outside.path())
        .expect("report");
    assert_eq!(report.copies.len(), 0);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(leaked(outside.path()), 0, "no file was written");
    let plan = bench.library.library_export_plan("None").expect("plan");
    let report = drain(&plan, outside.path());
    assert_eq!(report.exported, 0);
    assert_eq!(report.failures.len(), 1);
    assert!(
        fs::read_dir(&report.folder).map_or(true, |entries| entries
            .flatten()
            .all(|entry| entry.path().is_dir())),
        "no image file left the library"
    );
    assert!(bench.library.ensure_may_leave(bench.id).is_err());
}

#[test]
fn an_image_whose_cleanup_is_unfinished_cannot_leave_either() {
    let (bench, _) = interrupted(SealStep::AfterCommit);
    let outside = tempfile::tempdir().expect("outside");
    assert!(bench.library.export_copies(&[bench.id]).is_err());
    assert_eq!(
        bench
            .library
            .export_to(&[bench.id], outside.path())
            .expect("r")
            .copies
            .len(),
        0
    );
}

#[test]
fn a_format_13_library_with_annotations_gains_the_sealing_columns_and_stays_usable() {
    let mut bench = Bench::new();
    let original_before = bench
        .library
        .original_path(bench.id)
        .expect("q")
        .expect("some");
    let root = bench.library.root().to_path_buf();
    let spare = tempfile::tempdir().expect("spare folder");
    let parked = Library::create(spare.path(), "Parking").expect("parking library");
    drop(std::mem::replace(&mut bench.library, parked));
    let connection = Connection::open(root.join("library.db")).expect("database");
    connection
        .execute_batch(
            "ALTER TABLE asset_annotations DROP COLUMN seal_generation;
             ALTER TABLE asset_annotations DROP COLUMN seal_base_hash;
             ALTER TABLE asset_annotations DROP COLUMN seal_phase;
             ALTER TABLE asset_annotations DROP COLUMN seal_journal;
             PRAGMA user_version = 13;",
        )
        .expect("format 13 simulated");
    drop(connection);

    bench.library = Library::open(&root).expect("upgraded");

    assert_eq!(
        bench.library.format_version().expect("version"),
        pigoune_core::CURRENT_FORMAT_VERSION
    );
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Unsealed
    );
    assert_eq!(
        bench
            .library
            .original_path(bench.id)
            .expect("q")
            .expect("some"),
        original_before
    );
    assert!(
        bench
            .library
            .annotation_document(bench.id)
            .expect("doc")
            .is_some()
    );
    bench.seal(1, 9);
    assert_eq!(
        bench.library.seal_status(bench.id).expect("status"),
        SealStatus::Verified { generation: 1 }
    );
}
