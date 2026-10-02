use rusqlite::{Connection, params};

use super::{AssetId, Library, LibraryError};

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
    let mut were_favorite = Vec::new();
    let mut were_not = Vec::new();
    for asset in assets {
        if is_favorite(connection, *asset)? {
            were_favorite.push(*asset);
        } else {
            were_not.push(*asset);
        }
        connection.execute(
            "UPDATE assets SET is_favorite = ?2 WHERE id = ?1",
            params![asset, favorite],
        )?;
    }
    Ok(AssetCommand::Batch(vec![
        AssetCommand::SetFavorite {
            assets: were_favorite,
            favorite: true,
        },
        AssetCommand::SetFavorite {
            assets: were_not,
            favorite: false,
        },
    ]))
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
