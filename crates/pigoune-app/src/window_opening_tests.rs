use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use pigoune_core::{AssetFilter, AssetView, ImportOutcome, Library};

use super::{PigouneWindow, SIDEBAR_REFRESHES};
use crate::sidebar_item::PigouneSidebarItem;

const COUNTS_DELAY_AND_MARGIN: Duration = Duration::from_millis(900);

struct Folder(PathBuf);

impl Folder {
    fn new(label: &str) -> Self {
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

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pigoune-core/tests/fixtures")
        .join(name)
}

fn library_with(folder: &Folder, name: &str, collection: &str, files: &[&str]) -> Library {
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

fn pump(duration: Duration) {
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

struct Rig {
    window: PigouneWindow,
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

fn rig() -> Option<Rig> {
    let _ = adw::init();
    let _ = gio::resources_register_include!("pigoune.gresource");
    let draft_folder = glib::user_cache_dir().join("pigoune").join("capture-draft");
    if draft_folder.exists() {
        return None;
    }
    let window = PigouneWindow::new(&application(), crate::settings::load_in_memory());
    Some(Rig { window })
}

fn contains(rows: &[String], text: &str) -> bool {
    rows.iter().any(|row| row.contains(text))
}

pub fn opening_a_library_refreshes_the_sidebar_once_and_shows_the_same_rows() {
    let Some(rig) = rig() else {
        return;
    };
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
    let Some(rig) = rig() else {
        return;
    };
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
    let Some(rig) = rig() else {
        return;
    };
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
    let Some(rig) = rig() else {
        return;
    };
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
