use std::fs;
use std::path::{Path, PathBuf};

const ABANDONED: [&str; 3] = ["seal", "lock_masks", "mask_lock"];

fn files_under(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, found);
        } else {
            found.push(path);
        }
    }
}

#[test]
fn nothing_of_the_abandoned_definitive_masking_is_left_in_sources_interface_or_translations() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let this_file = Path::new(file!())
        .file_name()
        .expect("a file name")
        .to_owned();
    let mut files = Vec::new();
    files_under(&workspace.join("crates"), &mut files);
    files_under(&workspace.join("po"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        if file.file_name() == Some(&this_file) {
            continue;
        }
        let lowered = text.to_lowercase();
        for word in ABANDONED {
            if lowered.contains(word) {
                found.push(format!("{} mentions {word}", file.display()));
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
