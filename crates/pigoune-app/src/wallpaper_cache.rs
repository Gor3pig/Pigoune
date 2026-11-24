use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gtk::glib;

const KEEP_FOR: Duration = Duration::from_hours(24);

pub fn folder() -> Result<PathBuf, String> {
    let folder = standard_folder();
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

pub fn forget_old_files() {
    forget_older_than(&standard_folder(), KEEP_FOR);
}

fn standard_folder() -> PathBuf {
    glib::user_cache_dir().join("pigoune").join("wallpapers")
}

fn forget_older_than(folder: &Path, age: Duration) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    let limit = SystemTime::now().checked_sub(age);
    for entry in entries.flatten() {
        let path = entry.path();
        if limit.is_some_and(|limit| last_change(&path).is_some_and(|changed| changed < limit)) {
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }
}

fn last_change(path: &Path) -> Option<SystemTime> {
    let own = fs::symlink_metadata(path).ok()?.modified().ok()?;
    let Ok(children) = fs::read_dir(path) else {
        return Some(own);
    };
    Some(
        children
            .flatten()
            .filter_map(|child| last_change(&child.path()))
            .fold(own, SystemTime::max),
    )
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, SystemTime};

    use super::forget_older_than;

    const DAY: Duration = Duration::from_hours(24);

    fn fresh_folder() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let folder = std::env::temp_dir().join(format!(
            "pigoune-wallpaper-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).expect("folder");
        folder
    }

    fn age(path: &Path, by: Duration) {
        let file = File::open(path).expect("opens");
        file.set_modified(SystemTime::now() - by).expect("aged");
    }

    #[test]
    fn an_old_wallpaper_folder_is_forgotten_and_a_recent_one_stays() {
        let root = fresh_folder();
        let old = root.join("old");
        let recent = root.join("recent");
        fs::create_dir_all(&old).expect("folder");
        fs::create_dir_all(&recent).expect("folder");
        fs::write(old.join("a.png"), b"x").expect("file");
        fs::write(recent.join("b.png"), b"x").expect("file");
        age(&old.join("a.png"), DAY * 3);
        age(&old, DAY * 3);

        forget_older_than(&root, DAY);

        assert!(!old.exists());
        assert!(recent.join("b.png").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_with_a_recent_file_inside_is_kept_whatever_its_own_date() {
        let root = fresh_folder();
        let folder = root.join("asset");
        fs::create_dir_all(&folder).expect("folder");
        fs::write(folder.join("fresh.png"), b"x").expect("file");
        age(&folder, DAY * 3);

        forget_older_than(&root, DAY);

        assert!(folder.join("fresh.png").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        let root = fresh_folder();

        forget_older_than(&root.join("absent"), DAY);

        let _ = fs::remove_dir_all(&root);
    }
}
