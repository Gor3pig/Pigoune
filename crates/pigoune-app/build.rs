use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const UI_SOURCE_DIR: &str = "ui";
const ICONS_SOURCE_DIR: &str = "../../data/icons";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let staging_dir = out_dir.join("resources");

    compile_blueprints(&staging_dir.join("ui"));
    stage_application_icons(&staging_dir.join("icons"));

    glib_build_tools::compile_resources(
        &[staging_dir.as_path()],
        "resources.gresource.xml",
        "pigoune.gresource",
    );

    println!("cargo::rerun-if-changed={UI_SOURCE_DIR}");
    println!("cargo::rerun-if-changed={ICONS_SOURCE_DIR}");
}

fn compile_blueprints(output_dir: &Path) {
    let blueprints = fs::read_dir(UI_SOURCE_DIR)
        .expect("ui directory exists")
        .map(|entry| entry.expect("ui entry is readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "blp"));

    fs::create_dir_all(output_dir).expect("ui output directory can be created");

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(output_dir)
        .arg(UI_SOURCE_DIR)
        .args(blueprints)
        .status()
        .expect("blueprint-compiler is installed");

    assert!(status.success(), "blueprint-compiler failed");
}

fn stage_application_icons(output_dir: &Path) {
    for size in [16, 32, 48, 64, 128, 256, 512] {
        let target_dir = output_dir.join(format!("{size}x{size}/apps"));
        fs::create_dir_all(&target_dir).expect("icon output directory can be created");
        fs::copy(
            Path::new(ICONS_SOURCE_DIR).join(format!("pigoune-{size}x{size}.png")),
            target_dir.join("io.github.gor3pig.Pigoune.png"),
        )
        .expect("official icon exists");
    }
}
