use rusqlite::{Connection, OptionalExtension, params};

use super::tag::{self, find_by_name};
use super::{AssetId, Library, LibraryError, TagId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagCommand {
    Add { assets: Vec<AssetId>, name: String },
    Link { tag: TagId, assets: Vec<AssetId> },
    Unlink { tag: TagId, assets: Vec<AssetId> },
    Rename { tag: TagId, name: String },
    Merge { from: TagId, into: TagId },
    Delete { tag: TagId },
    Recreate { tag: TagId, name: String },
    Batch(Vec<TagCommand>),
}

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("a tag name cannot be empty")]
    InvalidName,
    #[error("the tag {0} does not exist")]
    NotFound(TagId),
    #[error("the resource {0} does not exist")]
    AssetNotFound(AssetId),
    #[error("another tag already has this name")]
    NameTaken(TagId),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<rusqlite::Error> for TagError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

impl Library {
    pub fn apply_tag_command(&mut self, command: &TagCommand) -> Result<TagCommand, TagError> {
        let transaction = self.connection.transaction()?;
        let inverse = apply(&transaction, command)?;
        transaction.commit()?;
        Ok(inverse)
    }
}

fn apply(connection: &Connection, command: &TagCommand) -> Result<TagCommand, TagError> {
    match command {
        TagCommand::Add { assets, name } => add(connection, assets, name),
        TagCommand::Link { tag, assets } => link(connection, *tag, assets),
        TagCommand::Unlink { tag, assets } => unlink(connection, *tag, assets),
        TagCommand::Rename { tag, name } => rename(connection, *tag, name),
        TagCommand::Merge { from, into } => merge(connection, *from, *into),
        TagCommand::Delete { tag } => delete(connection, *tag),
        TagCommand::Recreate { tag, name } => recreate(connection, *tag, name),
        TagCommand::Batch(commands) => {
            let mut inverses = commands
                .iter()
                .map(|command| apply(connection, command))
                .collect::<Result<Vec<_>, _>>()?;
            inverses.reverse();
            Ok(TagCommand::Batch(inverses))
        }
    }
}

fn add(connection: &Connection, assets: &[AssetId], name: &str) -> Result<TagCommand, TagError> {
    let name = valid_name(name)?;
    for asset in assets {
        ensure_asset_is_visible(connection, *asset)?;
    }
    let (tag, created) = match find_by_name(connection, name)? {
        Some(existing) => (existing.id, false),
        None => (insert(connection, TagId::generate(), name)?, true),
    };
    let unlink = link(connection, tag, assets)?;
    if created {
        Ok(TagCommand::Batch(vec![unlink, TagCommand::Delete { tag }]))
    } else {
        Ok(unlink)
    }
}

fn link(connection: &Connection, tag: TagId, assets: &[AssetId]) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, tag)?;
    let mut linked = Vec::new();
    for asset in assets {
        ensure_asset_exists(connection, *asset)?;
        let changed = connection.execute(
            "INSERT OR IGNORE INTO asset_tags (asset_id, tag_id) VALUES (?1, ?2)",
            params![asset, tag],
        )?;
        if changed > 0 {
            linked.push(*asset);
        }
    }
    Ok(TagCommand::Unlink {
        tag,
        assets: linked,
    })
}

fn unlink(connection: &Connection, tag: TagId, assets: &[AssetId]) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, tag)?;
    let mut unlinked = Vec::new();
    for asset in assets {
        let changed = connection.execute(
            "DELETE FROM asset_tags WHERE asset_id = ?1 AND tag_id = ?2",
            params![asset, tag],
        )?;
        if changed > 0 {
            unlinked.push(*asset);
        }
    }
    Ok(TagCommand::Link {
        tag,
        assets: unlinked,
    })
}

fn rename(connection: &Connection, tag: TagId, name: &str) -> Result<TagCommand, TagError> {
    let old_name = name_of(connection, tag)?;
    let name = valid_name(name)?;
    if let Some(existing) = find_by_name(connection, name)?
        && existing.id != tag
    {
        return Err(TagError::NameTaken(existing.id));
    }
    connection.execute(
        "UPDATE tags SET name = ?2, normalized_name = ?3 WHERE id = ?1",
        params![tag, name, tag::normalized(name)],
    )?;
    Ok(TagCommand::Rename {
        tag,
        name: old_name,
    })
}

fn merge(connection: &Connection, from: TagId, into: TagId) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, into)?;
    let carried = tagged_assets(connection, from)?;
    let restore_from = delete(connection, from)?;
    let unlink_into = link(connection, into, &carried)?;
    Ok(TagCommand::Batch(vec![unlink_into, restore_from]))
}

fn delete(connection: &Connection, tag: TagId) -> Result<TagCommand, TagError> {
    let name = name_of(connection, tag)?;
    let assets = tagged_assets(connection, tag)?;
    connection.execute("DELETE FROM tags WHERE id = ?1", [tag])?;
    Ok(TagCommand::Batch(vec![
        TagCommand::Recreate { tag, name },
        TagCommand::Link { tag, assets },
    ]))
}

fn recreate(connection: &Connection, tag: TagId, name: &str) -> Result<TagCommand, TagError> {
    let name = valid_name(name)?;
    if let Some(existing) = find_by_name(connection, name)? {
        return Err(TagError::NameTaken(existing.id));
    }
    insert(connection, tag, name)?;
    Ok(TagCommand::Delete { tag })
}

fn insert(connection: &Connection, tag: TagId, name: &str) -> Result<TagId, TagError> {
    connection.execute(
        "INSERT INTO tags (id, name, normalized_name) VALUES (?1, ?2, ?3)",
        params![tag, name, tag::normalized(name)],
    )?;
    Ok(tag)
}

fn valid_name(name: &str) -> Result<&str, TagError> {
    let name = name.trim();
    if name.is_empty() {
        Err(TagError::InvalidName)
    } else {
        Ok(name)
    }
}

fn name_of(connection: &Connection, tag: TagId) -> Result<String, TagError> {
    connection
        .query_row("SELECT name FROM tags WHERE id = ?1", [tag], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or(TagError::NotFound(tag))
}

fn ensure_tag_exists(connection: &Connection, tag: TagId) -> Result<(), TagError> {
    name_of(connection, tag).map(|_| ())
}

fn ensure_asset_exists(connection: &Connection, asset: AssetId) -> Result<(), TagError> {
    connection
        .query_row("SELECT 1 FROM assets WHERE id = ?1", [asset], |_| Ok(()))
        .optional()?
        .ok_or(TagError::AssetNotFound(asset))
}

fn ensure_asset_is_visible(connection: &Connection, asset: AssetId) -> Result<(), TagError> {
    connection
        .query_row(
            "SELECT 1 FROM assets WHERE id = ?1 AND trashed_at_unix_ms IS NULL",
            [asset],
            |_| Ok(()),
        )
        .optional()?
        .ok_or(TagError::AssetNotFound(asset))
}

fn tagged_assets(connection: &Connection, tag: TagId) -> Result<Vec<AssetId>, TagError> {
    let mut statement = connection
        .prepare("SELECT asset_id FROM asset_tags WHERE tag_id = ?1 ORDER BY asset_id")?;
    let assets = statement
        .query_map([tag], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(assets)
}
