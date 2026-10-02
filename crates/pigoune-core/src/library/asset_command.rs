use rusqlite::{Connection, OptionalExtension, params};

use super::{AssetId, Change, Library, LibraryError, clock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetCommand {
    SetFavorite {
        assets: Vec<AssetId>,
        favorite: bool,
    },
    Rename {
        asset: AssetId,
        name: String,
    },
    SetText {
        asset: AssetId,
        field: TextField,
        value: String,
    },
    SetTrashed {
        assets: Vec<AssetId>,
        trashed: bool,
    },
    Batch(Vec<AssetCommand>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextField {
    Note,
    SourceUrl,
    License,
    Author,
}

impl TextField {
    fn column(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::SourceUrl => "source_url",
            Self::License => "license",
            Self::Author => "author",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("the resource {0} does not exist")]
    NotFound(AssetId),
    #[error("a resource name cannot be empty")]
    InvalidName,
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<rusqlite::Error> for AssetError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

impl Library {
    pub fn apply_asset_command(
        &mut self,
        command: &AssetCommand,
    ) -> Result<AssetCommand, AssetError> {
        let inverse = self.run_asset_command(command)?;
        self.history.record(
            Change::Asset(command.clone()),
            Change::Asset(inverse.clone()),
        );
        Ok(inverse)
    }

    pub(super) fn run_asset_command(
        &mut self,
        command: &AssetCommand,
    ) -> Result<AssetCommand, AssetError> {
        let transaction = self.connection.transaction()?;
        let inverse = apply(&transaction, command)?;
        transaction.commit()?;
        Ok(inverse)
    }
}

fn apply(connection: &Connection, command: &AssetCommand) -> Result<AssetCommand, AssetError> {
    match command {
        AssetCommand::SetFavorite { assets, favorite } => {
            set_favorite(connection, assets, *favorite)
        }
        AssetCommand::Rename { asset, name } => rename(connection, *asset, name),
        AssetCommand::SetText {
            asset,
            field,
            value,
        } => set_text(connection, *asset, *field, value),
        AssetCommand::SetTrashed { assets, trashed } => set_trashed(connection, assets, *trashed),
        AssetCommand::Batch(commands) => {
            let mut inverses = commands
                .iter()
                .map(|command| apply(connection, command))
                .collect::<Result<Vec<_>, _>>()?;
            inverses.reverse();
            Ok(AssetCommand::Batch(inverses))
        }
    }
}

fn set_favorite(
    connection: &Connection,
    assets: &[AssetId],
    favorite: bool,
) -> Result<AssetCommand, AssetError> {
    let mut changed = Vec::new();
    for asset in assets {
        if is_favorite(connection, *asset)? != favorite {
            changed.push(*asset);
        }
        connection.execute(
            "UPDATE assets SET is_favorite = ?2 WHERE id = ?1",
            params![asset, favorite],
        )?;
    }
    Ok(AssetCommand::SetFavorite {
        assets: changed,
        favorite: !favorite,
    })
}

fn rename(connection: &Connection, asset: AssetId, name: &str) -> Result<AssetCommand, AssetError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AssetError::InvalidName);
    }
    let old_name: String = visible_value(connection, asset, "display_name")?;
    connection.execute(
        "UPDATE assets SET display_name = ?2 WHERE id = ?1",
        params![asset, name],
    )?;
    Ok(AssetCommand::Rename {
        asset,
        name: old_name,
    })
}

fn set_text(
    connection: &Connection,
    asset: AssetId,
    field: TextField,
    value: &str,
) -> Result<AssetCommand, AssetError> {
    let column = field.column();
    let old_value: String = visible_value(connection, asset, column)?;
    connection.execute(
        &format!("UPDATE assets SET {column} = ?2 WHERE id = ?1"),
        params![asset, value.trim()],
    )?;
    Ok(AssetCommand::SetText {
        asset,
        field,
        value: old_value,
    })
}

fn set_trashed(
    connection: &Connection,
    assets: &[AssetId],
    trashed: bool,
) -> Result<AssetCommand, AssetError> {
    let moment = trashed.then(clock::now_unix_ms);
    let mut changed = Vec::new();
    for asset in assets {
        if !exists(connection, *asset)? {
            return Err(AssetError::NotFound(*asset));
        }
        let updated = connection.execute(
            "UPDATE assets SET trashed_at_unix_ms = ?2
             WHERE id = ?1 AND (trashed_at_unix_ms IS NULL) = ?3",
            params![asset, moment, trashed],
        )?;
        if updated > 0 {
            changed.push(*asset);
        }
    }
    Ok(AssetCommand::SetTrashed {
        assets: changed,
        trashed: !trashed,
    })
}

fn exists(connection: &Connection, asset: AssetId) -> Result<bool, AssetError> {
    Ok(connection
        .query_row("SELECT 1 FROM assets WHERE id = ?1", [asset], |_| Ok(()))
        .optional()?
        .is_some())
}

fn visible_value(
    connection: &Connection,
    asset: AssetId,
    column: &str,
) -> Result<String, AssetError> {
    connection
        .query_row(
            &format!("SELECT {column} FROM assets WHERE id = ?1 AND trashed_at_unix_ms IS NULL"),
            [asset],
            |row| row.get(0),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AssetError::NotFound(asset),
            other => other.into(),
        })
}

fn is_favorite(connection: &Connection, asset: AssetId) -> Result<bool, AssetError> {
    connection
        .query_row(
            "SELECT is_favorite FROM assets WHERE id = ?1 AND trashed_at_unix_ms IS NULL",
            [asset],
            |row| row.get(0),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AssetError::NotFound(asset),
            other => other.into(),
        })
}
