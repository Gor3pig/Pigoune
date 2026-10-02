use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::{Asset, AssetId, TextField};

pub struct AssetEntry {
    pub asset: Asset,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};
    use std::rc::Rc;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::AssetEntry;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PigouneAssetObject)]
    pub struct PigouneAssetObject {
        pub entry: OnceCell<AssetEntry>,
        pub name_key: RefCell<Option<Rc<glib::FilenameCollationKey>>>,
        #[property(get, set)]
        pub favorite: Cell<bool>,
        #[property(get, set = Self::set_display_name)]
        pub display_name: RefCell<String>,
        #[property(get, set)]
        pub note: RefCell<String>,
        #[property(get, set)]
        pub source_url: RefCell<String>,
        #[property(get, set)]
        pub license: RefCell<String>,
        #[property(get, set)]
        pub author: RefCell<String>,
    }

    impl PigouneAssetObject {
        fn set_display_name(&self, name: String) {
            self.name_key.replace(None);
            self.display_name.replace(name);
            self.obj().notify_display_name();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetObject {
        const NAME: &'static str = "PigouneAssetObject";
        type Type = super::PigouneAssetObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneAssetObject {}
}

glib::wrapper! {
    pub struct PigouneAssetObject(ObjectSubclass<imp::PigouneAssetObject>);
}

impl PigouneAssetObject {
    pub fn new(entry: AssetEntry) -> Self {
        let asset = &entry.asset;
        let object: Self = glib::Object::builder()
            .property("favorite", asset.is_favorite)
            .property("display-name", &asset.display_name)
            .property("note", &asset.note)
            .property("source-url", &asset.source_url)
            .property("license", &asset.license)
            .property("author", &asset.author)
            .build();
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

    pub fn name_key(&self) -> Rc<glib::FilenameCollationKey> {
        let mut key = self.imp().name_key.borrow_mut();
        Rc::clone(
            key.get_or_insert_with(|| {
                Rc::new(glib::FilenameCollationKey::from(self.display_name()))
            }),
        )
    }

    pub fn text(&self, field: TextField) -> String {
        match field {
            TextField::Note => self.note(),
            TextField::SourceUrl => self.source_url(),
            TextField::License => self.license(),
            TextField::Author => self.author(),
        }
    }

    pub fn set_text(&self, field: TextField, value: &str) {
        let property = match field {
            TextField::Note => "note",
            TextField::SourceUrl => "source-url",
            TextField::License => "license",
            TextField::Author => "author",
        };
        self.set_property(property, value);
    }

    pub fn file(&self) -> &Path {
        &self.entry().file
    }

    pub fn thumbnail_file(&self) -> &Path {
        &self.entry().thumbnail_file
    }
}
