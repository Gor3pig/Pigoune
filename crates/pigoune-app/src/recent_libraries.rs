use std::path::Path;

use pigoune_core::library_display_name;

pub const RECENT_LIBRARIES_LIMIT: usize = 5;

#[must_use]
pub fn with_opened(recent: &[String], opened: &str) -> Vec<String> {
    std::iter::once(opened.to_owned())
        .chain(recent.iter().filter(|path| *path != opened).cloned())
        .take(RECENT_LIBRARIES_LIMIT)
        .collect()
}

#[must_use]
pub fn without(recent: &[String], removed: &str) -> Vec<String> {
    recent
        .iter()
        .filter(|path| *path != removed)
        .cloned()
        .collect()
}

#[must_use]
pub fn labels(paths: &[String]) -> Vec<String> {
    let names: Vec<String> = paths
        .iter()
        .map(|path| library_display_name(Path::new(path)))
        .collect();
    paths
        .iter()
        .zip(&names)
        .map(|(path, name)| {
            let shared = names.iter().filter(|other| *other == name).count() > 1;
            match parent_folder(path).filter(|_| shared) {
                Some(folder) => format!("{name} ({folder})"),
                None => name.clone(),
            }
        })
        .collect()
}

fn parent_folder(path: &str) -> Option<String> {
    Path::new(path)
        .parent()?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{RECENT_LIBRARIES_LIMIT, labels, with_opened, without};

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn the_opened_library_comes_first_and_only_once() {
        let recent = paths(&["/a/Un.pigoune", "/b/Deux.pigoune"]);
        assert_eq!(
            with_opened(&recent, "/b/Deux.pigoune"),
            paths(&["/b/Deux.pigoune", "/a/Un.pigoune"])
        );
    }

    #[test]
    fn the_list_keeps_the_most_recent_libraries_only() {
        let recent: Vec<String> = (0..RECENT_LIBRARIES_LIMIT)
            .map(|rank| format!("/l/{rank}.pigoune"))
            .collect();
        let updated = with_opened(&recent, "/l/new.pigoune");
        assert_eq!(updated.len(), RECENT_LIBRARIES_LIMIT);
        assert_eq!(updated[0], "/l/new.pigoune");
        assert!(!updated.contains(&format!("/l/{}.pigoune", RECENT_LIBRARIES_LIMIT - 1)));
    }

    #[test]
    fn a_missing_library_can_be_removed() {
        let recent = paths(&["/a/Un.pigoune", "/b/Deux.pigoune"]);
        assert_eq!(
            without(&recent, "/a/Un.pigoune"),
            paths(&["/b/Deux.pigoune"])
        );
    }

    #[test]
    fn libraries_with_the_same_name_are_told_apart_by_their_folder() {
        let recent = paths(&[
            "/srv/archives/Logos.pigoune",
            "/media/usb/Logos.pigoune",
            "/srv/Travail.pigoune",
        ]);
        assert_eq!(
            labels(&recent),
            paths(&["Logos (archives)", "Logos (usb)", "Travail"])
        );
    }
}
