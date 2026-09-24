pub(crate) use gettextrs::gettext;

const DOMAIN: &str = "pigoune";
const LOCALE_DIR: &str = match option_env!("PIGOUNE_LOCALEDIR") {
    Some(directory) => directory,
    None => "/usr/share/locale",
};

/// Initialize translation before starting GTK or any worker threads.
///
/// # Safety
/// The process must still be single-threaded, as changing the locale is not thread-safe.
pub(crate) unsafe fn init() -> Result<(), std::io::Error> {
    // SAFETY: The caller guarantees that no other thread can access the locale.
    unsafe { gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "") };
    gettextrs::bindtextdomain(DOMAIN, LOCALE_DIR)?;
    gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8")?;
    gettextrs::textdomain(DOMAIN)?;
    Ok(())
}
