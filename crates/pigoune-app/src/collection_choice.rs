use pigoune_core::{CollectionId, CollectionPath, comparable};

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

#[cfg(test)]
mod tests {
    use pigoune_core::{CollectionId, CollectionPath};

    use super::{choices, path_label};

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
