pub const APP_ID: &str = "io.github.gor3pig.Pigoune";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RESOURCE_BASE_PATH: &str = "/io/github/gor3pig/Pigoune";
pub const GETTEXT_PACKAGE: &str = "pigoune";
pub const LOCALEDIR: &str = match option_env!("PIGOUNE_LOCALEDIR") {
    Some(installed_locale_dir) => installed_locale_dir,
    None => concat!(env!("OUT_DIR"), "/locale"),
};
pub const DEVELOPMENT_SCHEMAS_DIR: &str = concat!(env!("OUT_DIR"), "/schemas");
