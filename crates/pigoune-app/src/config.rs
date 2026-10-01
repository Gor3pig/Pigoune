pub const APP_ID: &str = "io.github.gor3pig.Pigoune";
pub const RESOURCE_BASE_PATH: &str = "/io/github/gor3pig/Pigoune";
pub const GETTEXT_PACKAGE: &str = "pigoune";
pub const LOCALEDIR: &str = match option_env!("PIGOUNE_LOCALEDIR") {
    Some(installed_locale_dir) => installed_locale_dir,
    None => concat!(env!("OUT_DIR"), "/locale"),
};
