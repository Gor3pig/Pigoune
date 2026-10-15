use std::collections::VecDeque;

use super::{
    AssetCommand, AssetError, AssetId, CollectionCommand, CollectionError, DuplicateCommand,
    DuplicateError, Library, SmartCollectionCommand, SmartCollectionError, TagCommand, TagError,
};

pub const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Asset(AssetCommand),
    Collection(CollectionCommand),
    Tag(TagCommand),
    SmartCollection(SmartCollectionCommand),
    Duplicate(DuplicateCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeStamp(u64);

#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Collection(#[from] CollectionError),
    #[error(transparent)]
    Tag(#[from] TagError),
    #[error(transparent)]
    SmartCollection(#[from] SmartCollectionError),
    #[error(transparent)]
    Duplicate(#[from] DuplicateError),
}

#[derive(Debug, Default)]
pub struct History {
    entries: VecDeque<Entry>,
    redoable: Vec<Entry>,
    recorded: u64,
}

#[derive(Debug)]
struct Entry {
    stamp: ChangeStamp,
    done: Change,
    inverse: Change,
}

impl History {
    pub fn record(&mut self, done: Change, inverse: Change) {
        if inverse.changes_nothing() || inverse == done {
            return;
        }
        let done = done.narrowed_to(&inverse);
        self.redoable.clear();
        if self.entries.len() == HISTORY_LIMIT {
            self.entries.pop_front();
        }
        self.recorded += 1;
        self.entries.push_back(Entry {
            stamp: ChangeStamp(self.recorded),
            done,
            inverse,
        });
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.redoable.clear();
    }

    pub fn forget_trashing_of(&mut self, restored: AssetId) {
        self.redoable.clear();
        self.entries.retain_mut(|entry| {
            let (
                Change::Asset(AssetCommand::SetTrashed {
                    assets: trashed,
                    trashed: true,
                }),
                Change::Asset(AssetCommand::SetTrashed {
                    assets: to_restore,
                    trashed: false,
                }),
            ) = (&mut entry.done, &mut entry.inverse)
            else {
                return true;
            };
            trashed.retain(|asset| *asset != restored);
            to_restore.retain(|asset| *asset != restored);
            !to_restore.is_empty()
        });
    }
}

impl Library {
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.history.entries.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.history.redoable.is_empty()
    }

    #[must_use]
    pub fn latest_change(&self) -> Option<ChangeStamp> {
        self.history.entries.back().map(|entry| entry.stamp)
    }

    #[must_use]
    pub fn latest_redoable(&self) -> Option<ChangeStamp> {
        self.history.redoable.last().map(|entry| entry.stamp)
    }

    pub fn redo_change(&mut self, stamp: ChangeStamp) -> Result<Option<Change>, UndoError> {
        if self.latest_redoable() == Some(stamp) {
            self.redo()
        } else {
            Ok(None)
        }
    }

    pub fn undo_change(&mut self, stamp: ChangeStamp) -> Result<Option<Change>, UndoError> {
        if self.latest_change() == Some(stamp) {
            self.undo()
        } else {
            Ok(None)
        }
    }

    pub fn undo(&mut self) -> Result<Option<Change>, UndoError> {
        let Some(entry) = self.history.entries.pop_back() else {
            return Ok(None);
        };
        let redo = self.run_change(&entry.inverse)?;
        self.history.redoable.push(Entry {
            stamp: entry.stamp,
            done: entry.done.clone(),
            inverse: redo,
        });
        Ok(Some(entry.done))
    }

    pub fn redo(&mut self) -> Result<Option<Change>, UndoError> {
        let Some(entry) = self.history.redoable.pop() else {
            return Ok(None);
        };
        let inverse = self.run_change(&entry.inverse)?;
        if self.history.entries.len() == HISTORY_LIMIT {
            self.history.entries.pop_front();
        }
        self.history.entries.push_back(Entry {
            stamp: entry.stamp,
            done: entry.done.clone(),
            inverse,
        });
        Ok(Some(entry.done))
    }

    fn run_change(&mut self, change: &Change) -> Result<Change, UndoError> {
        Ok(match change {
            Change::Asset(command) => Change::Asset(self.run_asset_command(command)?),
            Change::Collection(command) => {
                Change::Collection(self.run_collection_command(command)?)
            }
            Change::Tag(command) => Change::Tag(self.run_tag_command(command)?),
            Change::SmartCollection(command) => {
                Change::SmartCollection(self.run_smart_collection_command(command)?)
            }
            Change::Duplicate(command) => Change::Duplicate(self.run_duplicate_command(command)?),
        })
    }
}

impl Change {
    fn narrowed_to(self, inverse: &Change) -> Change {
        match (self, inverse) {
            (
                Self::Asset(AssetCommand::SetFavorite { favorite, .. }),
                Self::Asset(AssetCommand::SetFavorite { assets, .. }),
            ) => Self::Asset(AssetCommand::SetFavorite {
                assets: assets.clone(),
                favorite,
            }),
            (
                Self::Asset(AssetCommand::SetTrashed { trashed, .. }),
                Self::Asset(AssetCommand::SetTrashed { assets, .. }),
            ) => Self::Asset(AssetCommand::SetTrashed {
                assets: assets.clone(),
                trashed,
            }),
            (
                Self::Collection(CollectionCommand::AddAssets { collection, .. }),
                Self::Collection(CollectionCommand::RemoveAssets { assets, .. }),
            ) => Self::Collection(CollectionCommand::AddAssets {
                collection,
                assets: assets.clone(),
            }),
            (
                Self::Collection(CollectionCommand::RemoveAssets { collection, .. }),
                Self::Collection(CollectionCommand::AddAssets { assets, .. }),
            ) => Self::Collection(CollectionCommand::RemoveAssets {
                collection,
                assets: assets.clone(),
            }),
            (
                Self::Tag(TagCommand::Link { tag, .. }),
                Self::Tag(TagCommand::Unlink { assets, .. }),
            ) => Self::Tag(TagCommand::Link {
                tag,
                assets: assets.clone(),
            }),
            (
                Self::Tag(TagCommand::Unlink { tag, .. }),
                Self::Tag(TagCommand::Link { assets, .. }),
            ) => Self::Tag(TagCommand::Unlink {
                tag,
                assets: assets.clone(),
            }),
            (
                Self::Duplicate(DuplicateCommand::Ignore { .. }),
                Self::Duplicate(DuplicateCommand::Unignore { pairs }),
            ) => Self::Duplicate(DuplicateCommand::Ignore {
                pairs: pairs.clone(),
            }),
            (
                Self::Duplicate(DuplicateCommand::Unignore { .. }),
                Self::Duplicate(DuplicateCommand::Ignore { pairs }),
            ) => Self::Duplicate(DuplicateCommand::Unignore {
                pairs: pairs.clone(),
            }),
            (done, _) => done,
        }
    }

    fn changes_nothing(&self) -> bool {
        match self {
            Self::Asset(command) => command.changes_nothing(),
            Self::Collection(command) => command.changes_nothing(),
            Self::Tag(command) => command.changes_nothing(),
            Self::SmartCollection(_) => false,
            Self::Duplicate(command) => command.changes_nothing(),
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
            Self::Create { .. }
            | Self::Delete { .. }
            | Self::Rename { .. }
            | Self::Restyle { .. }
            | Self::Move { .. }
            | Self::Arrange { .. }
            | Self::Trash { .. } => false,
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}

impl TagCommand {
    fn changes_nothing(&self) -> bool {
        match self {
            Self::Link { assets, .. } | Self::Unlink { assets, .. } => assets.is_empty(),
            Self::Add { .. }
            | Self::Rename { .. }
            | Self::Move { .. }
            | Self::Merge { .. }
            | Self::Dissolve { .. }
            | Self::Delete { .. }
            | Self::Recreate { .. } => false,
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}
