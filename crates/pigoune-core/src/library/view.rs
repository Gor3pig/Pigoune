use std::collections::HashMap;

use super::{CollectionId, Library, LibraryError, SmartCollectionId, TagId};

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

const WITH_TAG: &str = "id IN (SELECT asset_id FROM asset_tags WHERE tag_id = ?1)";

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

impl Library {
    pub fn view_counts(&self) -> Result<ViewCounts, LibraryError> {
        let count = |condition: &str| -> Result<usize, LibraryError> {
            let total: i64 = self.connection.query_row(
                &format!("SELECT count(*) FROM assets WHERE {condition}"),
                [],
                |row| row.get(0),
            )?;
            Ok(usize::try_from(total).unwrap_or(0))
        };

        let mut statement = self.connection.prepare(
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
             WHERE assets.trashed_at_unix_ms IS NULL
             GROUP BY ancestry.ancestor_id",
        )?;
        let collections = statement
            .query_map([], |row| {
                let total: i64 = row.get(1)?;
                Ok((row.get(0)?, usize::try_from(total).unwrap_or(0)))
            })?
            .collect::<Result<_, _>>()?;

        let mut statement = self.connection.prepare(
            "SELECT tags.id, count(assets.id) FROM tags
             LEFT JOIN asset_tags ON asset_tags.tag_id = tags.id
             LEFT JOIN assets ON assets.id = asset_tags.asset_id
                 AND assets.trashed_at_unix_ms IS NULL
             GROUP BY tags.id",
        )?;
        let tags = statement
            .query_map([], |row| {
                let total: i64 = row.get(1)?;
                Ok((row.get(0)?, usize::try_from(total).unwrap_or(0)))
            })?
            .collect::<Result<_, _>>()?;

        let smart_collections = self
            .smart_collections()?
            .into_iter()
            .map(|collection| {
                let total = self.smart_collection_assets(collection.id)?.len();
                Ok((collection.id, total))
            })
            .collect::<Result<_, LibraryError>>()?;

        Ok(ViewCounts {
            tags,
            smart_collections,
            all: count(&filter(AssetView::All).clause())?,
            favorites: count(&filter(AssetView::Favorites).clause())?,
            unclassified: count(&filter(AssetView::Unclassified).clause())?,
            trash: count(&filter(AssetView::Trash).clause())?,
            collections,
        })
    }
}
