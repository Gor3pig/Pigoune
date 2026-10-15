use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params};

use super::duplicates::MOST_IN_A_GROUP;
use super::{AssetId, Change, ChangeStamp, Library, LibraryError, clock};

pub type AssetPair = (AssetId, AssetId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateCommand {
    Ignore { pairs: Vec<AssetPair> },
    Unignore { pairs: Vec<AssetPair> },
}

impl DuplicateCommand {
    pub(super) fn changes_nothing(&self) -> bool {
        match self {
            Self::Ignore { pairs } | Self::Unignore { pairs } => pairs.is_empty(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DuplicateError {
    #[error("the asset {0} does not exist")]
    NotFound(AssetId),
    #[error("a group of {0} assets is larger than any group of duplicates")]
    TooLarge(usize),
    #[error("nothing changed: these assets were already in that state")]
    NothingChanged,
    #[error(transparent)]
    Library(#[from] LibraryError),
}

impl From<rusqlite::Error> for DuplicateError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Library(error.into())
    }
}

#[must_use]
pub fn ordered_pair(first: AssetId, second: AssetId) -> AssetPair {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn pairs_among(group: &[AssetId]) -> Vec<AssetPair> {
    let mut pairs = Vec::new();
    for (index, first) in group.iter().enumerate() {
        for second in &group[index + 1..] {
            if first != second {
                pairs.push(ordered_pair(*first, *second));
            }
        }
    }
    pairs
}

impl Library {
    pub fn ignore_duplicate_group(
        &mut self,
        group: &[AssetId],
    ) -> Result<ChangeStamp, DuplicateError> {
        if group.len() > MOST_IN_A_GROUP {
            return Err(DuplicateError::TooLarge(group.len()));
        }
        self.change_duplicates(&DuplicateCommand::Ignore {
            pairs: pairs_among(group),
        })
    }

    pub fn ignore_duplicate_member(
        &mut self,
        member: AssetId,
        others: &[AssetId],
    ) -> Result<ChangeStamp, DuplicateError> {
        if others.len() >= MOST_IN_A_GROUP {
            return Err(DuplicateError::TooLarge(others.len() + 1));
        }
        let pairs = others
            .iter()
            .filter(|other| **other != member)
            .map(|other| ordered_pair(member, *other))
            .collect();
        self.change_duplicates(&DuplicateCommand::Ignore { pairs })
    }

    pub fn unignore_duplicate_group(
        &mut self,
        group: &[AssetId],
    ) -> Result<ChangeStamp, DuplicateError> {
        self.change_duplicates(&DuplicateCommand::Unignore {
            pairs: pairs_among(group),
        })
    }

    pub fn unignore_all_duplicates(&mut self) -> Result<ChangeStamp, DuplicateError> {
        let pairs = self.ignored_pairs()?.into_iter().collect();
        self.change_duplicates(&DuplicateCommand::Unignore { pairs })
    }

    pub fn apply_duplicate_command(
        &mut self,
        command: &DuplicateCommand,
    ) -> Result<DuplicateCommand, DuplicateError> {
        let inverse = self.run_duplicate_command(command)?;
        self.history.record(
            Change::Duplicate(command.clone()),
            Change::Duplicate(inverse.clone()),
        );
        Ok(inverse)
    }

    pub(super) fn run_duplicate_command(
        &mut self,
        command: &DuplicateCommand,
    ) -> Result<DuplicateCommand, DuplicateError> {
        let transaction = self.connection.transaction()?;
        let inverse = apply(&transaction, command)?;
        transaction.commit()?;
        Ok(inverse)
    }

    fn change_duplicates(
        &mut self,
        command: &DuplicateCommand,
    ) -> Result<ChangeStamp, DuplicateError> {
        let before = self.latest_change();
        self.apply_duplicate_command(command)?;
        match self.latest_change() {
            Some(stamp) if Some(stamp) != before => Ok(stamp),
            _ => Err(DuplicateError::NothingChanged),
        }
    }

    pub fn ignored_duplicate_groups(&self) -> Result<Vec<Vec<AssetId>>, LibraryError> {
        let mut neighbours: HashMap<AssetId, Vec<AssetId>> = HashMap::new();
        for (first, second) in self.visible_ignored_pairs()? {
            neighbours.entry(first).or_default().push(second);
            neighbours.entry(second).or_default().push(first);
        }
        let mut seen: HashSet<AssetId> = HashSet::new();
        let mut groups: Vec<Vec<AssetId>> = Vec::new();
        let starts: BTreeSet<AssetId> = neighbours.keys().copied().collect();
        for start in &starts {
            if !seen.insert(*start) {
                continue;
            }
            let mut group = vec![*start];
            let mut index = 0;
            while index < group.len() {
                for next in &neighbours[&group[index]] {
                    if seen.insert(*next) {
                        group.push(*next);
                    }
                }
                index += 1;
            }
            group.sort();
            groups.push(group);
        }
        groups.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        Ok(groups)
    }

    pub fn ignored_duplicate_count(&self) -> Result<usize, LibraryError> {
        Ok(self.ignored_duplicate_groups()?.len())
    }

    pub(super) fn ignored_pairs(&self) -> Result<HashSet<AssetPair>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT first_asset, second_asset FROM ignored_duplicates")?;
        let pairs = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(pairs)
    }

    fn visible_ignored_pairs(&self) -> Result<Vec<AssetPair>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT i.first_asset, i.second_asset
             FROM ignored_duplicates i
             JOIN assets a ON a.id = i.first_asset
             JOIN assets b ON b.id = i.second_asset
             WHERE a.trashed_at_unix_ms IS NULL AND b.trashed_at_unix_ms IS NULL",
        )?;
        let pairs = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(pairs)
    }

    pub fn is_ignored_duplicate(
        &self,
        first: AssetId,
        second: AssetId,
    ) -> Result<bool, LibraryError> {
        let (first, second) = ordered_pair(first, second);
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM ignored_duplicates WHERE first_asset = ?1 AND second_asset = ?2",
                params![first, second],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }
}

fn apply(
    connection: &Connection,
    command: &DuplicateCommand,
) -> Result<DuplicateCommand, DuplicateError> {
    match command {
        DuplicateCommand::Ignore { pairs } => {
            let moment = clock::now_unix_ms();
            let mut changed = Vec::new();
            for (first, second) in pairs {
                for asset in [first, second] {
                    if !exists(connection, *asset)? {
                        return Err(DuplicateError::NotFound(*asset));
                    }
                }
                let added = connection.execute(
                    "INSERT OR IGNORE INTO ignored_duplicates
                     (first_asset, second_asset, ignored_at_unix_ms) VALUES (?1, ?2, ?3)",
                    params![first, second, moment],
                )?;
                if added > 0 {
                    changed.push((*first, *second));
                }
            }
            Ok(DuplicateCommand::Unignore { pairs: changed })
        }
        DuplicateCommand::Unignore { pairs } => {
            let mut changed = Vec::new();
            for (first, second) in pairs {
                let removed = connection.execute(
                    "DELETE FROM ignored_duplicates WHERE first_asset = ?1 AND second_asset = ?2",
                    params![first, second],
                )?;
                if removed > 0 {
                    changed.push((*first, *second));
                }
            }
            Ok(DuplicateCommand::Ignore { pairs: changed })
        }
    }
}

fn exists(connection: &Connection, asset: AssetId) -> Result<bool, DuplicateError> {
    Ok(connection
        .query_row("SELECT 1 FROM assets WHERE id = ?1", [asset], |_| Ok(()))
        .optional()?
        .is_some())
}
