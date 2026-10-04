use std::fs;
use std::path::Path;

use pigoune_core::{ImportOutcome, Library};

fn sample_file(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn the_thumbnail_cache_is_measured_and_cleared_without_touching_the_resources() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let ImportOutcome::Imported(id) = library
        .import_file(&sample_file("red-dot.png"), None)
        .expect("import succeeds")
    else {
        panic!("the resource is new");
    };
    assert_eq!(library.thumbnail_cache_bytes(), 0);

    for (pixels, size) in [(128, 300), (256, 700)] {
        let thumbnail = library.thumbnail_file(id, pixels);
        fs::create_dir_all(thumbnail.parent().expect("thumbnail folder")).expect("folder created");
        fs::write(&thumbnail, vec![0; size]).expect("thumbnail written");
    }
    assert_eq!(library.thumbnail_cache_bytes(), 1000);

    library.clear_thumbnail_cache().expect("cache cleared");

    assert_eq!(library.thumbnail_cache_bytes(), 0);
    let asset = library.asset(id).expect("readable").expect("still there");
    assert!(library.file_of(&asset).is_file());
    library
        .clear_thumbnail_cache()
        .expect("clearing twice is harmless");
}
