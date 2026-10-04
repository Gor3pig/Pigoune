use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::ImportError;
use super::staging::StagingDir;

const BATCH_DURATION: Duration = Duration::from_secs(1);

pub(super) struct ImportBatch {
    staged: Vec<(StagingDir, PathBuf)>,
    started: Instant,
}

impl ImportBatch {
    pub(super) fn begin(connection: &Connection) -> Result<Self, ImportError> {
        connection.execute_batch("BEGIN")?;
        Ok(Self {
            staged: Vec::new(),
            started: Instant::now(),
        })
    }

    pub(super) fn stage(&mut self, staging: StagingDir, destination: PathBuf) {
        self.staged.push((staging, destination));
    }

    pub(super) fn is_due(&self) -> bool {
        self.started.elapsed() >= BATCH_DURATION
    }

    pub(super) fn commit(self, connection: &Connection) -> Result<(), ImportError> {
        let mut promoted = Vec::with_capacity(self.staged.len());
        for (staging, destination) in self.staged {
            if let Err(error) = promote(staging, &destination) {
                roll_back(connection, &promoted);
                return Err(error.into());
            }
            promoted.push(destination);
        }
        if let Err(error) = connection.execute_batch("COMMIT") {
            roll_back(connection, &promoted);
            return Err(error.into());
        }
        Ok(())
    }

    pub(super) fn abandon(self, connection: &Connection) {
        roll_back(connection, &[]);
        drop(self.staged);
    }
}

fn promote(staging: StagingDir, destination: &Path) -> io::Result<()> {
    if let Some(bucket) = destination.parent() {
        fs::create_dir_all(bucket)?;
    }
    staging.promote_to(destination)
}

fn roll_back(connection: &Connection, promoted: &[PathBuf]) {
    let _ = connection.execute_batch("ROLLBACK");
    for destination in promoted {
        let _ = fs::remove_dir_all(destination);
    }
}
