use pigoune_core::{Collection, CollectionError, Library};
use rusqlite::Connection;
use tempfile::TempDir;

fn library_in(workspace: &TempDir) -> Library {
    Library::create(workspace.path(), "Essai").expect("library is created")
}

#[test]
fn a_root_collection_is_created_with_its_name() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);

    let id = library
        .create_collection("Marques", None)
        .expect("collection is created");

    assert_eq!(
        library.collection(id).expect("collection is read"),
        Some(Collection {
            id,
            name: "Marques".to_owned(),
            parent: None,
        })
    );
}

#[test]
fn a_collection_can_be_created_inside_another() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let brands = library
        .create_collection("Marques", None)
        .expect("parent is created");

    let tech = library
        .create_collection("  Tech  ", Some(brands))
        .expect("child is created");

    let child = library
        .collection(tech)
        .expect("collection is read")
        .expect("collection exists");
    assert_eq!(child.name, "Tech");
    assert_eq!(child.parent, Some(brands));
}

#[test]
fn a_blank_name_is_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);

    for name in ["", "   ", "\t\n"] {
        let result = library.create_collection(name, None);
        assert!(
            matches!(result, Err(CollectionError::InvalidName)),
            "{name:?}: {result:?}"
        );
    }
}

#[test]
fn a_missing_or_trashed_parent_is_refused() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = library_in(&workspace);
    let gone = library
        .create_collection("Disparue", None)
        .expect("collection is created");
    let trashed = library
        .create_collection("Jetée", None)
        .expect("collection is created");
    let database = Connection::open(library.root().join("library.db")).expect("database opens");
    database
        .execute("DELETE FROM collections WHERE id = ?1", [gone.to_string()])
        .expect("collection removed");
    database
        .execute(
            "UPDATE collections SET trashed_at_unix_ms = 1 WHERE id = ?1",
            [trashed.to_string()],
        )
        .expect("collection trashed");

    for parent in [gone, trashed] {
        let result = library.create_collection("Enfant", Some(parent));
        assert!(
            matches!(result, Err(CollectionError::NotFound(id)) if id == parent),
            "{result:?}"
        );
    }
}
