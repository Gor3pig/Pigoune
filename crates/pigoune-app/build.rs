use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const UI_SOURCE_DIR: &str = "ui";
const ICONS_SOURCE_DIR: &str = "../../data/icons";
const PO_SOURCE_DIR: &str = "../../po";
const GETTEXT_PACKAGE: &str = "pigoune";
const DATA_SOURCE_DIR: &str = "../../data";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let staging_dir = out_dir.join("resources");

    compile_blueprints(&staging_dir.join("ui"));
    stage_application_icons(&staging_dir.join("icons"));
    compile_development_translations(&out_dir.join("locale"));
    compile_development_settings_schema(&out_dir.join("schemas"));

    glib_build_tools::compile_resources(
        &[staging_dir.as_path()],
        "resources.gresource.xml",
        "pigoune.gresource",
    );

    println!("cargo::rerun-if-changed={UI_SOURCE_DIR}");
    println!("cargo::rerun-if-changed={ICONS_SOURCE_DIR}");
    println!("cargo::rerun-if-changed={PO_SOURCE_DIR}");
    println!("cargo::rerun-if-changed={DATA_SOURCE_DIR}/io.github.gor3pig.Pigoune.gschema.xml");
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

fn compile_development_translations(locale_dir: &Path) {
    let languages =
        fs::read_to_string(Path::new(PO_SOURCE_DIR).join("LINGUAS")).expect("po/LINGUAS exists");

    for language in languages.split_whitespace() {
        let messages_dir = locale_dir.join(language).join("LC_MESSAGES");
        fs::create_dir_all(&messages_dir).expect("locale directory can be created");

        let status = Command::new("msgfmt")
            .arg("--output-file")
            .arg(messages_dir.join(format!("{GETTEXT_PACKAGE}.mo")))
            .arg(Path::new(PO_SOURCE_DIR).join(format!("{language}.po")))
            .status()
            .expect("msgfmt is installed");

        assert!(status.success(), "msgfmt failed for {language}");
    }
}

fn compile_development_settings_schema(schemas_dir: &Path) {
    fs::create_dir_all(schemas_dir).expect("schemas directory can be created");

    let status = Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg("--targetdir")
        .arg(schemas_dir)
        .arg(DATA_SOURCE_DIR)
        .status()
        .expect("glib-compile-schemas is installed");

    assert!(status.success(), "glib-compile-schemas failed");
}
