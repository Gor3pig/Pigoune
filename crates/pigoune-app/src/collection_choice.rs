use pigoune_core::{Collection, CollectionId, CollectionPath, comparable};

const PATH_SEPARATOR: &str = " › ";

pub fn path_label(path: &CollectionPath) -> String {
    path.names.join(PATH_SEPARATOR)
}

pub fn choices<'a>(
    all: &'a [CollectionPath],
    typed: &str,
    held_by_all: &[CollectionId],
) -> Vec<&'a CollectionPath> {
    let typed = comparable(typed.trim());
    all.iter()
        .filter(|path| !held_by_all.contains(&path.id))
        .filter(|path| comparable(&path_label(path)).contains(&typed))
        .collect()
}

pub fn unavailable_destinations(
    collections: &[Collection],
    moving: CollectionId,
) -> Vec<CollectionId> {
    let mut unavailable = vec![moving];
    let mut index = 0;
    while index < unavailable.len() {
        let parent = unavailable[index];
        unavailable.extend(
            collections
                .iter()
                .filter(|collection| collection.parent == Some(parent))
                .map(|collection| collection.id),
        );
        index += 1;
    }
    unavailable.extend(
        collections
            .iter()
            .find(|collection| collection.id == moving)
            .and_then(|collection| collection.parent),
    );
    unavailable
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Collection, CollectionId, CollectionLook, CollectionPath};

    use super::{choices, path_label, unavailable_destinations};

    fn id(number: u8) -> CollectionId {
        CollectionId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id")
    }

    fn collection(number: u8, parent: Option<u8>) -> Collection {
        Collection {
            id: id(number),
            name: format!("Collection {number}"),
            parent: parent.map(id),
            position: 0,
            created_at_unix_ms: 0,
            look: CollectionLook::default(),
        }
    }

    #[test]
    fn a_collection_cannot_go_into_itself_its_descendants_or_its_own_parent() {
        let all = [
            collection(1, None),
            collection(2, Some(1)),
            collection(3, Some(2)),
            collection(4, Some(2)),
            collection(5, None),
        ];
        let unavailable = unavailable_destinations(&all, id(2));
        for blocked in [1, 2, 3, 4] {
            assert!(unavailable.contains(&id(blocked)), "{blocked}");
        }
        assert!(!unavailable.contains(&id(5)));
    }

    #[test]
    fn a_top_level_collection_blocks_only_itself_and_what_it_holds() {
        let all = [
            collection(1, None),
            collection(2, Some(1)),
            collection(3, None),
        ];
        let unavailable = unavailable_destinations(&all, id(1));
        assert!(unavailable.contains(&id(1)) && unavailable.contains(&id(2)));
        assert!(!unavailable.contains(&id(3)));
    }

    fn path(number: u8, names: &[&str]) -> CollectionPath {
        CollectionPath {
            id: CollectionId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id"),
            names: names.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    #[test]
    fn a_path_reads_from_the_root() {
        assert_eq!(path_label(&path(1, &["Marques", "Tech"])), "Marques › Tech");
        assert_eq!(path_label(&path(2, &["Icônes"])), "Icônes");
    }

    #[test]
    fn choices_match_any_part_of_the_path_and_skip_full_collections() {
        let all = [
            path(1, &["Icônes"]),
            path(2, &["Marques"]),
            path(3, &["Marques", "Tech"]),
        ];
        let labels =
            |found: Vec<&CollectionPath>| found.into_iter().map(path_label).collect::<Vec<_>>();
        assert_eq!(
            labels(choices(&all, "", &[])),
            ["Icônes", "Marques", "Marques › Tech"]
        );
        assert_eq!(
            labels(choices(&all, " MARQ ", &[])),
            ["Marques", "Marques › Tech"]
        );
        assert_eq!(labels(choices(&all, "tech", &[])), ["Marques › Tech"]);
        assert_eq!(
            labels(choices(&all, "", &[all[1].id])),
            ["Icônes", "Marques › Tech"]
        );
    }

    #[test]
    fn choices_ignore_accents_and_ligatures() {
        let all = [path(1, &["Icônes"]), path(2, &["Cœurs"])];
        let labels =
            |found: Vec<&CollectionPath>| found.into_iter().map(path_label).collect::<Vec<_>>();
        assert_eq!(labels(choices(&all, "icones", &[])), ["Icônes"]);
        assert_eq!(labels(choices(&all, "coeurs", &[])), ["Cœurs"]);
    }
}
