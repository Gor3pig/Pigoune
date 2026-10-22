use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::params_from_iter;

use super::query_cache::{Counted, LinkRows};
use super::{AssetFilter, CollectionId, Library, LibraryError, SmartCollectionId, TagId};

pub const SUBTREE: &str = "WITH RECURSIVE subtree(id) AS (
        SELECT id FROM collections WHERE id = ?1 AND trashed_at_unix_ms IS NULL
        UNION
        SELECT child.id FROM collections child
        JOIN subtree ON child.parent_id = subtree.id
        WHERE child.trashed_at_unix_ms IS NULL
    )";

const IN_SUBTREE: &str = "id IN (SELECT asset_id FROM asset_collections
        WHERE collection_id IN (SELECT id FROM subtree))";

const FAVORITE: &str = "is_favorite = 1";

const EVERYTHING: &str = "1 = 1";

const NOTHING: &str = "1 = 0";

const NOT_TRASHED: &str = "trashed_at_unix_ms IS NULL";

const TRASHED: &str = "trashed_at_unix_ms IS NOT NULL";

const WITH_TAG: &str = "id IN (SELECT asset_id FROM asset_tags WHERE tag_id IN (
        WITH RECURSIVE tag_subtree(id) AS (
            SELECT id FROM tags WHERE id = ?1
            UNION
            SELECT child.id FROM tags child JOIN tag_subtree ON child.parent_id = tag_subtree.id
        )
        SELECT id FROM tag_subtree))";

const IN_NO_COLLECTION: &str = "NOT EXISTS (SELECT 1 FROM asset_collections
        JOIN collections ON collections.id = asset_collections.collection_id
        WHERE asset_collections.asset_id = assets.id AND collections.trashed_at_unix_ms IS NULL)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AssetView {
    #[default]
    All,
    Favorites,
    Unclassified,
    Collection(CollectionId),
    Tag(TagId),
    Smart(SmartCollectionId),
    Trash,
}

type LinkCounts = (HashMap<CollectionId, usize>, HashMap<TagId, usize>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewCounts {
    pub all: usize,
    pub favorites: usize,
    pub unclassified: usize,
    pub trash: usize,
    pub collections: HashMap<CollectionId, usize>,
    pub tags: HashMap<TagId, usize>,
    pub smart_collections: HashMap<SmartCollectionId, usize>,
}

impl ViewCounts {
    #[must_use]
    pub fn of(&self, view: AssetView) -> usize {
        match view {
            AssetView::All => self.all,
            AssetView::Favorites => self.favorites,
            AssetView::Unclassified => self.unclassified,
            AssetView::Collection(id) => self.collections.get(&id).copied().unwrap_or(0),
            AssetView::Tag(id) => self.tags.get(&id).copied().unwrap_or(0),
            AssetView::Smart(id) => self.smart_collections.get(&id).copied().unwrap_or(0),
            AssetView::Trash => self.trash,
        }
    }
}

pub struct ViewFilter {
    pub trash_state: &'static str,
    pub condition: &'static str,
    pub parameter: Option<String>,
}

impl ViewFilter {
    pub fn clause(&self) -> String {
        format!("{} AND ({})", self.trash_state, self.condition)
    }
}

pub fn filter(view: AssetView) -> ViewFilter {
    let (condition, parameter) = match view {
        AssetView::All | AssetView::Trash => (EVERYTHING, None),
        AssetView::Favorites => (FAVORITE, None),
        AssetView::Unclassified => (IN_NO_COLLECTION, None),
        AssetView::Collection(id) => (IN_SUBTREE, Some(id.to_string())),
        AssetView::Tag(id) => (WITH_TAG, Some(id.to_string())),
        AssetView::Smart(_) => (NOTHING, None),
    };
    let trash_state = if view == AssetView::Trash {
        TRASHED
    } else {
        NOT_TRASHED
    };
    ViewFilter {
        trash_state,
        condition,
        parameter,
    }
}

const TAG_REACH: &str = "WITH RECURSIVE reach(tag_id, ancestor_id) AS (
        SELECT id, id FROM tags
        UNION
        SELECT reach.tag_id, tags.parent_id FROM reach
        JOIN tags ON tags.id = reach.ancestor_id
        WHERE tags.parent_id IS NOT NULL
    )";

fn tag_counts_query(restricted: bool) -> String {
    if restricted {
        format!(
            "{TAG_REACH}
             SELECT reach.ancestor_id, count(DISTINCT assets.id) FROM reach
             LEFT JOIN asset_tags ON asset_tags.tag_id = reach.tag_id
             LEFT JOIN assets ON assets.id = asset_tags.asset_id
                 AND assets.trashed_at_unix_ms IS NULL
                 AND assets.id IN (SELECT value FROM json_each(?1))
             GROUP BY reach.ancestor_id"
        )
    } else {
        format!(
            "{TAG_REACH}
             SELECT reach.ancestor_id, count(DISTINCT asset_tags.asset_id) FROM reach
             LEFT JOIN asset_tags ON asset_tags.tag_id = reach.tag_id
                 AND asset_tags.asset_id NOT IN (
                     SELECT id FROM assets WHERE trashed_at_unix_ms IS NOT NULL
                 )
             GROUP BY reach.ancestor_id"
        )
    }
}

impl Library {
    pub fn view_count(&self, view: AssetView) -> Result<usize, LibraryError> {
        if let AssetView::Smart(id) = view {
            return Ok(self.smart_collection_assets(id)?.len());
        }
        let filter = filter(view);
        let clause = filter.clause();
        let total: i64 = self.connection.query_row(
            &format!("{SUBTREE} SELECT count(*) FROM assets WHERE {clause}"),
            [filter.parameter],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(total).unwrap_or(0))
    }

    pub fn view_counts(&self) -> Result<ViewCounts, LibraryError> {
        self.counts_restricted_to(None)
    }

    pub fn view_counts_matching(&self, filter: &AssetFilter) -> Result<ViewCounts, LibraryError> {
        if !filter.narrows() {
            return self.view_counts();
        }
        let filter = self.with_library_screen(filter);
        let tags = self.tag_names_by_asset()?;
        let all = self.shared_assets(AssetView::All)?;
        let assets = self.matching(all.iter(), &filter, &tags);
        let trashed = self
            .matching(self.shared_assets(AssetView::Trash)?.iter(), &filter, &tags)
            .len();
        let ids = format!(
            "[{}]",
            assets
                .iter()
                .map(|asset| format!("\"{}\"", asset.id))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut counts = self.counts_restricted_to(Some(&ids))?;
        counts.trash = trashed;
        counts.smart_collections = self
            .smart_collections()?
            .iter()
            .map(|collection| {
                (
                    collection.id,
                    self.count_among(&collection.filter, assets.iter().copied(), &tags),
                )
            })
            .collect();
        Ok(counts)
    }

    fn counts_restricted_to(&self, ids: Option<&str>) -> Result<ViewCounts, LibraryError> {
        let restriction = if ids.is_some() {
            " AND assets.id IN (SELECT value FROM json_each(?1))"
        } else {
            ""
        };
        let count = |condition: &str| -> Result<usize, LibraryError> {
            let total: i64 = self.connection.query_row(
                &format!("SELECT count(*) FROM assets WHERE {condition}{restriction}"),
                params_from_iter(ids),
                |row| row.get(0),
            )?;
            Ok(usize::try_from(total).unwrap_or(0))
        };

        let (collections, tags) = if ids.is_none() {
            self.unrestricted_link_counts()?
        } else {
            (
                Self::collection_counts(&self.connection, ids)?,
                Self::tag_counts(&self.connection, ids)?,
            )
        };

        Ok(ViewCounts {
            tags,
            smart_collections: self.smart_collection_counts()?,
            all: count(&filter(AssetView::All).clause())?,
            favorites: count(&filter(AssetView::Favorites).clause())?,
            unclassified: count(&filter(AssetView::Unclassified).clause())?,
            trash: count(&filter(AssetView::Trash).clause())?,
            collections,
        })
    }

    fn unrestricted_link_counts(&self) -> Result<LinkCounts, LibraryError> {
        let stamp = self.connection.total_changes();
        let cacheable = self.connection.is_autocommit();
        let (known_collections, known_tags) = if cacheable {
            let cache = self.query_cache.lock(stamp);
            (cache.collection_counts.clone(), cache.tag_counts.clone())
        } else {
            (None, None)
        };
        let rows = if cacheable && (known_collections.is_none() || known_tags.is_none()) {
            Some(self.link_rows()?)
        } else {
            None
        };
        let collections = if let Some(known) = known_collections {
            known
        } else {
            let fresh = Arc::new(Counted {
                rows: rows.map_or((0, 0, 0), LinkRows::behind_collection_counts),
                counts: Self::collection_counts(&self.connection, None)?,
            });
            if rows.is_some() {
                self.query_cache.lock(stamp).collection_counts = Some(fresh.clone());
            }
            fresh
        };
        let tags = if let Some(known) = known_tags {
            known
        } else {
            let fresh = Arc::new(Counted {
                rows: rows.map_or((0, 0, 0), LinkRows::behind_tag_counts),
                counts: Self::tag_counts(&self.connection, None)?,
            });
            if rows.is_some() {
                self.query_cache.lock(stamp).tag_counts = Some(fresh.clone());
            }
            fresh
        };
        Ok((collections.counts.clone(), tags.counts.clone()))
    }

    pub(super) fn link_rows(&self) -> Result<LinkRows, LibraryError> {
        let count = |value: i64| usize::try_from(value).unwrap_or(0);
        let (tags, asset_tags, collections, asset_collections, trashed_assets): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = self.connection.query_row(
            "SELECT (SELECT count(*) FROM tags),
                    (SELECT count(*) FROM asset_tags),
                    (SELECT count(*) FROM collections),
                    (SELECT count(*) FROM asset_collections),
                    (SELECT count(*) FROM assets WHERE trashed_at_unix_ms IS NOT NULL)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        Ok(LinkRows {
            tags: count(tags),
            asset_tags: count(asset_tags),
            collections: count(collections),
            asset_collections: count(asset_collections),
            trashed_assets: count(trashed_assets),
        })
    }

    fn collection_counts(
        connection: &rusqlite::Connection,
        ids: Option<&str>,
    ) -> Result<HashMap<CollectionId, usize>, LibraryError> {
        let restriction = if ids.is_some() {
            " AND assets.id IN (SELECT value FROM json_each(?1))"
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "WITH RECURSIVE ancestry(collection_id, ancestor_id) AS (
                 SELECT id, id FROM collections WHERE trashed_at_unix_ms IS NULL
                 UNION
                 SELECT ancestry.collection_id, collections.parent_id FROM ancestry
                 JOIN collections ON collections.id = ancestry.ancestor_id
                 WHERE collections.parent_id IS NOT NULL
             )
             SELECT ancestry.ancestor_id, count(DISTINCT assets.id) FROM ancestry
             JOIN asset_collections ON asset_collections.collection_id = ancestry.collection_id
             JOIN assets ON assets.id = asset_collections.asset_id
             WHERE assets.trashed_at_unix_ms IS NULL{restriction}
             GROUP BY ancestry.ancestor_id"
        ))?;
        let counts = statement
            .query_map(params_from_iter(ids), |row| {
                let total: i64 = row.get(1)?;
                Ok((row.get(0)?, usize::try_from(total).unwrap_or(0)))
            })?
            .collect::<Result<_, _>>()?;
        Ok(counts)
    }

    fn tag_counts(
        connection: &rusqlite::Connection,
        ids: Option<&str>,
    ) -> Result<HashMap<TagId, usize>, LibraryError> {
        let mut statement = connection.prepare(&tag_counts_query(ids.is_some()))?;
        let counts = statement
            .query_map(params_from_iter(ids), |row| {
                let total: i64 = row.get(1)?;
                Ok((row.get(0)?, usize::try_from(total).unwrap_or(0)))
            })?
            .collect::<Result<_, _>>()?;
        Ok(counts)
    }
}

#[cfg(test)]
mod count_cache_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::{AssetView, CollectionId, Counted, Library, TagId};
    use crate::library::query_cache::LinkRows;
    use crate::library::{
        AssetCommand, AssetFilter, AssetId, CollectionCommand, ImportControl, ImportSummary,
        TagCommand,
    };
    use crate::{AssetColor, DominantColor, Rgb};

    struct Fixture {
        workspace: tempfile::TempDir,
        library: Library,
        first: AssetId,
        collection: CollectionId,
        other: CollectionId,
        made: usize,
    }

    fn fixture() -> Fixture {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let mut ids = Vec::new();
        for name in ["red-dot.png", "blue-photo.jpg"] {
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            match library.import_file(&source, None).expect("import succeeds") {
                crate::library::ImportOutcome::Imported(id) => ids.push(id),
                other => panic!("unexpected {other:?}"),
            }
        }
        let collection = library.create_collection("Une", None).expect("collection");
        let other = library
            .create_collection("Autre", None)
            .expect("collection");
        library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection,
                assets: vec![ids[0]],
            })
            .expect("added");
        library
            .apply_tag_command(&TagCommand::Add {
                assets: vec![ids[0]],
                name: "alpha".to_owned(),
            })
            .expect("tagged");
        Fixture {
            workspace,
            library,
            first: ids[0],
            collection,
            other,
            made: 0,
        }
    }

    type Kept = (
        Option<Arc<Counted<TagId>>>,
        Option<Arc<Counted<CollectionId>>>,
    );

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

        fn import_into(
            &mut self,
            paths: &[PathBuf],
            target: Option<CollectionId>,
        ) -> ImportSummary {
            self.library
                .import_paths(paths, target, |_| true, |_| ImportControl::Continue)
                .expect("the import runs")
        }

        fn warm(&self) -> Kept {
            self.library.view_counts().expect("counts");
            self.kept()
        }

        fn kept(&self) -> Kept {
            let cache = self
                .library
                .query_cache
                .lock(self.library.connection.total_changes());
            (cache.tag_counts.clone(), cache.collection_counts.clone())
        }

        fn assert_counts_are_recomputed_ones(&self) {
            let shown = self.library.view_counts().expect("counts");
            let tags = Library::tag_counts(&self.library.connection, None).expect("tags");
            let collections =
                Library::collection_counts(&self.library.connection, None).expect("collections");
            assert_eq!(shown.tags, tags);
            assert_eq!(shown.collections, collections);
        }
    }

    fn same<K>(left: Option<&Arc<Counted<K>>>, right: Option<&Arc<Counted<K>>>) -> bool {
        matches!((left, right), (Some(left), Some(right)) if Arc::ptr_eq(left, right))
    }

    fn nothing_kept(kept: &Kept) -> bool {
        kept.0.is_none() && kept.1.is_none()
    }

    #[test]
    fn counts_are_cached_at_the_first_unrestricted_read_and_reused() {
        let fixture = fixture();
        assert!(nothing_kept(&fixture.kept()));

        let warm = fixture.warm();
        fixture.library.view_counts().expect("counts");

        assert!(warm.0.is_some() && warm.1.is_some());
        let again = fixture.kept();
        assert!(same(warm.0.as_ref(), again.0.as_ref()) && same(warm.1.as_ref(), again.1.as_ref()));
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn a_plain_completed_import_keeps_both_maps_and_the_counts_stay_exact() {
        let mut fixture = fixture();
        let warm = fixture.warm();
        let paths = [fixture.variant(), fixture.variant()];

        let summary = fixture.import_into(&paths, None);

        assert_eq!(summary.imported.len(), 2);
        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()) && same(warm.1.as_ref(), kept.1.as_ref()));
        fixture.assert_counts_are_recomputed_ones();
        let counts = fixture.library.view_counts().expect("counts");
        assert_eq!((counts.all, counts.unclassified), (4, 3));
    }

    #[test]
    fn an_import_into_a_collection_keeps_only_the_tag_counts() {
        let mut fixture = fixture();
        let warm = fixture.warm();
        let paths = [fixture.variant()];

        fixture.import_into(&paths, Some(fixture.collection));

        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()));
        assert!(kept.1.is_none());
        fixture.assert_counts_are_recomputed_ones();
        let counts = fixture.library.view_counts().expect("counts");
        assert_eq!(counts.of(AssetView::Collection(fixture.collection)), 2);
    }

    #[test]
    fn known_items_added_to_another_collection_drop_the_collection_counts() {
        let mut fixture = fixture();
        let paths = [fixture.variant()];
        fixture.import_into(&paths, None);
        let warm = fixture.warm();

        let summary = fixture.import_into(&paths, Some(fixture.other));

        assert_eq!(summary.added_to_collection, 1);
        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()));
        assert!(kept.1.is_none());
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn a_folder_import_creating_collections_drops_the_collection_counts() {
        let mut fixture = fixture();
        let folder = fixture.workspace.path().join("Dossier");
        fs::create_dir_all(&folder).expect("folder");
        let inside = fixture.variant();
        fs::rename(&inside, folder.join("inside.png")).expect("moved");
        let before = fixture
            .library
            .visible_collections()
            .expect("collections")
            .len();
        let warm = fixture.warm();

        fixture.import_into(&[folder], None);

        let after = fixture
            .library
            .visible_collections()
            .expect("collections")
            .len();
        assert!(after > before);
        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()));
        assert!(kept.1.is_none());
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn a_restoration_from_the_trash_drops_both_maps() {
        let mut fixture = fixture();
        let paths = [fixture.variant()];
        let id = fixture.import_into(&paths, None).imported[0];
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: vec![id],
                trashed: true,
            })
            .expect("trashed");
        fixture.warm();

        let summary = fixture.import_into(&paths, None);

        assert_eq!(summary.restored_from_trash, 1);
        assert!(nothing_kept(&fixture.kept()));
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn a_cancelled_import_drops_both_maps() {
        let mut fixture = fixture();
        fixture.warm();
        let paths = [fixture.variant(), fixture.variant(), fixture.variant()];

        fixture
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

        assert!(nothing_kept(&fixture.kept()));
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn duplicates_and_unreadable_files_keep_the_maps() {
        let mut fixture = fixture();
        let warm = fixture.warm();
        let duplicate = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png");
        let missing = fixture.workspace.path().join("missing.png");

        fixture.import_into(&[duplicate, missing], None);

        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()) && same(warm.1.as_ref(), kept.1.as_ref()));
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn any_other_write_drops_both_maps_and_the_next_read_is_exact() {
        let mut fixture = fixture();
        fixture.warm();
        fixture
            .library
            .apply_tag_command(&TagCommand::Add {
                assets: vec![fixture.first],
                name: "beta".to_owned(),
            })
            .expect("tagged");
        assert!(nothing_kept(&fixture.kept()));
        fixture.assert_counts_are_recomputed_ones();
        let others = [
            AssetCommand::SetFavorite {
                assets: vec![fixture.first],
                favorite: true,
            },
            AssetCommand::SetTrashed {
                assets: vec![fixture.first],
                trashed: true,
            },
        ];
        for command in others {
            fixture.warm();
            fixture
                .library
                .apply_asset_command(&command)
                .expect("applied");
            assert!(nothing_kept(&fixture.kept()));
            fixture.assert_counts_are_recomputed_ones();
        }
    }

    #[test]
    fn a_membership_change_that_keeps_other_caches_never_keeps_the_collection_counts() {
        let mut fixture = fixture();
        let warm = fixture.warm();
        fixture
            .library
            .apply_collection_command(&CollectionCommand::AddAssets {
                collection: fixture.other,
                assets: vec![fixture.first],
            })
            .expect("added");

        let kept = fixture.kept();
        assert!(nothing_kept(&kept));
        assert!(warm.0.is_some());
        fixture.assert_counts_are_recomputed_ones();
        let counts = fixture.library.view_counts().expect("counts");
        assert_eq!(counts.of(AssetView::Collection(fixture.other)), 1);
    }

    #[test]
    fn deferred_colour_writes_drop_the_maps() {
        let mut fixture = fixture();
        fixture.warm();
        fixture
            .library
            .record_colors_batch(&[(
                fixture.first,
                vec![DominantColor {
                    family: AssetColor::Red,
                    average: Rgb::new(200, 10, 10),
                }],
            )])
            .expect("colours");

        assert!(nothing_kept(&fixture.kept()));
        fixture.assert_counts_are_recomputed_ones();
    }

    #[test]
    fn counts_restricted_by_a_filter_never_reuse_or_fill_the_unrestricted_ones() {
        let fixture = fixture();
        let filtered = fixture
            .library
            .view_counts_matching(&AssetFilter::text("alpha"))
            .expect("counts");
        assert!(nothing_kept(&fixture.kept()));
        let warm = fixture.warm();

        let again = fixture
            .library
            .view_counts_matching(&AssetFilter::text("alpha"))
            .expect("counts");

        assert_eq!(filtered, again);
        assert_eq!(filtered.all, 1);
        assert_eq!(fixture.library.view_counts().expect("counts").all, 2);
        let kept = fixture.kept();
        assert!(same(warm.0.as_ref(), kept.0.as_ref()) && same(warm.1.as_ref(), kept.1.as_ref()));
    }

    #[test]
    fn filtered_counts_hold_the_values_of_the_filter_not_the_cached_unfiltered_ones() {
        let fixture = fixture();
        let alpha = fixture
            .library
            .tag_named("alpha")
            .expect("read")
            .expect("tag")
            .id;
        let unfiltered = fixture.warm();
        assert!(unfiltered.0.is_some());
        let unfiltered_counts = fixture.library.view_counts().expect("counts");
        assert_eq!(unfiltered_counts.of(AssetView::Tag(alpha)), 1);
        assert_eq!(
            unfiltered_counts.of(AssetView::Collection(fixture.collection)),
            1
        );

        let filtered = fixture
            .library
            .view_counts_matching(&AssetFilter::text("blue-photo"))
            .expect("counts");

        assert_eq!(filtered.all, 1);
        assert_eq!(filtered.of(AssetView::Tag(alpha)), 0);
        assert_eq!(filtered.of(AssetView::Collection(fixture.collection)), 0);
    }

    #[test]
    fn an_adoption_whose_link_rows_moved_drops_exactly_the_maps_they_concern() {
        let fixture = fixture();
        let warm = fixture.warm();
        let stamp = fixture.library.connection.total_changes();
        let rows = fixture.library.asset_rows().expect("rows");
        let links = fixture.library.link_rows().expect("links");

        let accepted = fixture.library.query_cache.adopt(
            stamp,
            stamp + 1,
            Vec::new(),
            rows,
            LinkRows {
                asset_tags: links.asset_tags + 1,
                ..links
            },
        );

        assert!(accepted);
        let cache = fixture.library.query_cache.lock(stamp + 1);
        assert!(cache.tag_counts.is_none());
        assert!(
            matches!(&cache.collection_counts, Some(kept) if Arc::ptr_eq(kept, warm.1.as_ref().expect("warm")))
        );
    }

    #[test]
    fn counts_are_not_cached_inside_a_transaction() {
        let fixture = fixture();
        fixture
            .library
            .connection
            .execute_batch("BEGIN")
            .expect("transaction");

        fixture.library.view_counts().expect("counts");

        fixture
            .library
            .connection
            .execute_batch("ROLLBACK")
            .expect("rolled back");
        assert!(nothing_kept(&fixture.kept()));
    }
}
