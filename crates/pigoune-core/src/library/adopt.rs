use std::path::{Component, Path};

use super::import::{self, insert_asset};
use super::{AdoptError, AssetId, Library, layout};

impl Library {
    pub fn adopt_unrecorded(&mut self, file: &Path) -> Result<AssetId, AdoptError> {
        let id = self.adoptable_id(file)?;
        let prepared = import::prepare(&self.root.join(file))?;
        if let Some(known) = self.find_by_content_hash(&prepared.digest().hash)? {
            return Err(AdoptError::AlreadyKnown(file.to_path_buf(), known));
        }
        insert_asset(&self.connection, id, &prepared)?;
        Ok(id)
    }

    fn adoptable_id(&self, file: &Path) -> Result<AssetId, AdoptError> {
        let not_adoptable = || AdoptError::NotAdoptable(file.to_path_buf());
        let names: Vec<&str> = file
            .components()
            .map(|component| match component {
                Component::Normal(name) => name.to_str().ok_or_else(not_adoptable),
                _ => Err(not_adoptable()),
            })
            .collect::<Result<_, _>>()?;
        let [files, _bucket, folder, name] = names.as_slice() else {
            return Err(not_adoptable());
        };
        let id = AssetId::parse(folder).ok_or_else(not_adoptable)?;
        let expected = layout::stored_path(id, name);
        if *files != layout::FILES_DIR_NAME || Path::new(&expected) != file {
            return Err(not_adoptable());
        }
        if !self.root.join(file).is_file() || self.asset(id)?.is_some() {
            return Err(not_adoptable());
        }
        Ok(id)
    }
}
