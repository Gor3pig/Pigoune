use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::import::{self, ImportOutcome};
use super::import_batch::ImportBatch;
use super::import_plan::{ImportPlan, PlannedFile};
use super::{AssetId, CollectionId, ImportError, Library, LibraryError, collection};

pub const LARGE_FILE_BYTES: u64 = 50_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportProgress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportControl {
    Continue,
    Cancel,
}

#[derive(Debug, Default)]
pub enum ImportEnding {
    #[default]
    Completed,
    Cancelled,
    Interrupted(ImportError),
}

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: Vec<AssetId>,
    pub already_known: Vec<AssetId>,
    pub large_imported: usize,
    pub already_present: usize,
    pub added_to_collection: usize,
    pub restored_from_trash: usize,
    pub unsupported: usize,
    pub unreadable: Vec<PathBuf>,
    pub ignored_links: usize,
    pub ending: ImportEnding,
}

#[derive(Clone, Copy)]
struct SavedSummary {
    imported: usize,
    already_known: usize,
    large_imported: usize,
    already_present: usize,
    added_to_collection: usize,
    restored_from_trash: usize,
}

impl ImportSummary {
    fn saved(&self) -> SavedSummary {
        SavedSummary {
            imported: self.imported.len(),
            already_known: self.already_known.len(),
            large_imported: self.large_imported,
            already_present: self.already_present,
            added_to_collection: self.added_to_collection,
            restored_from_trash: self.restored_from_trash,
        }
    }

    fn return_to(&mut self, saved: SavedSummary) {
        self.imported.truncate(saved.imported);
        self.already_known.truncate(saved.already_known);
        self.large_imported = saved.large_imported;
        self.already_present = saved.already_present;
        self.added_to_collection = saved.added_to_collection;
        self.restored_from_trash = saved.restored_from_trash;
    }

    fn record(&mut self, result: Result<(ImportOutcome, u64), ImportError>) -> Option<ImportError> {
        match result {
            Ok((ImportOutcome::Imported(id), byte_size)) => {
                self.imported.push(id);
                if byte_size > LARGE_FILE_BYTES {
                    self.large_imported += 1;
                }
            }
            Ok((ImportOutcome::AlreadyPresent(id), _)) => {
                self.already_present += 1;
                self.already_known.push(id);
            }
            Ok((ImportOutcome::AddedToCollection(id), _)) => {
                self.added_to_collection += 1;
                self.already_known.push(id);
            }
            Ok((ImportOutcome::RestoredFromTrash(id), _)) => {
                self.restored_from_trash += 1;
                self.already_known.push(id);
            }
            Err(ImportError::UnsupportedFormat(_)) => self.unsupported += 1,
            Err(ImportError::Unreadable(path)) => self.unreadable.push(path),
            Err(serious) => return Some(serious),
        }
        None
    }
}

impl ImportSummary {
    fn leaves_cached_assets_complete(&self) -> bool {
        matches!(self.ending, ImportEnding::Completed) && self.restored_from_trash == 0
    }
}

impl Library {
    pub fn import_paths(
        &mut self,
        paths: &[PathBuf],
        target: Option<CollectionId>,
        is_intact: impl FnMut(&Path) -> bool,
        on_progress: impl FnMut(ImportProgress) -> ImportControl,
    ) -> Result<ImportSummary, ImportError> {
        let outside_transaction = self.connection.is_autocommit();
        let before = self.connection.total_changes();
        let result = self.import_in_batches(paths, target, is_intact, on_progress);
        if outside_transaction
            && let Ok(summary) = &result
            && summary.leaves_cached_assets_complete()
        {
            self.keep_queries_cached_after_import(before, &summary.imported);
        }
        result
    }

    fn keep_queries_cached_after_import(&self, before: u64, imported: &[AssetId]) {
        if !self.connection.is_autocommit() {
            return;
        }
        let Ok(mut found) = self.assets_among(imported) else {
            return;
        };
        let Some(created) = imported
            .iter()
            .map(|id| found.remove(id))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let Ok(rows) = self.asset_rows() else {
            return;
        };
        self.query_cache
            .adopt(before, self.connection.total_changes(), created, rows);
    }

    fn import_in_batches(
        &mut self,
        paths: &[PathBuf],
        target: Option<CollectionId>,
        mut is_intact: impl FnMut(&Path) -> bool,
        mut on_progress: impl FnMut(ImportProgress) -> ImportControl,
    ) -> Result<ImportSummary, ImportError> {
        self.ensure_target_is_usable(target)?;
        let plan = ImportPlan::of(paths);
        let total = plan.files.len();
        let mut summary = ImportSummary {
            unreadable: plan.unreadable,
            ignored_links: plan.ignored_links,
            ..ImportSummary::default()
        };
        let mut folders = FolderCollections::new(target);
        let mut batch = ImportBatch::begin(&self.connection)?;
        let mut saved = summary.saved();

        for (done, file) in plan.files.iter().enumerate() {
            if on_progress(ImportProgress { done, total }) == ImportControl::Cancel {
                summary.ending = ImportEnding::Cancelled;
                break;
            }
            let result = self.import_planned(file, &mut folders, &mut is_intact, &mut batch);
            if let Some(serious) = summary.record(result) {
                summary.ending = ImportEnding::Interrupted(serious);
                break;
            }
            if batch.is_due() {
                if let Err(failure) = batch.commit(&self.connection) {
                    summary.return_to(saved);
                    summary.ending = ImportEnding::Interrupted(failure);
                    return Ok(summary);
                }
                saved = summary.saved();
                batch = ImportBatch::begin(&self.connection)?;
            }
        }
        if let Err(failure) = batch.commit(&self.connection) {
            summary.return_to(saved);
            summary.ending = ImportEnding::Interrupted(failure);
        }
        Ok(summary)
    }

    fn import_planned(
        &mut self,
        file: &PlannedFile,
        folders: &mut FolderCollections,
        is_intact: &mut impl FnMut(&Path) -> bool,
        batch: &mut ImportBatch,
    ) -> Result<(ImportOutcome, u64), ImportError> {
        let prepared = import::prepare(&file.source)?;
        if !self.is_already_stored(&prepared)? && !is_intact(&file.source) {
            return Err(ImportError::Unreadable(file.source.clone()));
        }
        let outcome = self.store_prepared(
            &prepared,
            |connection| folders.resolve(connection, &file.folders),
            batch,
        )?;
        Ok((outcome, prepared.byte_size()))
    }
}

struct FolderCollections {
    target: Option<CollectionId>,
    known: HashMap<Vec<String>, CollectionId>,
}

impl FolderCollections {
    fn new(target: Option<CollectionId>) -> Self {
        Self {
            target,
            known: HashMap::new(),
        }
    }

    fn resolve(
        &mut self,
        connection: &Connection,
        folders: &[String],
    ) -> Result<Option<CollectionId>, LibraryError> {
        let mut parent = self.target;
        let mut key = Vec::with_capacity(folders.len());
        for name in folders {
            key.push(collection::comparable_name(name));
            let id = if let Some(&known) = self.known.get(&key) {
                known
            } else {
                let found = find_or_create(connection, parent, name)?;
                self.known.insert(key.clone(), found);
                found
            };
            parent = Some(id);
        }
        Ok(parent)
    }
}

fn find_or_create(
    connection: &Connection,
    parent: Option<CollectionId>,
    name: &str,
) -> Result<CollectionId, LibraryError> {
    let name = name.trim();
    match collection::find_child_named(connection, parent, name)? {
        Some(existing) => Ok(existing),
        None => collection::insert_collection(connection, name, parent),
    }
}

#[cfg(test)]
mod cache_tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::{ImportControl, ImportEnding, ImportSummary};
    use crate::library::query_cache::AssetRows;
    use crate::library::{AssetCommand, AssetView, ImportOutcome, Library};

    struct Fixture {
        workspace: tempfile::TempDir,
        library: Library,
        made: usize,
    }

    fn fixture() -> Fixture {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        for name in ["red-dot.png", "blue-photo.jpg"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            library.import_file(&source, None).expect("import succeeds");
        }
        Fixture {
            workspace,
            library,
            made: 0,
        }
    }

    impl Fixture {
        fn variant(&mut self) -> PathBuf {
            self.made += 1;
            let mut bytes =
                fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png"))
                    .expect("sample is read");
            bytes.extend_from_slice(format!("#{}", self.made).as_bytes());
            let path = self
                .workspace
                .path()
                .join(format!("variant-{}.png", self.made));
            fs::write(&path, bytes).expect("variant is written");
            path
        }

        fn import(&mut self, paths: &[PathBuf]) -> ImportSummary {
            self.library
                .import_paths(paths, None, |_| true, |_| ImportControl::Continue)
                .expect("the import runs")
        }

        fn warm(
            &self,
        ) -> (
            Arc<Vec<crate::library::Asset>>,
            Arc<Vec<crate::library::Asset>>,
        ) {
            let all = self.library.shared_assets(AssetView::All).expect("listing");
            let trash = self.library.shared_assets(AssetView::Trash).expect("trash");
            self.library.tag_names_by_asset().expect("tag names");
            (all, trash)
        }

        fn cached(&self) -> (bool, bool, bool) {
            let cache = self
                .library
                .query_cache
                .lock(self.library.connection.total_changes());
            (
                cache.all.is_some(),
                cache.trash.is_some(),
                cache.tags.is_some(),
            )
        }
    }

    #[test]
    fn a_completed_import_keeps_the_warm_caches_and_completes_the_listing() {
        let mut fixture = fixture();
        let (_, trash) = fixture.warm();
        let tags = fixture.library.tag_names_by_asset().expect("tag names");
        let paths = [fixture.variant(), fixture.variant()];

        let summary = fixture.import(&paths);

        assert_eq!(summary.imported.len(), 2);
        let kept = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(kept.len(), 4);
        assert_eq!(
            *kept,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
        assert!(Arc::ptr_eq(
            &fixture
                .library
                .shared_assets(AssetView::Trash)
                .expect("trash"),
            &trash
        ));
        assert!(Arc::ptr_eq(
            &fixture.library.tag_names_by_asset().expect("tag names"),
            &tags
        ));
    }

    #[test]
    fn a_cache_that_was_never_filled_is_not_filled_by_an_import() {
        let mut fixture = fixture();
        let paths = [fixture.variant()];

        fixture.import(&paths);

        assert_eq!(fixture.cached(), (false, false, false));
    }

    #[test]
    fn only_the_caches_that_were_warm_survive_and_the_listing_is_not_invented() {
        let mut fixture = fixture();
        fixture.library.tag_names_by_asset().expect("tag names");
        let paths = [fixture.variant()];

        fixture.import(&paths);

        assert_eq!(fixture.cached(), (false, false, true));
    }

    #[test]
    fn a_cancelled_import_drops_every_cache() {
        let mut fixture = fixture();
        fixture.warm();
        let paths = [fixture.variant(), fixture.variant(), fixture.variant()];

        let summary = fixture
            .library
            .import_paths(
                &paths,
                None,
                |_| true,
                |progress| {
                    if progress.done == 1 {
                        ImportControl::Cancel
                    } else {
                        ImportControl::Continue
                    }
                },
            )
            .expect("the import runs");

        assert!(matches!(summary.ending, ImportEnding::Cancelled));
        assert_eq!(fixture.cached(), (false, false, false));
    }

    #[test]
    fn an_import_interrupted_before_any_write_leaves_the_caches_exact() {
        let mut fixture = fixture();
        let (all, _) = fixture.warm();
        let paths = [fixture.variant()];
        let files = fixture.library.root.join("files");
        let moved = fixture.workspace.path().join("files-away");
        fs::rename(&files, &moved).expect("the folder moves");
        symlink(&moved, &files).expect("a link takes its place");

        let summary = fixture.import(&paths);

        assert!(matches!(summary.ending, ImportEnding::Interrupted(_)));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(*shown, *all);
        assert_eq!(
            *shown,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
    }

    #[test]
    fn an_import_interrupted_after_some_work_drops_every_cache() {
        let mut fixture = fixture();
        fixture.warm();
        let paths = [fixture.variant(), fixture.variant()];
        let files = fixture.library.root.join("files");
        let moved = fixture.workspace.path().join("files-away");

        let summary = fixture
            .library
            .import_paths(
                &paths,
                None,
                |_| true,
                |progress| {
                    if progress.done == 1 {
                        fs::rename(&files, &moved).expect("the folder moves");
                        symlink(&moved, &files).expect("a link takes its place");
                    }
                    ImportControl::Continue
                },
            )
            .expect("the import runs");

        assert!(matches!(summary.ending, ImportEnding::Interrupted(_)));
        assert_eq!(fixture.cached(), (false, false, false));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(
            *shown,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
    }

    #[test]
    fn a_restoration_from_the_trash_drops_every_cache() {
        let mut fixture = fixture();
        let paths = [fixture.variant()];
        let id = fixture.import(&paths).imported[0];
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: vec![id],
                trashed: true,
            })
            .expect("trashed");
        fixture.warm();

        let summary = fixture.import(&paths);

        assert_eq!(summary.restored_from_trash, 1);
        assert_eq!(fixture.cached(), (false, false, false));
    }

    #[test]
    fn a_cache_already_stale_before_the_import_is_not_revived() {
        let mut fixture = fixture();
        fixture.warm();
        let first = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing")[0]
            .id;
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: vec![first],
                favorite: true,
            })
            .expect("favorite");
        let paths = [fixture.variant()];

        fixture.import(&paths);

        assert_eq!(fixture.cached(), (false, false, false));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert!(
            shown
                .iter()
                .any(|asset| asset.id == first && asset.is_favorite)
        );
        assert_eq!(
            *shown,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
    }

    #[test]
    fn duplicates_and_unreadable_files_keep_the_caches_and_change_nothing_visible() {
        let mut fixture = fixture();
        let (all, trash) = fixture.warm();
        let duplicate = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png");
        let missing = fixture.workspace.path().join("missing.png");

        let summary = fixture.import(&[duplicate, missing]);

        assert_eq!(summary.already_present, 1);
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(*shown, *all);
        assert!(Arc::ptr_eq(
            &fixture
                .library
                .shared_assets(AssetView::Trash)
                .expect("trash"),
            &trash
        ));
    }

    #[test]
    fn an_adoption_whose_row_counts_do_not_add_up_is_refused_and_a_true_one_is_accepted() {
        let fixture = fixture();
        let (all, _) = fixture.warm();
        let stamp = fixture.library.connection.total_changes();
        let rows = fixture.library.asset_rows().expect("rows");

        let refused = fixture.library.query_cache.adopt(
            stamp,
            stamp + 1,
            Vec::new(),
            AssetRows {
                visible: rows.visible + 1,
                trashed: rows.trashed,
            },
        );
        let refused_trash = fixture.library.query_cache.adopt(
            stamp,
            stamp + 1,
            Vec::new(),
            AssetRows {
                visible: rows.visible,
                trashed: rows.trashed + 1,
            },
        );
        let accepted = fixture
            .library
            .query_cache
            .adopt(stamp, stamp + 1, Vec::new(), rows);

        assert!(!refused && !refused_trash && accepted);
        let cache = fixture.library.query_cache.lock(stamp + 1);
        assert!(cache.all.as_ref().is_some_and(|kept| **kept == *all));
    }

    #[test]
    fn imports_placed_among_newer_and_older_items_follow_the_listing_order() {
        let mut fixture = fixture();
        let ids: Vec<_> = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing")
            .iter()
            .map(|asset| asset.id)
            .collect();
        fixture
            .library
            .connection
            .execute(
                "UPDATE assets SET added_at_unix_ms = 99999999999990 WHERE id = ?1",
                [ids[0]],
            )
            .expect("one item is in the future");
        fixture
            .library
            .connection
            .execute(
                "UPDATE assets SET added_at_unix_ms = 1 WHERE id = ?1",
                [ids[1]],
            )
            .expect("one item is in the past");
        fixture.warm();
        let paths = [fixture.variant(), fixture.variant()];

        fixture.import(&paths);

        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(
            *shown,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
        assert_eq!(shown.len(), 4);
        assert_eq!(shown[0].id, ids[0]);
        assert_eq!(shown[3].id, ids[1]);
    }

    #[test]
    fn an_import_variant_never_used_as_cached_state_is_only_what_the_database_holds() {
        let mut fixture = fixture();
        fixture.warm();
        let paths = [fixture.variant()];
        fixture.import(&paths);

        let outcome = fixture
            .library
            .import_file(&paths[0], None)
            .expect("the same file again");

        assert!(matches!(outcome, ImportOutcome::AlreadyPresent(_)));
        let shown = fixture
            .library
            .shared_assets(AssetView::All)
            .expect("listing");
        assert_eq!(
            *shown,
            fixture.library.load_assets(AssetView::All).expect("fresh")
        );
    }
}
