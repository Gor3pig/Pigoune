use rusqlite::{Connection, OptionalExtension};

use super::{AssetId, Library, LibraryError, TagId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
}

impl Library {
    pub fn tags(&self) -> Result<Vec<Tag>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name FROM tags ORDER BY normalized_name, id")?;
        let tags = statement
            .query_map([], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(tags)
    }

    pub fn tags_of(&self, asset: AssetId) -> Result<Vec<Tag>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT tags.id, tags.name FROM tags
             JOIN asset_tags ON asset_tags.tag_id = tags.id
             WHERE asset_tags.asset_id = ?1
             ORDER BY tags.normalized_name, tags.id",
        )?;
        let tags = statement
            .query_map([asset], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(tags)
    }

    pub fn tag_named(&self, name: &str) -> Result<Option<Tag>, LibraryError> {
        find_by_name(&self.connection, name)
    }
}

pub fn normalized(name: &str) -> String {
    name.trim().to_lowercase()
}

pub fn find_by_name(connection: &Connection, name: &str) -> Result<Option<Tag>, LibraryError> {
    Ok(connection
        .query_row(
            "SELECT id, name FROM tags WHERE normalized_name = ?1",
            [normalized(name)],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            },
        )
        .optional()?)
}
