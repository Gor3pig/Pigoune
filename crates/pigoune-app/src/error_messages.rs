use gettextrs::gettext;

use crate::health_page::Refusal;
use pigoune_core::{
    AdoptError, AssetError, CollectionError, DuplicateError, LONGEST_TAG_NAME, LibraryError,
    RemoveRecordError, ReplaceError, SmartCollectionError, TagError, UndoError,
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
    Refusal {
        title,
        body,
        try_again: false,
    }
}

pub fn describe_replacement(error: &ReplaceError, file_name: &str, asset_name: &str) -> Refusal {
    let (title, body, try_again) = match error {
        ReplaceError::DifferentContent(_) => (
            gettext("This Is Not the Same File"),
            gettext(
                "The content of “{file}” is not identical to “{name}” as it was imported. Choose a copy identical to the original.",
            )
            .replace("{file}", file_name)
            .replace("{name}", asset_name),
            true,
        ),
        ReplaceError::Unreadable(_) => (
            gettext("“{file}” Cannot Be Read").replace("{file}", file_name),
            gettext("This file could not be read. Nothing was changed."),
            true,
        ),
        ReplaceError::OutsideLibrary(_) => (
            gettext("Unable to Replace the File"),
            gettext(
                "The library records a place for “{name}” outside its own folder, so Pigoune will not write there. Nothing was changed.",
            )
            .replace("{name}", asset_name),
            false,
        ),
        ReplaceError::AssetNotFound(_) => (
            gettext("This Asset No Longer Exists"),
            gettext("Check the library again to see its current state."),
            false,
        ),
        ReplaceError::Library(error) => (
            gettext("Unable to Replace the File"),
            describe(error),
            false,
        ),
    };
    Refusal {
        title,
        body,
        try_again,
    }
}

pub fn describe_removal(error: &RemoveRecordError, asset_name: &str) -> Refusal {
    let (title, body) = match error {
        RemoveRecordError::FileStillThere(_) => (
            gettext("“{name}” Was Not Removed").replace("{name}", asset_name),
            gettext(
                "Its file exists again, so nothing was removed. Check the library again to see its current state.",
            ),
        ),
        RemoveRecordError::AssetNotFound(_) => (
            gettext("This Asset No Longer Exists"),
            gettext("Check the library again to see its current state."),
        ),
        RemoveRecordError::Library(error) => (
            gettext("Unable to Remove the Asset from the Library"),
            describe(error),
        ),
    };
    Refusal {
        title,
        body,
        try_again: false,
    }
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
        LibraryError::UnsafeCache(entry) => gettext(
            "Pigoune cannot use “{entry}” in the cache of this library, because it is a link or another kind of item than expected. For safety, Pigoune does not follow it. You can look at it in the library folder, then rename or remove it yourself and try again.",
        )
        .replace("{entry}", &pigoune_core::cache_entry_label(entry)),
        LibraryError::UnsafeStorage(entry) => gettext(
            "Pigoune cannot use “{entry}” in this library, because it is a link, another kind of item than expected, or a place outside the library. For safety, Pigoune does not follow it. You can look at it in the library folder, then rename or remove it yourself and try again.",
        )
        .replace("{entry}", &pigoune_core::storage_entry_label(entry)),
        LibraryError::SharedDatabase => gettext(
            "The database of this library (“library.db”) is shared with another file through a hard link, so changes made here could change another library. Make an independent copy of the library, for example by copying its folder with your file manager, then open the copy.",
        ),
        LibraryError::Io(_) | LibraryError::Database(_) => {
            gettext("An unexpected error occurred: {error}").replace("{error}", &error.to_string())
        }
    }
}

pub fn describe_image_save(error: &LibraryError) -> String {
    match error {
        LibraryError::Io(_) | LibraryError::Database(_) => gettext(
            "The image could not be saved so that it can be imported. Check that the disk has free space and that you can write in the library folder, then try again.",
        ),
        other => describe(other),
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
            gettext("The collections changed in the meantime. Try again.")
        }
        CollectionError::AssetNotFound(_) => gettext("This asset no longer exists."),
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
        AssetError::NotFound(_) => gettext("This asset no longer exists."),
        AssetError::InvalidName => gettext("Enter a name for the asset."),
        AssetError::Library(error) => describe(error),
    }
}

pub fn describe_undo(error: &UndoError) -> String {
    match error {
        UndoError::Asset(error) => describe_asset(error),
        UndoError::Collection(error) => describe_collection(error),
        UndoError::Tag(error) => describe_tag(error),
        UndoError::SmartCollection(error) => describe_smart_collection(error),
        UndoError::Duplicate(error) => describe_duplicate(error),
    }
}

pub fn describe_duplicate(error: &DuplicateError) -> String {
    match error {
        DuplicateError::NotFound(_) => gettext("This asset no longer exists."),
        DuplicateError::NotAvailable(_) => gettext("This asset is in the trash."),
        DuplicateError::NotAlike(_) => gettext("These assets no longer look alike."),
        DuplicateError::UnknownAction(_) => gettext("These ignored duplicates no longer exist."),
        DuplicateError::NothingChanged => gettext("Nothing changed."),
        DuplicateError::Library(error) => describe(error),
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
        TagError::AssetNotFound(_) => gettext("This asset no longer exists."),
        TagError::NameTaken(_) => gettext("Another tag already has this name."),
        TagError::Cycle => gettext("A tag cannot be moved into itself or one of its sub-tags."),
        TagError::ContainsSlash => gettext("A tag name cannot contain a slash."),
        TagError::Library(error) => describe(error),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pigoune_core::LibraryError;

    use super::describe;

    #[test]
    fn a_refused_cache_entry_is_named_from_the_cache_folder_and_explained() {
        let error = LibraryError::UnsafeCache(PathBuf::from(
            "/library/Design.pigoune/cache/thumbnails/128",
        ));

        let text = describe(&error);

        assert!(text.contains("“cache/thumbnails/128”"), "{text}");
        assert!(text.contains("does not follow it"), "{text}");
        assert!(text.contains("rename or remove it yourself"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains('\u{2013}'),
            "{text}"
        );
    }

    #[test]
    fn an_unsafe_original_names_the_entry_and_never_promises_anything_else() {
        let error = LibraryError::UnsafeStorage(PathBuf::from("/library/Design.pigoune/files/ab"));

        let text = describe(&error);

        assert!(text.contains("“files/ab”"), "{text}");
        assert!(text.contains("does not follow it"), "{text}");
        assert!(text.contains("rename or remove it yourself"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains('\u{2013}'),
            "{text}"
        );
    }

    #[test]
    fn a_shared_database_asks_for_an_independent_copy() {
        let text = describe(&LibraryError::SharedDatabase);

        assert!(text.contains("hard link"), "{text}");
        assert!(text.contains("independent copy"), "{text}");
        assert!(
            !text.contains('\u{2014}') && !text.contains('\u{2013}'),
            "{text}"
        );
    }

    #[test]
    fn an_image_that_cannot_be_saved_gets_a_plain_explanation_without_a_system_message() {
        let unexpected = LibraryError::Io(std::io::Error::other("File too large (os error 27)"));

        let text = super::describe_image_save(&unexpected);

        assert!(text.contains("could not be saved"), "{text}");
        assert!(!text.contains("os error"), "{text}");
        assert!(!text.contains("unexpected"), "{text}");
    }

    #[test]
    fn the_explicit_reasons_are_kept_for_an_image_that_cannot_be_saved() {
        assert_eq!(
            super::describe_image_save(&LibraryError::StorageFull),
            describe(&LibraryError::StorageFull)
        );
        let refused =
            LibraryError::UnsafeCache(PathBuf::from("/library/Design.pigoune/cache/pasted"));
        assert!(super::describe_image_save(&refused).contains("“cache/pasted”"));
    }
}
