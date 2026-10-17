use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static BLOCKED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

pub fn replace(paths: Vec<PathBuf>) -> bool {
    let next: HashSet<PathBuf> = paths.into_iter().collect();
    let Ok(mut blocked) = BLOCKED.lock() else {
        return false;
    };
    let changed = blocked
        .as_ref()
        .map_or(!next.is_empty(), |known| *known != next);
    *blocked = Some(next);
    changed
}

pub fn is_blocked(path: &Path) -> bool {
    BLOCKED.lock().map_or(true, |blocked| {
        blocked.as_ref().is_some_and(|set| set.contains(path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_listed_files_are_blocked_and_the_list_can_change() {
        let first = PathBuf::from("/library/files/ab/one/a.png");
        let second = PathBuf::from("/library/files/ab/one/original.1.png");
        let other = PathBuf::from("/library/files/cd/two/b.png");
        assert!(replace(vec![first.clone(), second.clone()]));
        assert!(is_blocked(&first));
        assert!(is_blocked(&second));
        assert!(!is_blocked(&other));
        assert!(!replace(vec![second.clone(), first.clone()]));
        assert!(replace(vec![other.clone()]));
        assert!(!is_blocked(&first));
        assert!(is_blocked(&other));
        assert!(replace(Vec::new()));
        assert!(!is_blocked(&other));
    }
}
