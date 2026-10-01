use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct StagingDir {
    path: PathBuf,
    promoted: bool,
}

impl StagingDir {
    pub fn create(path: PathBuf) -> io::Result<Self> {
        fs::create_dir(&path)?;
        Ok(Self {
            path,
            promoted: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn promote_to(mut self, destination: &Path) -> io::Result<()> {
        fs::rename(&self.path, destination)?;
        self.promoted = true;
        Ok(())
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        if !self.promoted {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StagingDir;
    use std::fs;

    #[test]
    fn an_abandoned_staging_dir_is_removed_with_its_content() {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let staging_path = workspace.path().join("staging");

        let staging = StagingDir::create(staging_path.clone()).expect("staging created");
        fs::write(staging.path().join("partial.db"), "half written").expect("file written");
        drop(staging);

        assert!(!staging_path.exists());
    }

    #[test]
    fn a_promoted_staging_dir_becomes_the_destination() {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let staging_path = workspace.path().join("staging");
        let destination = workspace.path().join("final");

        let staging = StagingDir::create(staging_path.clone()).expect("staging created");
        fs::write(staging.path().join("complete.db"), "done").expect("file written");
        staging
            .promote_to(&destination)
            .expect("promotion succeeds");

        assert!(!staging_path.exists());
        assert!(destination.join("complete.db").is_file());
    }

    #[test]
    fn a_failed_promotion_removes_the_staging_dir_and_keeps_the_destination() {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let staging_path = workspace.path().join("staging");
        let destination = workspace.path().join("final");
        fs::create_dir(&destination).expect("destination created");
        fs::write(destination.join("precious.txt"), "keep me").expect("file written");

        let staging = StagingDir::create(staging_path.clone()).expect("staging created");
        assert!(staging.promote_to(&destination).is_err());

        assert!(!staging_path.exists());
        assert!(destination.join("precious.txt").is_file());
    }
}
