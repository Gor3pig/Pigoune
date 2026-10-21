use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use pigoune_core::{AssetCommand, AssetFilter, AssetView, ImportOutcome, Library};

use super::{PigouneWindow, SIDEBAR_REFRESHES};
use crate::sidebar_item::PigouneSidebarItem;

const COUNTS_DELAY_AND_MARGIN: Duration = Duration::from_millis(900);
const ISOLATED_CACHE: &str = "XDG_CACHE_HOME";
const CHILD_FLAG: &str = "PIGOUNE_OPENING_TESTS_CHILD";
const PROOF: &str = "OPENING TEST RAN:";

pub(super) struct Folder(pub(super) PathBuf);

impl Folder {
    pub(super) fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pigoune-opening-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        std::fs::create_dir_all(&path).expect("temporary folder is created");
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pigoune-core/tests/fixtures")
        .join(name)
}

pub(super) fn library_with(
    folder: &Folder,
    name: &str,
    collection: &str,
    files: &[&str],
) -> Library {
    let mut library = Library::create(&folder.0, name).expect("library is created");
    let collection = library
        .create_collection(collection, None)
        .expect("collection is created");
    for file in files {
        match library
            .import_file(&fixture(file), Some(collection))
            .expect("import succeeds")
        {
            ImportOutcome::Imported(_) => {}
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }
    library
        .create_smart_collection(
            "Everything shown",
            &AssetFilter {
                text: "red".to_owned(),
                ..AssetFilter::default()
            },
        )
        .expect("smart collection is created");
    library
}

fn refreshes() -> u32 {
    SIDEBAR_REFRESHES.with(std::cell::Cell::get)
}

pub(super) fn pump(duration: Duration) {
    let context = glib::MainContext::default();
    let end = Instant::now() + duration;
    while Instant::now() < end {
        while context.iteration(false) {}
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn rows(window: &PigouneWindow) -> Vec<String> {
    let list_view = &window.imp().sidebar.imp().list_view;
    let selection = list_view
        .model()
        .and_downcast::<gtk::SingleSelection>()
        .expect("the sidebar has a selection");
    let tree = selection
        .model()
        .and_downcast::<gtk::TreeListModel>()
        .expect("the sidebar is a tree");
    let mut described: Vec<String> = (0..tree.n_items())
        .filter_map(|position| tree.row(position))
        .filter_map(|row| {
            let item = row.item().and_downcast::<PigouneSidebarItem>()?;
            Some(format!(
                "{}{}|{}",
                "  ".repeat(usize::try_from(row.depth()).unwrap_or(0)),
                item.spoken_label(),
                row.is_expanded()
            ))
        })
        .collect();
    described.push(format!("selected={}", selection.selected()));
    described
}

pub(super) struct Rig {
    pub(super) window: PigouneWindow,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.window.destroy();
    }
}

thread_local! {
    static APPLICATION: std::cell::OnceCell<adw::Application> = const { std::cell::OnceCell::new() };
}

fn application() -> adw::Application {
    APPLICATION.with(|cell| {
        cell.get_or_init(|| {
            let application = adw::Application::builder()
                .application_id("io.github.gor3pig.PigouneOpeningTests")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            application
                .register(gio::Cancellable::NONE)
                .expect("the test application registers");
            application
        })
        .clone()
    })
}

pub(super) fn rig() -> Rig {
    adw::init().expect("the opening tests need a display: run them through ./check.sh");
    gio::resources_register_include!("pigoune.gresource")
        .expect("the compiled resources are registered");
    let isolated = std::env::var_os(ISOLATED_CACHE).expect("the isolated cache is announced");
    assert_eq!(
        glib::user_cache_dir(),
        PathBuf::from(&isolated),
        "the tests must never use the real user cache"
    );
    assert!(
        !glib::user_cache_dir().join("pigoune").exists(),
        "the isolated cache starts empty"
    );
    let window = PigouneWindow::new(&application(), crate::settings::load_in_memory());
    Rig { window }
}

fn contains(rows: &[String], text: &str) -> bool {
    rows.iter().any(|row| row.contains(text))
}

pub fn opening_a_library_refreshes_the_sidebar_once_and_shows_the_same_rows() {
    let rig = rig();
    let folder = Folder::new("once");
    let window = &rig.window;
    let library = library_with(&folder, "Once", "Alpha", &["red-dot.png", "blue-photo.jpg"]);

    let before = refreshes();
    window.show_library(library);
    let right_after = rows(window);
    pump(COUNTS_DELAY_AND_MARGIN);

    assert_eq!(refreshes() - before, 1, "one refresh opens a library");
    assert_eq!(rows(window), right_after, "nothing changes after the delay");
    assert!(contains(&right_after, "Alpha, 2 assets"));
    assert!(contains(&right_after, "Everything shown, 1 asset"));
    window.refresh_sidebar();
    assert_eq!(
        rows(window),
        right_after,
        "a forced refresh changes nothing, so the removed one was redundant"
    );
}

pub fn the_other_users_of_the_search_reset_still_refresh_the_counts_later() {
    let rig = rig();
    let folder = Folder::new("later");
    let window = &rig.window;
    window.show_library(library_with(
        &folder,
        "Later",
        "Alpha",
        &["red-dot.png", "blue-photo.jpg"],
    ));
    pump(COUNTS_DELAY_AND_MARGIN);
    let unfiltered = rows(window);

    let before = refreshes();
    window.reset_search();
    assert_eq!(refreshes() - before, 0, "the refresh waits for the delay");
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(refreshes() - before, 1, "the reset still refreshes later");
    assert_eq!(rows(window), unfiltered);

    let before = refreshes();
    window.search("red");
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(refreshes() - before, 1, "a search still refreshes later");
    let searched = rows(window);
    assert_ne!(searched, unfiltered, "the counts follow the search");
    assert!(contains(&searched, "Alpha, 1 asset"));
    window.refresh_sidebar();
    assert_eq!(rows(window), searched, "the delayed refresh was complete");

    window.reset_search();
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(
        rows(window),
        unfiltered,
        "clearing the search restores them"
    );
}

pub fn a_pending_refresh_never_shows_the_previous_library() {
    let rig = rig();
    let first_folder = Folder::new("first");
    let second_folder = Folder::new("second");
    let window = &rig.window;
    window.show_library(library_with(
        &first_folder,
        "First",
        "Alpha",
        &["red-dot.png", "blue-photo.jpg"],
    ));
    window.search("red");
    window.show_library(library_with(
        &second_folder,
        "Second",
        "Beta",
        &["navy-tile.bmp"],
    ));
    let right_after = rows(window);
    pump(COUNTS_DELAY_AND_MARGIN);

    let after = rows(window);
    assert_eq!(after, right_after, "a stale timer changes nothing");
    assert!(contains(&after, "Beta, 1 asset"));
    assert!(!contains(&after, "Alpha"));
    window.refresh_sidebar();
    assert_eq!(rows(window), after);
}

pub fn successive_openings_and_closings_stay_consistent() {
    let rig = rig();
    let first_folder = Folder::new("loop-a");
    let second_folder = Folder::new("loop-b");
    let window = &rig.window;
    let first_root = {
        let library = library_with(&first_folder, "LoopA", "Alpha", &["red-dot.png"]);
        let root = library.root().to_path_buf();
        window.show_library(library);
        root
    };
    let alone = rows(window);
    window.close_library();
    pump(COUNTS_DELAY_AND_MARGIN);
    assert!(window.imp().library.borrow().is_none());

    window.show_library(library_with(
        &second_folder,
        "LoopB",
        "Beta",
        &["blue-photo.jpg", "navy-tile.bmp"],
    ));
    window.close_library();
    window.show_library(Library::open(&first_root).expect("the first library reopens"));
    window.close_library();
    window.show_library(Library::open(&first_root).expect("and reopens again"));
    pump(COUNTS_DELAY_AND_MARGIN);

    let end = rows(window);
    assert_eq!(end, alone, "reopening gives the very same rows");
    assert!(!contains(&end, "Beta"));
    assert_eq!(window.imp().current_view.get(), AssetView::All);
    window.refresh_sidebar();
    assert_eq!(rows(window), end);
}

pub fn active_filters_never_leak_into_the_next_library() {
    let rig = rig();
    let first_folder = Folder::new("filter-a");
    let second_folder = Folder::new("filter-b");
    let window = &rig.window;
    let mut first = library_with(
        &first_folder,
        "Filtered",
        "Alpha",
        &["red-dot.png", "blue-photo.jpg"],
    );
    let favorite = first.visible_assets().expect("assets are listed")[0].id;
    first
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: vec![favorite],
            favorite: true,
        })
        .expect("a favorite is set");
    let first_root = first.root().to_path_buf();
    let second = library_with(&second_folder, "Clear", "Beta", &["navy-tile.bmp"]);
    let second_root = second.root().to_path_buf();

    window.show_library(second);
    let reference = rows(window);
    window.close_library();
    window.show_library(first);
    pump(COUNTS_DELAY_AND_MARGIN);
    let unfiltered = rows(window);
    let favorites_only = window
        .imp()
        .grid_header
        .filter_popover()
        .imp()
        .choices
        .imp()
        .favorites_check
        .get()
        .expect("the filter has a favorites check")
        .clone();

    let before = refreshes();
    favorites_only.set_active(true);
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(refreshes() - before, 1, "a filter still refreshes later");
    let filtered = rows(window);
    assert_ne!(filtered, unfiltered, "the counts follow the filter");
    assert!(contains(&filtered, "Alpha, 1 asset"));
    assert!(window.imp().filters.borrow().favorites_only);

    window.close_library();
    pump(COUNTS_DELAY_AND_MARGIN);
    let before = refreshes();
    window.show_library(Library::open(&second_root).expect("the second library opens"));
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(
        refreshes() - before,
        1,
        "one refresh opens with a filter set"
    );
    assert_eq!(rows(window), reference, "the new library shows no filter");
    assert!(!window.imp().filters.borrow().favorites_only);
    assert!(!favorites_only.is_active());

    favorites_only.set_active(true);
    let before = refreshes();
    window.show_library(Library::open(&first_root).expect("the first library reopens"));
    pump(COUNTS_DELAY_AND_MARGIN);
    assert!(
        (1..=2).contains(&(refreshes() - before)),
        "only the pending refresh may come on top of the opening one"
    );
    assert_eq!(
        rows(window),
        unfiltered,
        "a pending filter refresh is harmless"
    );
    assert!(!window.imp().filters.borrow().favorites_only);
}

pub fn rapid_openings_and_closings_with_a_pending_refresh_stay_consistent() {
    let rig = rig();
    let first_folder = Folder::new("rapid-a");
    let second_folder = Folder::new("rapid-b");
    let window = &rig.window;
    let first = library_with(
        &first_folder,
        "RapidA",
        "Alpha",
        &["red-dot.png", "blue-photo.jpg"],
    );
    let first_root = first.root().to_path_buf();
    let second = library_with(&second_folder, "RapidB", "Beta", &["navy-tile.bmp"]);
    let second_root = second.root().to_path_buf();

    window.show_library(second);
    let second_rows = rows(window);
    window.close_library();
    window.show_library(first);
    pump(COUNTS_DELAY_AND_MARGIN);
    let first_rows = rows(window);

    window.search("red");
    window.close_library();
    window.show_library(Library::open(&second_root).expect("the second library opens"));
    window.reset_search();
    window.show_library(Library::open(&first_root).expect("the first library opens"));
    window.close_library();
    window.show_library(Library::open(&second_root).expect("the second library reopens"));
    pump(COUNTS_DELAY_AND_MARGIN);
    let end = rows(window);
    assert_eq!(end, second_rows, "the last library alone is shown");
    assert!(!contains(&end, "Alpha"));
    assert_eq!(
        window.imp().library.borrow().as_ref().map(Library::root),
        Some(second_root.as_path())
    );

    window.reset_search();
    window.close_library();
    pump(COUNTS_DELAY_AND_MARGIN);
    assert!(window.imp().library.borrow().is_none());

    window.show_library(Library::open(&first_root).expect("the first library opens again"));
    window.search("red");
    pump(COUNTS_DELAY_AND_MARGIN);
    assert!(contains(&rows(window), "Alpha, 1 asset"));
    window.reset_search();
    pump(COUNTS_DELAY_AND_MARGIN);
    assert_eq!(rows(window), first_rows, "everything works after the storm");
}

const OPENING_TESTS: [(&str, fn()); 6] = [
    (
        "opening_a_library_refreshes_the_sidebar_once_and_shows_the_same_rows",
        opening_a_library_refreshes_the_sidebar_once_and_shows_the_same_rows,
    ),
    (
        "the_other_users_of_the_search_reset_still_refresh_the_counts_later",
        the_other_users_of_the_search_reset_still_refresh_the_counts_later,
    ),
    (
        "a_pending_refresh_never_shows_the_previous_library",
        a_pending_refresh_never_shows_the_previous_library,
    ),
    (
        "successive_openings_and_closings_stay_consistent",
        successive_openings_and_closings_stay_consistent,
    ),
    (
        "active_filters_never_leak_into_the_next_library",
        active_filters_never_leak_into_the_next_library,
    ),
    (
        "rapid_openings_and_closings_with_a_pending_refresh_stay_consistent",
        rapid_openings_and_closings_with_a_pending_refresh_stay_consistent,
    ),
];

#[test]
fn the_opening_tests_in_the_isolated_process() {
    if std::env::var_os(CHILD_FLAG).is_none() {
        return;
    }
    for (name, test) in OPENING_TESTS {
        test();
        writeln!(std::io::stdout(), "{PROOF} {name}").expect("the proof is written");
    }
}

#[test]
fn the_opening_tests_all_run_with_an_isolated_cache() {
    let cache = Folder::new("cache");
    let output = Command::new(std::env::current_exe().expect("the test binary is known"))
        .args([
            "--exact",
            "window::opening_tests::the_opening_tests_in_the_isolated_process",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ISOLATED_CACHE, &cache.0)
        .env(CHILD_FLAG, "1")
        .output()
        .expect("the test binary starts again");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "the opening tests failed:\n{report}"
    );
    for (name, _) in OPENING_TESTS {
        assert!(
            report.contains(&format!("{PROOF} {name}")),
            "the opening test {name} did not run:\n{report}"
        );
    }
}
