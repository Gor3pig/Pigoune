use gettextrs::gettext;
use pigoune_core::{CollectionError, LibraryError};

pub fn describe(error: &LibraryError) -> String {
    match error {
        LibraryError::InvalidName => {
            gettext("This name cannot be used. Avoid “/” and names starting with a dot.")
        }
        LibraryError::AlreadyExists(_) => {
            gettext("A file or folder with this name already exists here.")
        }
        LibraryError::NotFound(_) => gettext(
            "The library folder could not be found. It may have been moved, renamed or deleted, or it may be on a disk that is no longer connected.",
        ),
        LibraryError::NotALibrary(_) => gettext("This folder is not a Pigoune library."),
        LibraryError::NewerFormat { .. } => gettext(
            "This library was created by a newer version of Pigoune. Update Pigoune to open it.",
        ),
        LibraryError::InUse => gettext("This library is already open in another Pigoune window."),
        LibraryError::StorageFull => {
            gettext("There is not enough free space on the disk. Free up some space and try again.")
        }
        LibraryError::PermissionDenied => {
            gettext("Pigoune is not allowed to write to this location. Choose another folder.")
        }
        LibraryError::Damaged => {
            gettext("This library seems to be damaged. Your files are still in its “files” folder.")
        }
        LibraryError::Io(_) | LibraryError::Database(_) => {
            gettext("An unexpected error occurred: {error}").replace("{error}", &error.to_string())
        }
    }
}

pub fn describe_collection(error: &CollectionError) -> String {
    match error {
        CollectionError::InvalidName => gettext("Enter a name for the collection."),
        CollectionError::NameTaken(name) => {
            gettext("A collection named “{name}” already exists here.").replace("{name}", name)
        }
        CollectionError::NotFound(_) => gettext("This collection no longer exists."),
        CollectionError::WouldContainItself => {
            gettext("A collection cannot be placed inside itself or one of its sub-collections.")
        }
        CollectionError::OutdatedOrder => {
            gettext("The collections changed in the meantime. Please try again.")
        }
        CollectionError::Library(error) => describe(error),
    }
}
