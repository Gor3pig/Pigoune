use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use pigoune_core::Library;

use crate::dropped_content::{Dropped, DroppedImage};

use super::opening_tests::{Folder, fixture, library_with, rig};
use super::{PigouneWindow, WELCOME_PAGE};

const ISOLATED_CACHE: &str = "XDG_CACHE_HOME";
const CHILD_FLAG: &str = "PIGOUNE_IMPORT_TESTS_CHILD";
const PROOF: &str = "IMPORT TEST RAN:";
const START_ATTEMPTS: u32 = 1000;
const BUSY_TEXT: &str = "An import is already in progress. Wait for it to finish, then try again.";

thread_local! {
    pub(super) static TOASTS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn toasts_shown() -> Vec<String> {
    TOASTS.with(|shown| shown.borrow().clone())
}

fn forget_toasts() {
    TOASTS.with(|shown| shown.borrow_mut().clear());
}

fn root_of(window: &PigouneWindow) -> Option<PathBuf> {
    window
        .imp()
        .library
        .borrow()
        .as_ref()
        .map(|library| library.root().to_path_buf())
}

fn asset_count(window: &PigouneWindow) -> usize {
    window
        .imp()
        .library
        .borrow()
        .as_ref()
        .map_or(0, |library| library.visible_assets().expect("assets").len())
}

fn last_library(window: &PigouneWindow) -> String {
    window
        .settings()
        .string(crate::settings::LAST_LIBRARY_PATH)
        .to_string()
}

fn actions_enabled(window: &PigouneWindow) -> bool {
    window.imp().library_actions_enabled.get()
}

fn start_import(window: &PigouneWindow, file: &str) -> glib::JoinHandle<()> {
    let context = glib::MainContext::default();
    let started = window.clone();
    let paths = vec![fixture(file)];
    let handle = glib::spawn_future_local(async move {
        let guard = started.begin_import().expect("the import starts");
        started.import_paths(&guard, paths).await;
    });
    for _ in 0..START_ATTEMPTS {
        if window.imp().library.borrow().is_none() {
            return handle;
        }
        context.iteration(false);
    }
    panic!("the import never took the library");
}

fn finish(handle: glib::JoinHandle<()>) {
    glib::MainContext::default()
        .block_on(handle)
        .expect("the import ends");
}

struct Pair {
    first: Folder,
    second: Folder,
}

fn pair() -> Pair {
    Pair {
        first: Folder::new("import-first"),
        second: Folder::new("import-second"),
    }
}

fn two_libraries(window: &PigouneWindow, folders: &Pair) -> (PathBuf, PathBuf) {
    let second = library_with(&folders.second, "Second", "Beta", &["blue-photo.jpg"]);
    let second_root = second.root().to_path_buf();
    drop(second);
    let first = library_with(&folders.first, "First", "Alpha", &["red-dot.png"]);
    let first_root = first.root().to_path_buf();
    window.show_library(first);
    (first_root, second_root)
}

pub fn an_import_returns_its_own_library_and_gives_the_actions_back() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);
    let before = asset_count(window);

    let handle = start_import(window, "green-square.webp");
    assert!(!actions_enabled(window));
    finish(handle);

    assert_eq!(root_of(window), Some(first_root));
    assert_eq!(asset_count(window), before + 1);
    assert!(actions_enabled(window));
}

pub fn a_library_change_requested_during_an_import_is_refused() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, second_root) = two_libraries(window, &folders);
    let before = asset_count(window);
    let last = last_library(window);

    let handle = start_import(window, "green-square.webp");
    window.open_recent_library(&second_root.to_string_lossy());
    assert!(
        root_of(window).is_none(),
        "no other library was shown during the import"
    );
    assert_eq!(last_library(window), last);
    finish(handle);

    assert_eq!(root_of(window), Some(first_root.clone()));
    assert_eq!(last_library(window), first_root.to_string_lossy());
    assert_eq!(asset_count(window), before + 1);
    drop(rig);
    let second = Library::open(&second_root).expect("the second library was never opened");
    assert_eq!(second.visible_assets().expect("assets").len(), 1);
}

pub fn a_close_requested_during_an_import_changes_nothing() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);

    let handle = start_import(window, "green-square.webp");
    let _ = WidgetExt::activate_action(window, "win.close-library", None);
    assert_ne!(
        window.imp().stack.visible_child_name().as_deref(),
        Some(WELCOME_PAGE)
    );
    finish(handle);

    assert_eq!(root_of(window), Some(first_root));
    assert_ne!(
        window.imp().stack.visible_child_name().as_deref(),
        Some(WELCOME_PAGE)
    );
}

pub fn a_library_closed_by_force_during_an_import_is_not_restored() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);

    let handle = start_import(window, "green-square.webp");
    window.close_library();
    finish(handle);

    assert!(root_of(window).is_none(), "a closed library stays closed");
    assert_eq!(
        window.imp().stack.visible_child_name().as_deref(),
        Some(WELCOME_PAGE)
    );
    assert!(!actions_enabled(window));
    drop(rig);
    let reopened = Library::open(&first_root).expect("the library was released");
    assert_eq!(reopened.visible_assets().expect("assets").len(), 2);
}

pub fn a_library_shown_by_force_during_an_import_is_not_overwritten() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, second_root) = two_libraries(window, &folders);

    let handle = start_import(window, "green-square.webp");
    window.show_library(Library::open(&second_root).expect("the second library opens"));
    finish(handle);

    assert_eq!(root_of(window), Some(second_root));
    assert_eq!(asset_count(window), 1);
    drop(rig);
    let first = Library::open(&first_root).expect("the first library was released");
    assert_eq!(first.visible_assets().expect("assets").len(), 2);
}

pub fn a_failed_import_gives_the_actions_back() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);
    let before = asset_count(window);

    let context = glib::MainContext::default();
    let started = window.clone();
    let missing = folders.first.0.join("does-not-exist.png");
    let handle = glib::spawn_future_local(async move {
        let guard = started.begin_import().expect("the import starts");
        started.import_paths(&guard, vec![missing]).await;
    });
    context.block_on(handle).expect("the import ends");

    assert_eq!(root_of(window), Some(first_root));
    assert_eq!(asset_count(window), before);
    assert!(actions_enabled(window));
}

pub fn a_second_import_started_during_an_import_says_so_and_the_first_still_ends() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);
    let before = asset_count(window);
    forget_toasts();

    let handle = start_import(window, "green-square.webp");
    let context = glib::MainContext::default();
    let second = window.clone();
    let other = vec![fixture("blue-photo.jpg")];
    let refused = context.block_on(async move {
        match second.begin_import_or_explain() {
            Some(guard) => {
                second.import_paths(&guard, other).await;
                false
            }
            None => true,
        }
    });
    assert!(refused);
    assert!(
        toasts_shown().iter().any(|text| text == BUSY_TEXT),
        "the second import was refused without a word: {:?}",
        toasts_shown()
    );
    finish(handle);

    assert_eq!(root_of(window), Some(first_root));
    assert_eq!(asset_count(window), before + 1);
    assert!(actions_enabled(window));
}

pub fn the_pasting_chain_takes_the_guard_once_and_a_second_paste_is_refused() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (first_root, _) = two_libraries(window, &folders);
    let before = asset_count(window);
    forget_toasts();
    let bytes = std::fs::read(fixture("green-square.webp")).expect("fixture");
    let guard = std::rc::Rc::new(window.begin_import().expect("the first paste starts"));
    let running = window.clone();
    let held = std::rc::Rc::clone(&guard);
    let handle = glib::spawn_future_local(async move {
        let image = Dropped::Image(DroppedImage {
            bytes,
            name: Some("Pasted".to_owned()),
            extension: "webp".to_owned(),
        });
        running.import_content(&held, image, None).await;
    });
    let context = glib::MainContext::default();
    for _ in 0..START_ATTEMPTS {
        if window.imp().library.borrow().is_none() {
            break;
        }
        context.iteration(false);
    }
    assert!(window.imp().library.borrow().is_none());

    assert!(window.begin_import_or_explain().is_none());
    assert!(toasts_shown().iter().any(|text| text == BUSY_TEXT));
    finish(handle);
    drop(guard);

    assert_eq!(root_of(window), Some(first_root));
    assert_eq!(asset_count(window), before + 1);
    assert!(actions_enabled(window));
    assert!(
        window.begin_import_or_explain().is_some(),
        "a later paste is accepted"
    );
}

pub fn a_stale_expectation_refuses_an_import_and_says_so() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    let (_, second_root) = two_libraries(window, &folders);
    let expected = window.library_token().expect("a library is shown");
    window.show_library(Library::open(&second_root).expect("the second library opens"));
    forget_toasts();

    let refused = window.begin_import_expecting(Some(&expected));

    assert!(refused.is_none());
    assert!(
        toasts_shown()
            .iter()
            .any(|text| text.contains("library was changed"))
    );
    assert!(!window.imp().importing.get());
    assert!(actions_enabled(window));
    let current = window.library_token();
    assert!(window.begin_import_expecting(current.as_ref()).is_some());
}

pub fn the_guard_is_released_on_every_way_out() {
    let rig = rig();
    let window = &rig.window;
    let folders = pair();
    two_libraries(window, &folders);

    let guard = window.begin_import().expect("the import starts");
    assert!(window.imp().importing.get());
    assert!(!actions_enabled(window));
    drop(guard);
    assert!(!window.imp().importing.get());
    assert!(actions_enabled(window));

    let started = window.clone();
    glib::MainContext::default().block_on(async move {
        let guard = started.begin_import().expect("the import starts");
        started.import_paths(&guard, Vec::new()).await;
    });
    assert!(!window.imp().importing.get());
    assert!(actions_enabled(window));
}

const IMPORT_TESTS: [(&str, fn()); 10] = [
    (
        "an_import_returns_its_own_library_and_gives_the_actions_back",
        an_import_returns_its_own_library_and_gives_the_actions_back,
    ),
    (
        "a_library_change_requested_during_an_import_is_refused",
        a_library_change_requested_during_an_import_is_refused,
    ),
    (
        "a_close_requested_during_an_import_changes_nothing",
        a_close_requested_during_an_import_changes_nothing,
    ),
    (
        "a_library_closed_by_force_during_an_import_is_not_restored",
        a_library_closed_by_force_during_an_import_is_not_restored,
    ),
    (
        "a_library_shown_by_force_during_an_import_is_not_overwritten",
        a_library_shown_by_force_during_an_import_is_not_overwritten,
    ),
    (
        "a_failed_import_gives_the_actions_back",
        a_failed_import_gives_the_actions_back,
    ),
    (
        "a_second_import_started_during_an_import_says_so_and_the_first_still_ends",
        a_second_import_started_during_an_import_says_so_and_the_first_still_ends,
    ),
    (
        "the_pasting_chain_takes_the_guard_once_and_a_second_paste_is_refused",
        the_pasting_chain_takes_the_guard_once_and_a_second_paste_is_refused,
    ),
    (
        "a_stale_expectation_refuses_an_import_and_says_so",
        a_stale_expectation_refuses_an_import_and_says_so,
    ),
    (
        "the_guard_is_released_on_every_way_out",
        the_guard_is_released_on_every_way_out,
    ),
];

#[test]
fn the_import_tests_in_the_isolated_process() {
    if std::env::var_os(CHILD_FLAG).is_none() {
        return;
    }
    for (name, test) in IMPORT_TESTS {
        test();
        writeln!(std::io::stdout(), "{PROOF} {name}").expect("the proof is written");
    }
}

#[test]
fn the_import_tests_all_run_with_an_isolated_cache() {
    let cache = Folder::new("import-cache");
    let output = Command::new(std::env::current_exe().expect("the test binary is known"))
        .args([
            "--exact",
            "window::import_tests::the_import_tests_in_the_isolated_process",
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
        "the import tests failed:\n{report}"
    );
    for (name, _) in IMPORT_TESTS {
        assert!(
            report.contains(&format!("{PROOF} {name}")),
            "the import test {name} did not run:\n{report}"
        );
    }
}
