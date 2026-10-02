use std::path::{Path, PathBuf};

use adw::subclass::prelude::*;
use gtk::glib;
use pigoune_core::{Asset, AssetId};

pub struct AssetEntry {
    pub asset: Asset,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
}

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::AssetEntry;

    #[derive(Default)]
    pub struct PigouneAssetObject {
        pub entry: OnceCell<AssetEntry>,
        pub name_key: OnceCell<glib::FilenameCollationKey>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetObject {
        const NAME: &'static str = "PigouneAssetObject";
        type Type = super::PigouneAssetObject;
    }

    impl ObjectImpl for PigouneAssetObject {}
}

glib::wrapper! {
    pub struct PigouneAssetObject(ObjectSubclass<imp::PigouneAssetObject>);
}

impl PigouneAssetObject {
    pub fn new(entry: AssetEntry) -> Self {
        let object: Self = glib::Object::new();
        if object.imp().entry.set(entry).is_err() {
            unreachable!("a new asset object has no entry yet");
        }
        object
    }

    fn entry(&self) -> &AssetEntry {
        self.imp()
            .entry
            .get()
            .expect("asset objects are created with their entry")
    }

    pub fn asset(&self) -> &Asset {
        &self.entry().asset
    }

    pub fn id(&self) -> AssetId {
        self.asset().id
    }

    pub fn display_name(&self) -> &str {
        &self.asset().display_name
    }

    pub fn name_key(&self) -> &glib::FilenameCollationKey {
        self.imp()
            .name_key
            .get_or_init(|| glib::FilenameCollationKey::from(self.display_name()))
    }

    pub fn file(&self) -> &Path {
        &self.entry().file
    }

    pub fn thumbnail_file(&self) -> &Path {
        &self.entry().thumbnail_file
    }
}
