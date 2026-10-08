use rusqlite::{Connection, OptionalExtension, params};

use super::tag::{self, children, find_child, subtree};
use super::{AssetId, Change, Library, LibraryError, TagId};

pub const LONGEST_TAG_NAME: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagCommand {
    Add {
        assets: Vec<AssetId>,
        name: String,
    },
    Link {
        tag: TagId,
        assets: Vec<AssetId>,
    },
    Unlink {
        tag: TagId,
        assets: Vec<AssetId>,
    },
    Rename {
        tag: TagId,
        name: String,
    },
    Move {
        tag: TagId,
        parent: Option<TagId>,
    },
    Merge {
        from: TagId,
        into: TagId,
    },
    Dissolve {
        tag: TagId,
    },
    Delete {
        tag: TagId,
    },
    Recreate {
        tag: TagId,
        name: String,
        parent: Option<TagId>,
    },
    Batch(Vec<TagCommand>),
}

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("a tag name cannot be empty")]
    InvalidName,
    #[error("a tag is a single word")]
    NotOneWord,
    #[error("a tag has at most {LONGEST_TAG_NAME} characters")]
    TooLong,
    #[error("the tag {0} does not exist")]
    NotFound(TagId),
    #[error("the resource {0} does not exist")]
    AssetNotFound(AssetId),
    #[error("another tag already has this name")]
    NameTaken(TagId),
    #[error("a tag cannot be moved into itself or into one of its own sub-tags")]
    Cycle,
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
        let inverse = self.run_tag_command(command)?;
        self.history
            .record(Change::Tag(command.clone()), Change::Tag(inverse.clone()));
        Ok(inverse)
    }

    pub(super) fn run_tag_command(&mut self, command: &TagCommand) -> Result<TagCommand, TagError> {
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
        TagCommand::Move { tag, parent } => move_tag(connection, *tag, *parent),
        TagCommand::Merge { from, into } => merge(connection, *from, *into),
        TagCommand::Dissolve { tag } => dissolve(connection, *tag),
        TagCommand::Delete { tag } => delete(connection, *tag),
        TagCommand::Recreate { tag, name, parent } => recreate(connection, *tag, name, *parent),
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

fn add(connection: &Connection, assets: &[AssetId], path: &str) -> Result<TagCommand, TagError> {
    let names = path_names(path)?;
    for asset in assets {
        ensure_asset_is_visible(connection, *asset)?;
    }
    let mut parent = None;
    let mut created = Vec::new();
    for name in names {
        parent = Some(
            if let Some(existing) = find_child(connection, parent, name)? {
                existing.id
            } else {
                ensure_is_a_short_word(name)?;
                let tag = insert(connection, TagId::generate(), name, parent)?;
                created.push(tag);
                tag
            },
        );
    }
    let tag = parent.ok_or(TagError::InvalidName)?;
    let unlink = link(connection, tag, assets)?;
    match created.first() {
        Some(top) => Ok(TagCommand::Batch(vec![
            unlink,
            TagCommand::Delete { tag: *top },
        ])),
        None => Ok(unlink),
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
    ensure_is_a_short_word(name)?;
    ensure_name_is_free(connection, parent_of(connection, tag)?, name, tag)?;
    connection.execute(
        "UPDATE tags SET name = ?2, normalized_name = ?3 WHERE id = ?1",
        params![tag, name, tag::normalized(name)],
    )?;
    Ok(TagCommand::Rename {
        tag,
        name: old_name,
    })
}

fn move_tag(
    connection: &Connection,
    tag: TagId,
    parent: Option<TagId>,
) -> Result<TagCommand, TagError> {
    let name = name_of(connection, tag)?;
    let old_parent = parent_of(connection, tag)?;
    if let Some(parent) = parent {
        ensure_tag_exists(connection, parent)?;
        if subtree(connection, tag)?
            .iter()
            .any(|below| below.id == parent)
        {
            return Err(TagError::Cycle);
        }
    }
    ensure_name_is_free(connection, parent, &name, tag)?;
    connection.execute(
        "UPDATE tags SET parent_id = ?2 WHERE id = ?1",
        params![tag, parent],
    )?;
    Ok(TagCommand::Move {
        tag,
        parent: old_parent,
    })
}

fn merge(connection: &Connection, from: TagId, into: TagId) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, from)?;
    ensure_tag_exists(connection, into)?;
    if subtree(connection, from)?
        .iter()
        .any(|below| below.id == into)
    {
        return Err(TagError::Cycle);
    }
    let carried = tagged_assets(connection, from)?;
    let mut steps: Vec<TagCommand> = children(connection, from)?
        .into_iter()
        .map(|child| TagCommand::Move {
            tag: child.id,
            parent: Some(into),
        })
        .collect();
    steps.push(TagCommand::Delete { tag: from });
    steps.push(TagCommand::Link {
        tag: into,
        assets: carried,
    });
    apply(connection, &TagCommand::Batch(steps))
}

fn dissolve(connection: &Connection, tag: TagId) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, tag)?;
    let parent = parent_of(connection, tag)?;
    let mut steps: Vec<TagCommand> = children(connection, tag)?
        .into_iter()
        .map(|child| TagCommand::Move {
            tag: child.id,
            parent,
        })
        .collect();
    steps.push(TagCommand::Delete { tag });
    apply(connection, &TagCommand::Batch(steps))
}

fn delete(connection: &Connection, tag: TagId) -> Result<TagCommand, TagError> {
    ensure_tag_exists(connection, tag)?;
    let removed = subtree(connection, tag)?;
    let mut recreate = Vec::new();
    let mut relink = Vec::new();
    for below in &removed {
        recreate.push(TagCommand::Recreate {
            tag: below.id,
            name: below.name.clone(),
            parent: below.parent,
        });
        relink.push(TagCommand::Link {
            tag: below.id,
            assets: tagged_assets(connection, below.id)?,
        });
    }
    connection.execute("DELETE FROM tags WHERE id = ?1", [tag])?;
    recreate.extend(relink);
    Ok(TagCommand::Batch(recreate))
}

fn recreate(
    connection: &Connection,
    tag: TagId,
    name: &str,
    parent: Option<TagId>,
) -> Result<TagCommand, TagError> {
    let name = valid_name(name)?;
    if let Some(parent) = parent {
        ensure_tag_exists(connection, parent)?;
    }
    if let Some(existing) = find_child(connection, parent, name)? {
        return Err(TagError::NameTaken(existing.id));
    }
    insert(connection, tag, name, parent)?;
    Ok(TagCommand::Delete { tag })
}

fn insert(
    connection: &Connection,
    tag: TagId,
    name: &str,
    parent: Option<TagId>,
) -> Result<TagId, TagError> {
    connection.execute(
        "INSERT INTO tags (id, name, normalized_name, parent_id) VALUES (?1, ?2, ?3, ?4)",
        params![tag, name, tag::normalized(name), parent],
    )?;
    Ok(tag)
}

fn path_names(path: &str) -> Result<Vec<&str>, TagError> {
    let names: Vec<&str> = path.split('/').map(str::trim).collect();
    if names.iter().any(|name| name.is_empty()) {
        return Err(TagError::InvalidName);
    }
    Ok(names)
}

fn ensure_name_is_free(
    connection: &Connection,
    parent: Option<TagId>,
    name: &str,
    tag: TagId,
) -> Result<(), TagError> {
    match find_child(connection, parent, name)? {
        Some(existing) if existing.id != tag => Err(TagError::NameTaken(existing.id)),
        _ => Ok(()),
    }
}

fn parent_of(connection: &Connection, tag: TagId) -> Result<Option<TagId>, TagError> {
    connection
        .query_row("SELECT parent_id FROM tags WHERE id = ?1", [tag], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or(TagError::NotFound(tag))
}

fn valid_name(name: &str) -> Result<&str, TagError> {
    let name = name.trim();
    if name.is_empty() {
        Err(TagError::InvalidName)
    } else {
        Ok(name)
    }
}

fn ensure_is_a_short_word(name: &str) -> Result<(), TagError> {
    if name.chars().any(char::is_whitespace) {
        Err(TagError::NotOneWord)
    } else if name.chars().count() > LONGEST_TAG_NAME {
        Err(TagError::TooLong)
    } else {
        Ok(())
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
