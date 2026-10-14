use std::collections::HashSet;

use super::duplicates::{Stored, duplicate_kind};
use super::{AssetCommand, AssetError, AssetId, ChangeStamp, Library, LibraryError};

pub const MOST_CHOSEN_AT_ONCE: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum RemovalError {
    #[error("no asset was chosen to be moved to the trash")]
    NothingChosen,
    #[error("the asset to keep is among the assets chosen for the trash")]
    KeptAmongChosen,
    #[error("the asset {0} is chosen twice")]
    ChosenTwice(AssetId),
    #[error("{chosen} assets are chosen at once, the limit is {most}")]
    TooManyChosen { chosen: usize, most: usize },
    #[error("the asset {0} no longer exists or is already in the trash")]
    NotAvailable(AssetId),
    #[error("the asset {0} no longer looks like the asset to keep")]
    NotAlike(AssetId),
    #[error("the trash changed more assets than were chosen")]
    BeyondTheChoice,
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Library(#[from] LibraryError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedRemoval {
    kept: AssetId,
    chosen: Vec<AssetId>,
}

impl ConfirmedRemoval {
    pub fn new(kept: AssetId, chosen: Vec<AssetId>) -> Result<Self, RemovalError> {
        if chosen.is_empty() {
            return Err(RemovalError::NothingChosen);
        }
        if chosen.len() > MOST_CHOSEN_AT_ONCE {
            return Err(RemovalError::TooManyChosen {
                chosen: chosen.len(),
                most: MOST_CHOSEN_AT_ONCE,
            });
        }
        let mut seen = HashSet::new();
        for id in &chosen {
            if *id == kept {
                return Err(RemovalError::KeptAmongChosen);
            }
            if !seen.insert(*id) {
                return Err(RemovalError::ChosenTwice(*id));
            }
        }
        Ok(Self { kept, chosen })
    }

    #[must_use]
    pub fn kept(&self) -> AssetId {
        self.kept
    }

    #[must_use]
    pub fn chosen(&self) -> &[AssetId] {
        &self.chosen
    }
}

impl Library {
    pub fn trash_confirmed_duplicates(
        &mut self,
        removal: &ConfirmedRemoval,
    ) -> Result<ChangeStamp, RemovalError> {
        let kept = match self.stored_fingerprint(removal.kept)? {
            Stored::Ready(fingerprint) if self.is_available(removal.kept)? => fingerprint,
            _ => return Err(RemovalError::NotAvailable(removal.kept)),
        };
        for id in &removal.chosen {
            if !self.is_available(*id)? {
                return Err(RemovalError::NotAvailable(*id));
            }
            match self.stored_fingerprint(*id)? {
                Stored::Ready(other) if duplicate_kind(&kept, &other).is_some() => {}
                _ => return Err(RemovalError::NotAlike(*id)),
            }
        }
        let inverse = self.apply_asset_command(&AssetCommand::SetTrashed {
            assets: removal.chosen.clone(),
            trashed: true,
        })?;
        let stamp = self.latest_change();
        let changed: HashSet<AssetId> = match &inverse {
            AssetCommand::SetTrashed { assets, .. } => assets.iter().copied().collect(),
            _ => HashSet::new(),
        };
        let confirmed: HashSet<AssetId> = removal.chosen.iter().copied().collect();
        match stamp {
            Some(stamp) if changed == confirmed => Ok(stamp),
            Some(stamp) => {
                let _ = self.undo_change(stamp);
                Err(RemovalError::BeyondTheChoice)
            }
            None => Err(RemovalError::BeyondTheChoice),
        }
    }

    fn is_available(&self, id: AssetId) -> Result<bool, LibraryError> {
        Ok(self
            .asset(id)?
            .is_some_and(|asset| asset.trashed_at_unix_ms.is_none()))
    }
}
