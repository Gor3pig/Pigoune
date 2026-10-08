use rusqlite::{Connection, OptionalExtension, Row};

use super::{AssetId, Library, LibraryError, TagId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub parent: Option<TagId>,
}

const COLUMNS: &str = "tags.id, tags.name, tags.parent_id";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        parent: row.get(2)?,
    })
}

impl Library {
    pub fn tags(&self) -> Result<Vec<Tag>, LibraryError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM tags ORDER BY tags.normalized_name, tags.id"
        ))?;
        let tags = statement
            .query_map([], from_row)?
            .collect::<Result<_, _>>()?;
        Ok(tags)
    }

    pub fn tags_of(&self, asset: AssetId) -> Result<Vec<Tag>, LibraryError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM tags
             JOIN asset_tags ON asset_tags.tag_id = tags.id
             WHERE asset_tags.asset_id = ?1
             ORDER BY tags.normalized_name, tags.id"
        ))?;
        let tags = statement
            .query_map([asset], from_row)?
            .collect::<Result<_, _>>()?;
        Ok(tags)
    }

    pub fn tag_named(&self, name: &str) -> Result<Option<Tag>, LibraryError> {
        find_child(&self.connection, None, name)
    }

    pub fn tag_named_in(&self, parent: TagId, name: &str) -> Result<Option<Tag>, LibraryError> {
        find_child(&self.connection, Some(parent), name)
    }

    pub fn tag_path(&self, tag: TagId) -> Result<Vec<Tag>, LibraryError> {
        ancestry(&self.connection, tag)
    }
}

pub fn normalized(name: &str) -> String {
    name.trim().to_lowercase()
}

pub fn find_child(
    connection: &Connection,
    parent: Option<TagId>,
    name: &str,
) -> Result<Option<Tag>, LibraryError> {
    Ok(connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM tags
                 WHERE ifnull(parent_id, '') = ifnull(?1, '') AND normalized_name = ?2"
            ),
            rusqlite::params![parent, normalized(name)],
            from_row,
        )
        .optional()?)
}

pub fn ancestry(connection: &Connection, tag: TagId) -> Result<Vec<Tag>, LibraryError> {
    let mut path = Vec::new();
    let mut next = Some(tag);
    while let Some(id) = next {
        let found = connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM tags WHERE id = ?1"),
                [id],
                from_row,
            )
            .optional()?;
        let Some(found) = found else { break };
        next = found.parent;
        path.push(found);
    }
    path.reverse();
    Ok(path)
}

pub fn subtree(connection: &Connection, tag: TagId) -> Result<Vec<Tag>, LibraryError> {
    let mut statement = connection.prepare(&format!(
        "WITH RECURSIVE below(id, depth) AS (
             SELECT id, 0 FROM tags WHERE id = ?1
             UNION ALL
             SELECT tags.id, below.depth + 1 FROM tags JOIN below ON tags.parent_id = below.id
         )
         SELECT {COLUMNS} FROM tags JOIN below ON below.id = tags.id
         ORDER BY below.depth, tags.normalized_name, tags.id"
    ))?;
    let tags = statement
        .query_map([tag], from_row)?
        .collect::<Result<_, _>>()?;
    Ok(tags)
}

pub fn children(connection: &Connection, tag: TagId) -> Result<Vec<Tag>, LibraryError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM tags WHERE parent_id = ?1 ORDER BY normalized_name, id"
    ))?;
    let tags = statement
        .query_map([tag], from_row)?
        .collect::<Result<_, _>>()?;
    Ok(tags)
}
