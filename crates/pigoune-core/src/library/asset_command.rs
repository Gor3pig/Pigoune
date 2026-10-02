use rusqlite::{Connection, params};

use super::{AssetId, Library, LibraryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetCommand {
    SetFavorite {
        assets: Vec<AssetId>,
        favorite: bool,
    },
    Batch(Vec<AssetCommand>),
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("the resource {0} does not exist")]
    NotFound(AssetId),
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
