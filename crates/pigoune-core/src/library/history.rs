use std::collections::VecDeque;

use super::{
    AssetCommand, AssetError, CollectionCommand, CollectionError, Library, TagCommand, TagError,
};

pub const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Asset(AssetCommand),
    Collection(CollectionCommand),
    Tag(TagCommand),
}

#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Collection(#[from] CollectionError),
    #[error(transparent)]
    Tag(#[from] TagError),
}

#[derive(Debug, Default)]
pub struct History {
    entries: VecDeque<Entry>,
}

#[derive(Debug)]
struct Entry {
    done: Change,
    inverse: Change,
}

impl History {
    pub fn record(&mut self, done: Change, inverse: Change) {
        if inverse.changes_nothing() || inverse == done {
            return;
        }
        if self.entries.len() == HISTORY_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { done, inverse });
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Library {
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.history.entries.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<Change>, UndoError> {
        let Some(entry) = self.history.entries.pop_back() else {
            return Ok(None);
        };
        match entry.inverse {
            Change::Asset(command) => {
                self.run_asset_command(&command)?;
            }
            Change::Collection(command) => {
                self.run_collection_command(&command)?;
            }
            Change::Tag(command) => {
                self.run_tag_command(&command)?;
            }
        }
        Ok(Some(entry.done))
    }
}

impl Change {
    fn changes_nothing(&self) -> bool {
        match self {
            Self::Asset(command) => command.changes_nothing(),
            Self::Collection(command) => command.changes_nothing(),
            Self::Tag(command) => command.changes_nothing(),
        }
    }
}

impl AssetCommand {
    fn changes_nothing(&self) -> bool {
        match self {
            Self::SetFavorite { assets, .. } | Self::SetTrashed { assets, .. } => assets.is_empty(),
            Self::Rename { .. } | Self::SetText { .. } => false,
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}

impl CollectionCommand {
    fn changes_nothing(&self) -> bool {
        match self {
            Self::AddAssets { assets, .. }
            | Self::RemoveAssets { assets, .. }
            | Self::MoveAssets { assets, .. } => assets.is_empty(),
            Self::SetTrashed {
                collections,
                assets,
                ..
            } => collections.is_empty() && assets.is_empty(),
            Self::Rename { .. } | Self::Move { .. } | Self::Arrange { .. } | Self::Trash { .. } => {
                false
            }
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}

impl TagCommand {
    fn changes_nothing(&self) -> bool {
        match self {
            Self::Add { assets, .. } | Self::Link { assets, .. } | Self::Unlink { assets, .. } => {
                assets.is_empty()
            }
            Self::Rename { .. }
            | Self::Merge { .. }
            | Self::Delete { .. }
            | Self::Recreate { .. } => false,
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}
