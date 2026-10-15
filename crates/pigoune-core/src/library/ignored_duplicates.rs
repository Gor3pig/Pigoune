use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, params};

use super::{AssetId, Change, ChangeStamp, IgnoreId, Library, LibraryError, clock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreKind {
    Group,
    Member,
}

impl IgnoreKind {
    fn text(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Member => "member",
        }
    }

    fn from_text(text: &str) -> Self {
        if text == "member" {
            Self::Member
        } else {
            Self::Group
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreAction {
    pub id: IgnoreId,
    pub kind: IgnoreKind,
    pub subject: Option<AssetId>,
    pub members: Vec<AssetId>,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateCommand {
    Ignore(IgnoreAction),
    Unignore(IgnoreId),
    Batch(Vec<DuplicateCommand>),
}

impl DuplicateCommand {
    pub(super) fn changes_nothing(&self) -> bool {
        match self {
            Self::Ignore(action) => action.members.is_empty(),
            Self::Unignore(_) => false,
            Self::Batch(commands) => commands.iter().all(Self::changes_nothing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgnoreOutcome {
    pub stamp: ChangeStamp,
    pub covered: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredEntry {
    pub action: IgnoreId,
    pub kind: IgnoreKind,
    pub assets: Vec<AssetId>,
    pub against: Vec<AssetId>,
}

#[derive(Debug, thiserror::Error)]
pub enum DuplicateError {
    #[error("the asset {0} does not exist")]
    NotFound(AssetId),
    #[error("the asset {0} is in the trash")]
    NotAvailable(AssetId),
    #[error("the asset {0} no longer looks like the others")]
    NotAlike(AssetId),
    #[error("the ignored duplicates {0} no longer exist")]
    UnknownAction(IgnoreId),
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

#[derive(Default)]
pub(super) struct IgnoredSets {
    groups: HashMap<AssetId, Vec<u32>>,
    subjects: HashMap<AssetId, Vec<u32>>,
    targets: HashMap<AssetId, Vec<u32>>,
}

fn share(first: Option<&Vec<u32>>, second: Option<&Vec<u32>>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => first.iter().any(|action| second.contains(action)),
        _ => false,
    }
}

impl IgnoredSets {
    pub(super) fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.subjects.is_empty()
    }

    pub(super) fn contains(&self, first: AssetId, second: AssetId) -> bool {
        share(self.groups.get(&first), self.groups.get(&second))
            || share(self.subjects.get(&first), self.targets.get(&second))
            || share(self.subjects.get(&second), self.targets.get(&first))
    }
}

impl Library {
    pub fn ignore_duplicate_group(
        &mut self,
        group: &[AssetId],
    ) -> Result<IgnoreOutcome, DuplicateError> {
        let cluster = self.cluster_around(group)?;
        self.record_ignore(IgnoreKind::Group, None, cluster)
    }

    pub fn ignore_duplicate_member(
        &mut self,
        member: AssetId,
        others: &[AssetId],
    ) -> Result<IgnoreOutcome, DuplicateError> {
        let mut seeds = vec![member];
        seeds.extend(others.iter().copied().filter(|other| *other != member));
        let cluster = self.cluster_around(&seeds)?;
        let against = cluster
            .into_iter()
            .filter(|asset| *asset != member)
            .collect();
        self.record_ignore(IgnoreKind::Member, Some(member), against)
    }

    pub fn unignore_duplicate_action(
        &mut self,
        action: IgnoreId,
    ) -> Result<ChangeStamp, DuplicateError> {
        self.change_duplicates(&DuplicateCommand::Unignore(action))
    }

    pub fn unignore_all_duplicates(&mut self) -> Result<ChangeStamp, DuplicateError> {
        let actions = self.ignore_action_ids()?;
        let commands = actions
            .into_iter()
            .map(DuplicateCommand::Unignore)
            .collect();
        self.change_duplicates(&DuplicateCommand::Batch(commands))
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

    fn cluster_around(&self, seeds: &[AssetId]) -> Result<Vec<AssetId>, DuplicateError> {
        if seeds.len() < 2 {
            return Err(DuplicateError::NothingChanged);
        }
        for asset in seeds {
            let found = self.asset(*asset)?;
            match found {
                None => return Err(DuplicateError::NotFound(*asset)),
                Some(found) if found.trashed_at_unix_ms.is_some() => {
                    return Err(DuplicateError::NotAvailable(*asset));
                }
                Some(_) => {}
            }
        }
        self.duplicate_plan()?
            .cluster_of(seeds)
            .map_err(DuplicateError::NotAlike)
    }

    fn record_ignore(
        &mut self,
        kind: IgnoreKind,
        subject: Option<AssetId>,
        members: Vec<AssetId>,
    ) -> Result<IgnoreOutcome, DuplicateError> {
        let covered = members.len() + usize::from(subject.is_some());
        let action = IgnoreAction {
            id: IgnoreId::generate(),
            kind,
            subject,
            members,
            created_at_unix_ms: clock::now_unix_ms(),
        };
        let stamp = self.change_duplicates(&DuplicateCommand::Ignore(action))?;
        Ok(IgnoreOutcome { stamp, covered })
    }

    fn ignore_action_ids(&self) -> Result<Vec<IgnoreId>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM ignored_duplicate_actions ORDER BY created_at_unix_ms, id")?;
        let ids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn ignored_duplicate_entries(&self) -> Result<Vec<IgnoredEntry>, LibraryError> {
        let mut members: HashMap<IgnoreId, BTreeSet<AssetId>> = HashMap::new();
        let mut statement = self.connection.prepare(
            "SELECT m.action_id, m.asset_id
             FROM ignored_duplicate_members m
             JOIN assets a ON a.id = m.asset_id
             WHERE a.trashed_at_unix_ms IS NULL",
        )?;
        for row in statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))? {
            let (action, asset): (IgnoreId, AssetId) = row?;
            members.entry(action).or_default().insert(asset);
        }
        let mut statement = self.connection.prepare(
            "SELECT k.id, k.kind, k.subject_asset,
                    (SELECT a.trashed_at_unix_ms IS NULL FROM assets a WHERE a.id = k.subject_asset)
             FROM ignored_duplicate_actions k
             ORDER BY k.created_at_unix_ms DESC, k.id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, IgnoreId>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<AssetId>>(2)?,
                row.get::<_, Option<bool>>(3)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (action, kind, subject, subject_available) = row?;
            let present = members.remove(&action).unwrap_or_default();
            match (IgnoreKind::from_text(&kind), subject) {
                (IgnoreKind::Group, _) if present.len() >= 2 => entries.push(IgnoredEntry {
                    action,
                    kind: IgnoreKind::Group,
                    assets: present.into_iter().collect(),
                    against: Vec::new(),
                }),
                (IgnoreKind::Member, Some(subject))
                    if subject_available == Some(true) && !present.is_empty() =>
                {
                    entries.push(IgnoredEntry {
                        action,
                        kind: IgnoreKind::Member,
                        assets: vec![subject],
                        against: present.into_iter().collect(),
                    });
                }
                _ => {}
            }
        }
        Ok(entries)
    }

    pub fn ignored_duplicate_count(&self) -> Result<usize, LibraryError> {
        Ok(self.ignored_duplicate_entries()?.len())
    }

    pub(super) fn ignored_sets(&self) -> Result<IgnoredSets, LibraryError> {
        let mut sets = IgnoredSets::default();
        let mut index: HashMap<IgnoreId, u32> = HashMap::new();
        let mut statement = self
            .connection
            .prepare("SELECT id, kind, subject_asset FROM ignored_duplicate_actions")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, IgnoreId>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<AssetId>>(2)?,
            ))
        })?;
        let mut kinds: HashMap<u32, IgnoreKind> = HashMap::new();
        for row in rows {
            let (id, kind, subject) = row?;
            let next = u32::try_from(index.len()).unwrap_or(u32::MAX);
            index.insert(id, next);
            kinds.insert(next, IgnoreKind::from_text(&kind));
            if let Some(subject) = subject {
                sets.subjects.entry(subject).or_default().push(next);
            }
        }
        let mut statement = self
            .connection
            .prepare("SELECT action_id, asset_id FROM ignored_duplicate_members")?;
        for row in statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))? {
            let (action, asset): (IgnoreId, AssetId) = row?;
            let Some(&slot) = index.get(&action) else {
                continue;
            };
            match kinds.get(&slot) {
                Some(IgnoreKind::Group) => sets.groups.entry(asset).or_default().push(slot),
                _ => sets.targets.entry(asset).or_default().push(slot),
            }
        }
        Ok(sets)
    }

    pub fn is_ignored_duplicate(
        &self,
        first: AssetId,
        second: AssetId,
    ) -> Result<bool, LibraryError> {
        let in_action = "EXISTS (SELECT 1 FROM ignored_duplicate_members m
                                 WHERE m.action_id = k.id AND m.asset_id = ";
        let sql = format!(
            "SELECT 1 FROM ignored_duplicate_actions k WHERE
               (k.kind = 'group' AND {in_action}?1) AND {in_action}?2))
            OR (k.kind = 'member' AND
                ((k.subject_asset = ?1 AND {in_action}?2))
                 OR (k.subject_asset = ?2 AND {in_action}?1))))
             LIMIT 1"
        );
        Ok(self
            .connection
            .query_row(&sql, params![first, second], |_| Ok(()))
            .optional()?
            .is_some())
    }
}

pub(super) fn forget_dead_actions(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "DELETE FROM ignored_duplicate_actions
         WHERE kind = 'group'
           AND (SELECT count(*) FROM ignored_duplicate_members WHERE action_id = id) < 2;
         DELETE FROM ignored_duplicate_actions
         WHERE kind = 'member'
           AND (SELECT count(*) FROM ignored_duplicate_members WHERE action_id = id) < 1;",
    )
}

fn apply(
    connection: &Connection,
    command: &DuplicateCommand,
) -> Result<DuplicateCommand, DuplicateError> {
    match command {
        DuplicateCommand::Ignore(action) => {
            if let Some(subject) = action.subject
                && !exists(connection, subject)?
            {
                return Err(DuplicateError::NotFound(subject));
            }
            connection.execute(
                "INSERT INTO ignored_duplicate_actions (id, kind, subject_asset, created_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    action.id,
                    action.kind.text(),
                    action.subject,
                    action.created_at_unix_ms
                ],
            )?;
            let mut insert = connection.prepare(
                "INSERT OR IGNORE INTO ignored_duplicate_members (action_id, asset_id)
                 SELECT ?1, id FROM assets WHERE id = ?2",
            )?;
            for member in &action.members {
                insert.execute(params![action.id, member])?;
            }
            Ok(DuplicateCommand::Unignore(action.id))
        }
        DuplicateCommand::Unignore(id) => {
            let stored = connection
                .query_row(
                    "SELECT kind, subject_asset, created_at_unix_ms
                     FROM ignored_duplicate_actions WHERE id = ?1",
                    [id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<AssetId>>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((kind, subject, created_at_unix_ms)) = stored else {
                return Err(DuplicateError::UnknownAction(*id));
            };
            let mut statement = connection.prepare(
                "SELECT asset_id FROM ignored_duplicate_members WHERE action_id = ?1 ORDER BY asset_id",
            )?;
            let members = statement
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<AssetId>, _>>()?;
            connection.execute("DELETE FROM ignored_duplicate_actions WHERE id = ?1", [id])?;
            Ok(DuplicateCommand::Ignore(IgnoreAction {
                id: *id,
                kind: IgnoreKind::from_text(&kind),
                subject,
                members,
                created_at_unix_ms,
            }))
        }
        DuplicateCommand::Batch(commands) => {
            let mut inverses = commands
                .iter()
                .map(|command| apply(connection, command))
                .collect::<Result<Vec<_>, _>>()?;
            inverses.reverse();
            Ok(DuplicateCommand::Batch(inverses))
        }
    }
}

fn exists(connection: &Connection, asset: AssetId) -> Result<bool, DuplicateError> {
    Ok(connection
        .query_row("SELECT 1 FROM assets WHERE id = ?1", [asset], |_| Ok(()))
        .optional()?
        .is_some())
}

fn root(parent: &HashMap<String, String>, asset: &str) -> String {
    let mut current = asset.to_string();
    while let Some(next) = parent.get(&current).cloned() {
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(super) fn convert_pairs(connection: &Connection) -> Result<(), LibraryError> {
    connection.execute_batch(include_str!("migrations/v13.sql"))?;
    let mut statement = connection
        .prepare("SELECT first_asset, second_asset, ignored_at_unix_ms FROM ignored_duplicates")?;
    let pairs: Vec<(String, String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    drop(statement);
    let mut parent: HashMap<String, String> = HashMap::new();
    for (first, second, _) in &pairs {
        parent.entry(first.clone()).or_insert_with(|| first.clone());
        parent
            .entry(second.clone())
            .or_insert_with(|| second.clone());
        let (a, b) = (root(&parent, first), root(&parent, second));
        if a != b {
            parent.insert(a, b);
        }
    }
    let mut components: HashMap<String, (BTreeSet<String>, i64)> = HashMap::new();
    for (first, second, moment) in &pairs {
        let key = root(&parent, first);
        let entry = components
            .entry(key)
            .or_insert_with(|| (BTreeSet::new(), *moment));
        entry.0.insert(first.clone());
        entry.0.insert(second.clone());
        entry.1 = entry.1.min(*moment);
    }
    for (members, moment) in components.into_values() {
        let id = IgnoreId::generate();
        connection.execute(
            "INSERT INTO ignored_duplicate_actions (id, kind, subject_asset, created_at_unix_ms)
             VALUES (?1, 'group', NULL, ?2)",
            params![id, moment],
        )?;
        for member in members {
            connection.execute(
                "INSERT INTO ignored_duplicate_members (action_id, asset_id) VALUES (?1, ?2)",
                params![id, member],
            )?;
        }
    }
    connection.execute_batch("DROP TABLE ignored_duplicates")?;
    Ok(())
}
