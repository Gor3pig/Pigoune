use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const MINIMUM_KEEP_MS: u64 = 2 * 60 * 60 * 1000;
pub const NORMAL_KEEP_MS: u64 = 24 * 60 * 60 * 1000;
pub const BUDGET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const RESERVE_FLOOR_BYTES: u64 = 1024 * 1024 * 1024;
const RESERVE_PERCENT_DENOMINATOR: u64 = 20;
const TIME_DIGITS: usize = 13;
const RANDOM_DIGITS: usize = 16;
const PRIVATE_FOLDER_MODE: u32 = 0o700;
const STAGING_SUFFIX: &str = ".partial";
const SIZE_SUFFIX: &str = ".bytes";
const ROOT_SEPARATOR: char = ':';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeSpace {
    pub free: u64,
    pub total: u64,
}

pub type SpaceProbe<'a> = &'a dyn Fn(&Path) -> Option<VolumeSpace>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Operation,
    LooseFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    pub started_ms: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    Recent,
    Normal,
    Aged,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Shortage;

#[must_use]
pub fn age_of(started_ms: u64, now_ms: u64) -> Age {
    if started_ms > now_ms.saturating_add(NORMAL_KEEP_MS) {
        return Age::Aged;
    }
    let age = now_ms.saturating_sub(started_ms);
    if age < MINIMUM_KEEP_MS {
        Age::Recent
    } else if age < NORMAL_KEEP_MS {
        Age::Normal
    } else {
        Age::Aged
    }
}

#[must_use]
pub fn operation_name(now_ms: u64, random: u64) -> String {
    format!("{now_ms:013x}-{random:016x}")
}

#[must_use]
pub fn started_at(name: &str) -> Option<u64> {
    let (time, random) = name.split_once('-')?;
    let well_formed = time.len() == TIME_DIGITS
        && random.len() == RANDOM_DIGITS
        && time.chars().all(|digit| digit.is_ascii_hexdigit())
        && random.chars().all(|digit| digit.is_ascii_hexdigit());
    well_formed
        .then(|| u64::from_str_radix(time, 16).ok())
        .flatten()
}

#[must_use]
pub fn staging_name(operation: &str) -> String {
    format!(".{operation}{STAGING_SUFFIX}")
}

#[must_use]
pub fn size_name(operation: &str) -> String {
    format!(".{operation}{SIZE_SUFFIX}")
}

fn hidden_part<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    let inner = name.strip_prefix('.')?.strip_suffix(suffix)?;
    started_at(inner).map(|_| inner)
}

#[must_use]
pub fn reserve_for(volume_bytes: u64) -> u64 {
    RESERVE_FLOOR_BYTES.max(volume_bytes / RESERVE_PERCENT_DENOMINATOR)
}

fn fits(protected_bytes: u64, needed: u64, space: Option<VolumeSpace>) -> bool {
    if protected_bytes.saturating_add(needed) > BUDGET_BYTES {
        return false;
    }
    space.is_none_or(|space| space.free >= needed.saturating_add(reserve_for(space.total)))
}

pub fn make_room(
    entries: &[Entry],
    now_ms: u64,
    needed: u64,
    probe: &mut dyn FnMut() -> Option<VolumeSpace>,
    remove: &mut dyn FnMut(&Entry) -> io::Result<()>,
) -> Result<(), Shortage> {
    let mut kept: Vec<&Entry> = Vec::new();
    for entry in entries {
        if age_of(entry.started_ms, now_ms) == Age::Aged {
            let _ = remove(entry);
        } else {
            kept.push(entry);
        }
    }
    let mut protected: u64 = kept.iter().map(|entry| entry.bytes).sum();
    let recent: u64 = kept
        .iter()
        .filter(|entry| age_of(entry.started_ms, now_ms) == Age::Recent)
        .map(|entry| entry.bytes)
        .sum();
    if recent.saturating_add(needed) > BUDGET_BYTES {
        return Err(Shortage);
    }
    let mut normal: Vec<&Entry> = kept
        .into_iter()
        .filter(|entry| age_of(entry.started_ms, now_ms) == Age::Normal)
        .collect();
    normal.sort_by_key(|entry| entry.started_ms);
    let mut oldest_first = normal.into_iter();
    loop {
        if fits(protected, needed, probe()) {
            return Ok(());
        }
        let Some(entry) = oldest_first.next() else {
            return Err(Shortage);
        };
        if remove(entry).is_ok() {
            protected = protected.saturating_sub(entry.bytes);
        }
    }
}

pub fn secure_root(root: &Path) -> io::Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(root)
        && !metadata.is_dir()
    {
        fs::remove_file(root)?;
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_FOLDER_MODE)
        .create(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(PRIVATE_FOLDER_MODE))
}

pub fn create_staging(root: &Path, operation: &str) -> io::Result<PathBuf> {
    let published = root.join(operation);
    if fs::symlink_metadata(&published).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    let staging = root.join(staging_name(operation));
    fs::DirBuilder::new()
        .mode(PRIVATE_FOLDER_MODE)
        .create(&staging)?;
    Ok(staging)
}

pub fn publish(root: &Path, operation: &str, bytes: u64) -> io::Result<PathBuf> {
    let staging = root.join(staging_name(operation));
    let published = root.join(operation);
    fs::write(root.join(size_name(operation)), bytes.to_string())?;
    fs::rename(&staging, &published)?;
    Ok(published)
}

pub fn discard_staging(root: &Path, operation: &str) {
    let _ = fs::remove_dir_all(root.join(staging_name(operation)));
    let _ = fs::remove_file(root.join(size_name(operation)));
}

fn folder_bytes(folder: &Path) -> u64 {
    fs::read_dir(folder).map_or(0, |children| {
        children
            .flatten()
            .filter_map(|child| child.metadata().ok())
            .filter(fs::Metadata::is_file)
            .map(|metadata| metadata.len())
            .sum()
    })
}

fn recorded_bytes(root: &Path, operation: &str) -> u64 {
    fs::read_to_string(root.join(size_name(operation)))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or_else(|| folder_bytes(&root.join(operation)))
}

fn modified_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(0))
}

pub fn make_room_across(
    roots: &[&Path],
    now_ms: u64,
    needed: u64,
    protected: &dyn Fn(&Path) -> bool,
    probe: &mut dyn FnMut() -> Option<VolumeSpace>,
) -> Result<(), Shortage> {
    let mut entries = Vec::new();
    for (index, root) in roots.iter().enumerate() {
        for mut entry in list_entries(root) {
            if protected(&root.join(&entry.name)) {
                entry.started_ms = now_ms;
            }
            entry.name = format!("{index}{ROOT_SEPARATOR}{}", entry.name);
            entries.push(entry);
        }
    }
    make_room(&entries, now_ms, needed, probe, &mut |entry| {
        let located = entry
            .name
            .split_once(ROOT_SEPARATOR)
            .and_then(|(index, name)| {
                let root = roots.get(index.parse::<usize>().ok()?)?;
                Some((*root, name))
            });
        let Some((root, name)) = located else {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        };
        remove_entry(
            root,
            &Entry {
                name: name.to_owned(),
                ..entry.clone()
            },
        )
    })
}

#[must_use]
pub fn list_entries(root: &Path) -> Vec<Entry> {
    let Ok(children) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for child in children.flatten() {
        let name = child.file_name().to_string_lossy().into_owned();
        let Ok(metadata) = fs::symlink_metadata(child.path()) else {
            continue;
        };
        if let Some(started_ms) = started_at(&name)
            && metadata.is_dir()
        {
            entries.push(Entry {
                bytes: recorded_bytes(root, &name),
                name,
                kind: EntryKind::Operation,
                started_ms,
            });
        } else if (metadata.is_file() || metadata.file_type().is_symlink())
            && !name.starts_with('.')
        {
            entries.push(Entry {
                bytes: if metadata.is_file() {
                    metadata.len()
                } else {
                    0
                },
                name,
                kind: EntryKind::LooseFile,
                started_ms: modified_ms(&metadata),
            });
        }
    }
    entries
}

pub fn remove_entry(root: &Path, entry: &Entry) -> io::Result<()> {
    let path = root.join(&entry.name);
    let metadata = fs::symlink_metadata(&path)?;
    match entry.kind {
        EntryKind::Operation if metadata.is_dir() => {
            fs::remove_dir_all(&path)?;
            let _ = fs::remove_file(root.join(size_name(&entry.name)));
            Ok(())
        }
        EntryKind::LooseFile if metadata.is_file() || metadata.file_type().is_symlink() => {
            fs::remove_file(&path)
        }
        _ => Ok(()),
    }
}

pub fn remove_unfinished(root: &Path) {
    let Ok(children) = fs::read_dir(root) else {
        return;
    };
    for child in children.flatten() {
        let name = child.file_name().to_string_lossy().into_owned();
        let Ok(metadata) = fs::symlink_metadata(child.path()) else {
            continue;
        };
        if hidden_part(&name, STAGING_SUFFIX).is_some() && metadata.is_dir() {
            let _ = fs::remove_dir_all(child.path());
        } else if let Some(operation) = hidden_part(&name, SIZE_SUFFIX)
            && fs::symlink_metadata(root.join(operation)).is_err()
        {
            let _ = fs::remove_file(child.path());
        }
    }
}

pub fn tidy(root: &Path, now_ms: u64) {
    tidy_except(root, now_ms, &|_| false);
}

pub fn tidy_except(root: &Path, now_ms: u64, protected: &dyn Fn(&Path) -> bool) {
    remove_unfinished(root);
    for entry in list_entries(root) {
        if age_of(entry.started_ms, now_ms) == Age::Aged && !protected(&root.join(&entry.name)) {
            let _ = remove_entry(root, &entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::{
        Age, BUDGET_BYTES, Entry, EntryKind, MINIMUM_KEEP_MS, NORMAL_KEEP_MS, Shortage,
        VolumeSpace, age_of, list_entries, make_room, make_room_across, operation_name,
        remove_entry, reserve_for, secure_root, started_at, tidy, tidy_except,
    };

    const HOUR: u64 = 60 * 60 * 1000;
    const GIB: u64 = 1024 * 1024 * 1024;
    const NOW: u64 = 1_000 * HOUR;

    fn operation(hours_old: u64, bytes: u64) -> Entry {
        let started_ms = NOW - hours_old * HOUR;
        Entry {
            name: operation_name(started_ms, hours_old),
            kind: EntryKind::Operation,
            started_ms,
            bytes,
        }
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "it stands in for a probe that may not know the free space"
    )]
    fn roomy() -> Option<VolumeSpace> {
        Some(VolumeSpace {
            free: 500 * GIB,
            total: 1000 * GIB,
        })
    }

    #[test]
    fn a_name_carries_its_start_and_is_never_the_same_twice_at_one_instant() {
        let first = operation_name(NOW, 1);
        let second = operation_name(NOW, 2);
        assert_ne!(first, second);
        assert_eq!(started_at(&first), Some(NOW));
        assert_eq!(started_at("legacy-name.png"), None);
        assert_eq!(started_at("zzzzzzzzzzzzz-0000000000000000"), None);
    }

    #[test]
    fn the_age_has_three_stages_and_a_clock_in_the_far_future_never_protects_forever() {
        assert_eq!(age_of(NOW, NOW), Age::Recent);
        assert_eq!(age_of(NOW - MINIMUM_KEEP_MS + 1, NOW), Age::Recent);
        assert_eq!(age_of(NOW - MINIMUM_KEEP_MS, NOW), Age::Normal);
        assert_eq!(age_of(NOW - NORMAL_KEEP_MS + 1, NOW), Age::Normal);
        assert_eq!(age_of(NOW - NORMAL_KEEP_MS, NOW), Age::Aged);
        assert_eq!(age_of(NOW + HOUR, NOW), Age::Recent);
        assert_eq!(age_of(NOW + 2 * NORMAL_KEEP_MS, NOW), Age::Aged);
    }

    #[test]
    fn the_reserve_is_one_gibibyte_or_five_percent_of_the_volume() {
        assert_eq!(reserve_for(10 * GIB), GIB);
        assert_eq!(reserve_for(100 * GIB), 5 * GIB);
    }

    #[test]
    fn nothing_is_removed_when_everything_fits() {
        let entries = [operation(1, GIB), operation(5, GIB)];
        let removed = RefCell::new(Vec::new());
        let result = make_room(&entries, NOW, GIB, &mut roomy, &mut |entry| {
            removed.borrow_mut().push(entry.name.clone());
            Ok(())
        });
        assert_eq!(result, Ok(()));
        assert!(removed.borrow().is_empty());
    }

    #[test]
    fn entries_older_than_a_day_are_removed_even_when_there_is_room() {
        let entries = [operation(30, GIB), operation(1, GIB)];
        let removed = RefCell::new(Vec::new());
        make_room(&entries, NOW, GIB, &mut roomy, &mut |entry| {
            removed.borrow_mut().push(entry.started_ms);
            Ok(())
        })
        .expect("there is room");
        assert_eq!(*removed.borrow(), [NOW - 30 * HOUR]);
    }

    #[test]
    fn the_budget_frees_the_oldest_normal_entries_first_and_never_a_recent_one() {
        let entries = [
            operation(1, 3 * GIB),
            operation(10, 3 * GIB),
            operation(20, 3 * GIB),
        ];
        let removed = RefCell::new(Vec::new());
        let result = make_room(&entries, NOW, GIB, &mut roomy, &mut |entry| {
            removed.borrow_mut().push(entry.started_ms);
            Ok(())
        });
        assert_eq!(result, Ok(()));
        assert_eq!(*removed.borrow(), [NOW - 20 * HOUR]);
    }

    #[test]
    fn a_recent_entry_is_never_removed_so_the_new_operation_fails_instead() {
        let entries = [operation(0, 6 * GIB), operation(1, 2 * GIB)];
        let removed = Cell::new(0);
        let result = make_room(&entries, NOW, GIB, &mut roomy, &mut |_| {
            removed.set(removed.get() + 1);
            Ok(())
        });
        assert_eq!(result, Err(Shortage));
        assert_eq!(removed.get(), 0);
    }

    #[test]
    fn nothing_normal_is_removed_when_the_request_can_never_fit_the_budget() {
        let entries = [operation(5, GIB)];
        let removed = Cell::new(0);
        let result = make_room(&entries, NOW, BUDGET_BYTES + 1, &mut roomy, &mut |_| {
            removed.set(removed.get() + 1);
            Ok(())
        });
        assert_eq!(result, Err(Shortage));
        assert_eq!(removed.get(), 0);
    }

    #[test]
    fn a_full_disk_frees_normal_entries_then_gives_up_without_touching_recent_ones() {
        let entries = [operation(1, GIB), operation(8, GIB), operation(9, GIB)];
        let free = Cell::new(0);
        let removed = RefCell::new(Vec::new());
        let result = make_room(
            &entries,
            NOW,
            GIB,
            &mut || {
                Some(VolumeSpace {
                    free: free.get(),
                    total: 100 * GIB,
                })
            },
            &mut |entry| {
                removed.borrow_mut().push(entry.started_ms);
                Ok(())
            },
        );
        assert_eq!(result, Err(Shortage));
        assert_eq!(*removed.borrow(), [NOW - 9 * HOUR, NOW - 8 * HOUR]);
    }

    #[test]
    fn a_volume_whose_free_space_returns_after_a_removal_lets_the_operation_through() {
        let entries = [operation(8, GIB), operation(9, GIB)];
        let freed = Cell::new(0);
        let result = make_room(
            &entries,
            NOW,
            GIB,
            &mut || {
                Some(VolumeSpace {
                    free: freed.get() * 10 * GIB,
                    total: 100 * GIB,
                })
            },
            &mut |_| {
                freed.set(freed.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Ok(()));
        assert_eq!(freed.get(), 1);
    }

    #[test]
    fn the_export_folder_is_private_and_a_planted_link_is_replaced_not_followed() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = workspace.path().join("outside");
        fs::create_dir(&outside).expect("outside");
        fs::write(outside.join("keep.txt"), b"keep").expect("file");
        let root = workspace.path().join("export");
        symlink(&outside, &root).expect("link");

        secure_root(&root).expect("secured");

        assert!(
            !fs::symlink_metadata(&root)
                .expect("root")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::metadata(&root).expect("root").permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::read(outside.join("keep.txt")).expect("kept"), b"keep");
    }

    #[test]
    fn removing_an_operation_never_follows_links_out_of_the_export_folder() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = workspace.path().join("outside");
        fs::create_dir(&outside).expect("outside");
        fs::write(outside.join("keep.txt"), b"keep").expect("file");
        let root = workspace.path().join("export");
        secure_root(&root).expect("root");
        let name = operation_name(NOW - 30 * HOUR, 7);
        fs::create_dir(root.join(&name)).expect("operation");
        symlink(&outside, root.join(&name).join("escape")).expect("link");
        symlink(outside.join("keep.txt"), root.join(&name).join("file")).expect("link");

        let entries = list_entries(&root);
        assert_eq!(entries.len(), 1);
        remove_entry(&root, &entries[0]).expect("removed");

        assert!(!root.join(&name).exists());
        assert_eq!(fs::read(outside.join("keep.txt")).expect("kept"), b"keep");
    }

    #[test]
    fn a_link_named_like_an_operation_is_not_treated_as_one() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = workspace.path().join("outside");
        fs::create_dir(&outside).expect("outside");
        fs::write(outside.join("keep.txt"), b"keep").expect("file");
        let root = workspace.path().join("export");
        secure_root(&root).expect("root");
        let name = operation_name(NOW - 30 * HOUR, 7);
        symlink(&outside, root.join(&name)).expect("link");

        tidy(&root, NOW);

        assert_eq!(fs::read(outside.join("keep.txt")).expect("kept"), b"keep");
    }

    #[test]
    fn tidying_removes_unfinished_work_and_old_operations_but_keeps_the_rest() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = workspace.path().join("export");
        secure_root(&root).expect("root");
        let old = operation_name(NOW - 30 * HOUR, 1);
        let young = operation_name(NOW - HOUR, 2);
        let unfinished = operation_name(NOW - HOUR, 3);
        for name in [&old, &young] {
            fs::create_dir(root.join(name)).expect("operation");
            fs::write(root.join(name).join("a.png"), b"a").expect("file");
            fs::write(root.join(format!(".{name}.bytes")), b"1").expect("size");
        }
        fs::create_dir(root.join(format!(".{unfinished}.partial"))).expect("partial");
        fs::write(root.join(format!(".{unfinished}.bytes")), b"1").expect("orphan size");
        fs::create_dir(root.join("unknown folder")).expect("unknown");

        tidy(&root, NOW);

        assert!(!root.join(&old).exists());
        assert!(!root.join(format!(".{old}.bytes")).exists());
        assert!(root.join(&young).join("a.png").exists());
        assert!(root.join(format!(".{young}.bytes")).exists());
        assert!(!root.join(format!(".{unfinished}.partial")).exists());
        assert!(!root.join(format!(".{unfinished}.bytes")).exists());
        assert!(root.join("unknown folder").exists());
    }

    fn published(root: &std::path::Path, hours_old: u64, bytes: u64) -> std::path::PathBuf {
        secure_root(root).expect("root");
        let name = operation_name(NOW - hours_old * HOUR, hours_old);
        let staging = super::create_staging(root, &name).expect("staging");
        fs::write(staging.join("a.png"), b"a").expect("file");
        super::publish(root, &name, bytes).expect("published")
    }

    #[test]
    fn two_roots_share_one_budget_and_free_the_oldest_normal_entry_of_either() {
        let workspace = tempfile::tempdir().expect("workspace");
        let first = workspace.path().join("first");
        let second = workspace.path().join("second");
        let oldest = published(&first, 20, 3 * GIB);
        let middle = published(&second, 10, 3 * GIB);
        let recent = published(&first, 1, 3 * GIB);

        let result = make_room_across(&[&first, &second], NOW, GIB, &|_| false, &mut roomy);

        assert_eq!(result, Ok(()));
        assert!(!oldest.exists());
        assert!(middle.exists());
        assert!(recent.exists());
    }

    #[test]
    fn a_single_root_behaves_as_before() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = workspace.path().join("only");
        let aged = published(&root, 30, GIB);
        let recent = published(&root, 1, GIB);

        make_room_across(&[&root], NOW, GIB, &|_| false, &mut roomy).expect("there is room");

        assert!(!aged.exists());
        assert!(recent.exists());
    }

    #[test]
    fn a_missing_root_counts_for_nothing() {
        let workspace = tempfile::tempdir().expect("workspace");
        let missing = workspace.path().join("missing");

        let result = make_room_across(&[&missing], NOW, GIB, &|_| false, &mut roomy);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn a_protected_entry_is_kept_whatever_its_age_and_counts_as_recent() {
        let workspace = tempfile::tempdir().expect("workspace");
        let first = workspace.path().join("first");
        let second = workspace.path().join("second");
        let protected_old = published(&first, 30, 6 * GIB);
        let normal = published(&second, 10, GIB);
        let keep = protected_old.clone();

        let result = make_room_across(
            &[&first, &second],
            NOW,
            3 * GIB,
            &|path| path == keep,
            &mut roomy,
        );

        assert_eq!(result, Err(Shortage));
        assert!(protected_old.exists());
        assert!(normal.exists());
    }

    #[test]
    fn a_protected_normal_entry_is_spared_while_an_older_unprotected_one_is_freed() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = workspace.path().join("root");
        let protected_normal = published(&root, 5, 4 * GIB);
        let older = published(&root, 20, 3 * GIB);
        let keep = protected_normal.clone();

        let result = make_room_across(&[&root], NOW, 2 * GIB, &|path| path == keep, &mut roomy);

        assert_eq!(result, Ok(()));
        assert!(protected_normal.exists());
        assert!(!older.exists());
    }

    #[test]
    fn tidying_can_spare_a_protected_old_operation() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = workspace.path().join("root");
        let spared = published(&root, 30, 1);
        let removed = published(&root, 40, 1);
        let keep = spared.clone();

        tidy_except(&root, NOW, &|path| path == keep);

        assert!(spared.exists());
        assert!(!removed.exists());
    }
}
