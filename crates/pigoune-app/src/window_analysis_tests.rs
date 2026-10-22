use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use adw::subclass::prelude::*;
use gtk::glib;
use pigoune_core::{AssetColor, DominantColor, Library, Rgb, RgbaImage, visual_analysis};

use super::PigouneWindow;
use super::opening_tests::{Folder, library_with, rig};

const ISOLATED_CACHE: &str = "XDG_CACHE_HOME";
const CHILD_FLAG: &str = "PIGOUNE_ANALYSIS_TESTS_CHILD";
const PROOF: &str = "ANALYSIS TEST RAN:";
const WAIT_IN_THUMBNAIL: Duration = Duration::from_millis(300);
const TWO_FILES: [&str; 2] = ["red-dot.png", "blue-photo.jpg"];
const ONE_FILE: [&str; 1] = ["blue-photo.jpg"];

#[derive(Clone, Copy)]
enum Analysis {
    Colors,
    Hashes,
}

struct Libraries {
    first: Folder,
    second: Folder,
    first_root: PathBuf,
    second_root: PathBuf,
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("folder is created");
    for entry in std::fs::read_dir(from).expect("folder is read").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type is read").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("file is copied");
        }
    }
}

fn digest_of(root: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    for relative in ["library.db", "cache/visual-details.db"] {
        if let Ok(bytes) = std::fs::read(root.join(relative)) {
            relative.hash(&mut hasher);
            bytes.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn read_from<T>(window: &PigouneWindow, root: &Path, read: impl Fn(&Library) -> T) -> T {
    if let Some(library) = window.imp().library.borrow().as_ref()
        && library.root() == root
    {
        return read(library);
    }
    read(&Library::open(root).expect("the library opens"))
}

fn colors_of(window: &PigouneWindow, root: &Path) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = read_from(window, root, |library| {
        library
            .visible_assets()
            .expect("assets are listed")
            .into_iter()
            .map(|asset| (asset.display_name.clone(), format!("{:?}", asset.colors)))
            .collect()
    });
    found.sort();
    found
}

fn awaiting_hash(window: &PigouneWindow, root: &Path) -> usize {
    read_from(window, root, |library| {
        library
            .assets_awaiting_visual_hash(usize::MAX)
            .expect("the waiting assets are listed")
            .len()
    })
}

fn analysis_with_a_known_hash() -> pigoune_core::VisualAnalysis {
    let pixels: Vec<u8> = (0..16 * 16 * 4)
        .map(|index| u8::try_from(index % 251).expect("a byte"))
        .collect();
    let image = RgbaImage::new(16, 16, pixels).expect("the image is valid");
    visual_analysis(&image).expect("the image is analysed")
}

fn libraries(files: &[&str]) -> Libraries {
    let first = Folder::new("analysis-first");
    let second = Folder::new("analysis-second");
    let created = library_with(&first, "First", "Alpha", files);
    let first_root = created.root().to_path_buf();
    drop(created);
    let second_root = second.0.join("Copy.pigoune");
    copy_tree(&first_root, &second_root);
    let mut copy = Library::open(&second_root).expect("the copy opens");
    let green = vec![DominantColor {
        family: AssetColor::Green,
        average: Rgb::new(0, 200, 0),
    }];
    let assets = copy.visible_assets().expect("assets are listed");
    let colors: Vec<_> = assets
        .iter()
        .map(|asset| (asset.id, green.clone()))
        .collect();
    copy.record_colors_batch(&colors).expect("known colours");
    let analysis = analysis_with_a_known_hash();
    let hashes: Vec<_> = assets
        .iter()
        .map(|asset| (asset.id, Some(analysis.clone())))
        .collect();
    copy.record_visual_analysis_batch(&hashes)
        .expect("known hashes");
    drop(copy);
    Libraries {
        first,
        second,
        first_root,
        second_root,
    }
}

fn pump(duration: Duration) {
    let context = glib::MainContext::default();
    let started = Instant::now();
    while started.elapsed() < duration {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_until_calm(window: &PigouneWindow) {
    let context = glib::MainContext::default();
    while !window.imp().activity.is_calm() {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn open_in(window: &PigouneWindow, root: &Path) {
    window.show_library(Library::open(root).expect("the library opens"));
    window.imp().analysing_colors.set(true);
}

fn start(window: &PigouneWindow, analysis: Analysis) -> glib::JoinHandle<bool> {
    let analysing = window.clone();
    glib::spawn_future_local(async move {
        match analysis {
            Analysis::Colors => analysing.analyse_pending_colors().await,
            Analysis::Hashes => analysing.analyse_pending_visual_hashes().await,
        }
    })
}

fn stale_results_never_reach_another_library(
    analysis: Analysis,
    files: &[&str],
    results_waiting: bool,
    reopen_the_first: bool,
) {
    let rig = rig();
    let window = &rig.window;
    let libraries = libraries(files);
    let known = colors_of(window, &libraries.second_root);
    open_in(window, &libraries.first_root);
    wait_until_calm(window);
    let held = crate::thumbnails::hold_every_load_slot();
    assert_eq!(held.len(), 4);
    let task = start(window, analysis);
    pump(WAIT_IN_THUMBNAIL);
    if results_waiting {
        window.imp().activity.noticed();
    }
    open_in(window, &libraries.second_root);
    let guarded = if reopen_the_first {
        open_in(window, &libraries.first_root);
        &libraries.first_root
    } else {
        &libraries.second_root
    };
    let before = digest_of(guarded);
    drop(held);
    let finished = glib::MainContext::default().block_on(task);
    assert_eq!(
        finished.ok(),
        Some(false),
        "the stale analysis is abandoned"
    );
    assert_eq!(digest_of(guarded), before, "the stale results were written");
    if !reopen_the_first {
        assert_eq!(colors_of(window, &libraries.second_root), known);
        assert_eq!(awaiting_hash(window, &libraries.second_root), 0);
    }
    drop(libraries.first);
    drop(libraries.second);
}

pub fn colors_waiting_for_a_thumbnail_are_dropped_after_a_library_change() {
    stale_results_never_reach_another_library(Analysis::Colors, &TWO_FILES, true, false);
}

pub fn colors_of_the_last_item_are_dropped_after_a_library_change() {
    stale_results_never_reach_another_library(Analysis::Colors, &ONE_FILE, false, false);
}

pub fn hashes_waiting_for_a_thumbnail_are_dropped_after_a_library_change() {
    stale_results_never_reach_another_library(Analysis::Hashes, &TWO_FILES, true, false);
}

pub fn hashes_of_the_last_item_are_dropped_after_a_library_change() {
    stale_results_never_reach_another_library(Analysis::Hashes, &ONE_FILE, false, false);
}

pub fn colors_are_dropped_when_the_first_library_is_opened_again() {
    stale_results_never_reach_another_library(Analysis::Colors, &TWO_FILES, true, true);
}

pub fn hashes_are_dropped_when_the_first_library_is_opened_again() {
    stale_results_never_reach_another_library(Analysis::Hashes, &ONE_FILE, false, true);
}

pub fn an_ordinary_analysis_still_records_colors_and_hashes() {
    let rig = rig();
    let window = &rig.window;
    let libraries = libraries(&TWO_FILES);
    open_in(window, &libraries.first_root);
    let before = colors_of(window, &libraries.first_root);
    assert!(before.iter().all(|(_, colors)| colors == "[]"));
    assert_eq!(awaiting_hash(window, &libraries.first_root), 2);
    let context = glib::MainContext::default();

    let colors = context.block_on(start(window, Analysis::Colors));
    let hashes = context.block_on(start(window, Analysis::Hashes));

    assert_eq!(colors.ok(), Some(true));
    assert_eq!(hashes.ok(), Some(true));
    let after = colors_of(window, &libraries.first_root);
    assert!(after.iter().any(|(_, colors)| colors != "[]"));
    assert_eq!(awaiting_hash(window, &libraries.first_root), 0);
    assert_eq!(colors_of(window, &libraries.second_root).len(), 2);
}

const ANALYSIS_TESTS: [(&str, fn()); 7] = [
    (
        "colors_waiting_for_a_thumbnail_are_dropped_after_a_library_change",
        colors_waiting_for_a_thumbnail_are_dropped_after_a_library_change,
    ),
    (
        "colors_of_the_last_item_are_dropped_after_a_library_change",
        colors_of_the_last_item_are_dropped_after_a_library_change,
    ),
    (
        "hashes_waiting_for_a_thumbnail_are_dropped_after_a_library_change",
        hashes_waiting_for_a_thumbnail_are_dropped_after_a_library_change,
    ),
    (
        "hashes_of_the_last_item_are_dropped_after_a_library_change",
        hashes_of_the_last_item_are_dropped_after_a_library_change,
    ),
    (
        "colors_are_dropped_when_the_first_library_is_opened_again",
        colors_are_dropped_when_the_first_library_is_opened_again,
    ),
    (
        "hashes_are_dropped_when_the_first_library_is_opened_again",
        hashes_are_dropped_when_the_first_library_is_opened_again,
    ),
    (
        "an_ordinary_analysis_still_records_colors_and_hashes",
        an_ordinary_analysis_still_records_colors_and_hashes,
    ),
];

#[test]
fn the_analysis_tests_in_the_isolated_process() {
    if std::env::var_os(CHILD_FLAG).is_none() {
        return;
    }
    for (name, test) in ANALYSIS_TESTS {
        test();
        writeln!(std::io::stdout(), "{PROOF} {name}").expect("the proof is written");
    }
}

#[test]
fn the_analysis_tests_all_run_with_an_isolated_cache() {
    let cache = Folder::new("analysis-cache");
    let output = Command::new(std::env::current_exe().expect("the test binary is known"))
        .args([
            "--exact",
            "window::analysis_tests::the_analysis_tests_in_the_isolated_process",
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
        "the analysis tests failed:\n{report}"
    );
    for (name, _) in ANALYSIS_TESTS {
        assert!(
            report.contains(&format!("{PROOF} {name}")),
            "{name} did not run:\n{report}"
        );
    }
}
