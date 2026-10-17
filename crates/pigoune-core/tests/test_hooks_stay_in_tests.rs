use std::fs;
use std::path::Path;

fn text(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)).expect("readable")
}

#[test]
fn the_migration_stop_exists_only_behind_a_feature_that_no_shipped_crate_enables() {
    let schema = text("src/library/schema.rs");
    assert_eq!(schema.matches("process::abort").count(), 1);
    let before_abort = &schema[..schema.find("process::abort").expect("present")];
    let guard = before_abort
        .rfind("#[cfg(feature = \"migration-stops\")]")
        .expect("the abort is guarded");
    assert!(
        before_abort.len() - guard < 400,
        "the guard is the attribute of the function that aborts"
    );
    for shipped in ["../pigoune-app/Cargo.toml", "../../Cargo.toml"] {
        assert!(
            !text(shipped).contains("migration-stops"),
            "{shipped} must not enable the test hook"
        );
    }
    let core = text("Cargo.toml");
    let features = core.split("[features]").nth(1).expect("a features table");
    let defaults = features
        .lines()
        .any(|line| line.trim_start().starts_with("default"));
    assert!(!defaults, "the hook is never on by default");
}
