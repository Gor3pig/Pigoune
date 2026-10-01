use gettextrs::gettext;
use pigoune_core::LibraryError;

pub fn describe(error: &LibraryError) -> String {
    match error {
        LibraryError::InvalidName => {
            gettext("This name cannot be used. Avoid “/” and names starting with a dot.")
        }
        LibraryError::AlreadyExists(_) => {
            gettext("A file or folder with this name already exists here.")
        }
        LibraryError::NotFound(_) => gettext(
            "The folder could not be found. It may have been moved, renamed or disconnected.",
        ),
        LibraryError::NotALibrary(_) => gettext("This folder is not a Pigoune library."),
        LibraryError::NewerFormat { .. } => gettext(
            "This library was created by a newer version of Pigoune. Update Pigoune to open it.",
        ),
        LibraryError::InUse => gettext("This library is already open in another Pigoune window."),
        LibraryError::Io(_) | LibraryError::Database(_) => {
            gettext("An unexpected error occurred: {error}").replace("{error}", &error.to_string())
        }
    }
}
