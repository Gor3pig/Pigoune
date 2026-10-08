use gettextrs::gettext;

use crate::health_page::Refusal;
use pigoune_core::{
    AdoptError, AssetError, CollectionError, LONGEST_TAG_NAME, LibraryError, SmartCollectionError,
    TagError, UndoError,
};

pub fn describe_adoption(error: &AdoptError, name: &str, known_name: Option<&str>) -> Refusal {
    let title = gettext("“{name}” Cannot Be Added").replace("{name}", name);
    let body = match error {
        AdoptError::UnsupportedFormat(_) => {
            gettext("This file is not an image that Pigoune recognizes. It was not changed.")
        }
        AdoptError::AlreadyKnown(..) => gettext(
            "Its content is the same as “{other}”, which is already in the library. The file was not changed.",
        )
        .replace("{other}", known_name.unwrap_or_default()),
        AdoptError::NotAdoptable(_) => gettext(
            "This file is not stored the way Pigoune stores its files, so it cannot be added from here. It was not changed.",
        ),
        AdoptError::Unreadable(_) => gettext("This file could not be read. It was not changed."),
        AdoptError::Library(error) => describe(error),
    };
    Refusal { title, body }
}

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
        CollectionError::AssetNotFound(_) => gettext("This resource no longer exists."),
        CollectionError::NotEmpty(name) => gettext(
            "The collection “{name}” is no longer empty, so creating it is not undone. Use “Delete…” to remove it.",
        )
        .replace("{name}", name),
        CollectionError::InvalidLook => {
            gettext("This icon or color cannot be used for a collection.")
        }
        CollectionError::Library(error) => describe(error),
    }
}

pub fn describe_asset(error: &AssetError) -> String {
    match error {
        AssetError::NotFound(_) => gettext("This resource no longer exists."),
        AssetError::InvalidName => gettext("Enter a name for the resource."),
        AssetError::Library(error) => describe(error),
    }
}

pub fn describe_undo(error: &UndoError) -> String {
    match error {
        UndoError::Asset(error) => describe_asset(error),
        UndoError::Collection(error) => describe_collection(error),
        UndoError::Tag(error) => describe_tag(error),
        UndoError::SmartCollection(error) => describe_smart_collection(error),
    }
}

pub fn describe_smart_collection(error: &SmartCollectionError) -> String {
    match error {
        SmartCollectionError::InvalidName => gettext("Enter a name for the smart collection."),
        SmartCollectionError::NameTaken(_) => {
            gettext("Another smart collection already has this name.")
        }
        SmartCollectionError::NotFound(_) => gettext("This smart collection no longer exists."),
        SmartCollectionError::OutdatedOrder => {
            gettext("The smart collections changed in the meantime. Try again.")
        }
        SmartCollectionError::Library(error) => describe(error),
    }
}

pub fn describe_tag(error: &TagError) -> String {
    match error {
        TagError::InvalidName => gettext("Enter a name for the tag."),
        TagError::NotOneWord => {
            gettext("A tag is a single word: join several words with a hyphen.")
        }
        TagError::TooLong => gettext("A tag has at most {count} characters.")
            .replace("{count}", &LONGEST_TAG_NAME.to_string()),
        TagError::NotFound(_) => gettext("This tag no longer exists."),
        TagError::AssetNotFound(_) => gettext("This resource no longer exists."),
        TagError::NameTaken(_) => gettext("Another tag already has this name."),
        TagError::Library(error) => describe(error),
    }
}
