use std::collections::HashMap;

use super::{CollectionId, Library, LibraryError};

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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewCounts {
    pub all: usize,
    pub favorites: usize,
    pub unclassified: usize,
    pub collections: HashMap<CollectionId, usize>,
}

impl ViewCounts {
    #[must_use]
    pub fn of(&self, view: AssetView) -> usize {
        match view {
            AssetView::All => self.all,
            AssetView::Favorites => self.favorites,
            AssetView::Unclassified => self.unclassified,
            AssetView::Collection(id) => self.collections.get(&id).copied().unwrap_or(0),
        }
    }
}

pub fn condition(view: AssetView) -> (&'static str, Option<CollectionId>) {
    match view {
        AssetView::All => ("?1 IS NULL", None),
        AssetView::Favorites => (FAVORITE, None),
        AssetView::Unclassified => (IN_NO_COLLECTION, None),
        AssetView::Collection(id) => (IN_SUBTREE, Some(id)),
    }
}

impl Library {
    pub fn view_counts(&self) -> Result<ViewCounts, LibraryError> {
        let count = |condition: &str| -> Result<usize, LibraryError> {
            let total: i64 = self.connection.query_row(
                &format!(
                    "SELECT count(*) FROM assets WHERE trashed_at_unix_ms IS NULL AND ({condition})"
                ),
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

        Ok(ViewCounts {
            all: count("1 = 1")?,
            favorites: count(FAVORITE)?,
            unclassified: count(IN_NO_COLLECTION)?,
            collections,
        })
    }
}
