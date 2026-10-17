use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::annotated_files;
use crate::thumbnails;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::{AnnotationRender, Asset, AssetId, TextField};

pub struct AssetEntry {
    pub asset: Asset,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
    pub annotation: Option<AnnotationRender>,
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

    pub fn is_unchanged_by(&self, newer: &Self) -> bool {
        let Asset {
            id,
            display_name: _,
            original_file_name,
            stored_path,
            format,
            dimensions,
            byte_size,
            content_hash,
            is_animated,
            embedded_sizes,
            added_at_unix_ms,
            trashed_at_unix_ms,
            is_favorite: _,
            note: _,
            source_url: _,
            license: _,
            author: _,
            colors,
        } = self.asset();
        let other = newer.asset();
        *id == other.id
            && *original_file_name == other.original_file_name
            && *stored_path == other.stored_path
            && *format == other.format
            && *dimensions == other.dimensions
            && *byte_size == other.byte_size
            && *content_hash == other.content_hash
            && *is_animated == other.is_animated
            && *embedded_sizes == other.embedded_sizes
            && *added_at_unix_ms == other.added_at_unix_ms
            && *trashed_at_unix_ms == other.trashed_at_unix_ms
            && *colors == other.colors
            && self.source_file() == newer.source_file()
            && self.thumbnail_file() == newer.thumbnail_file()
            && self.annotation_key() == newer.annotation_key()
    }

    pub fn adopt_edits_of(&self, newer: &Self) -> bool {
        if self.favorite() != newer.favorite() {
            self.set_favorite(newer.favorite());
        }
        if self.note() != newer.note() {
            self.set_note(newer.note());
        }
        if self.source_url() != newer.source_url() {
            self.set_source_url(newer.source_url());
        }
        if self.license() != newer.license() {
            self.set_license(newer.license());
        }
        if self.author() != newer.author() {
            self.set_author(newer.author());
        }
        let renamed = self.display_name() != newer.display_name();
        if renamed {
            self.set_display_name(newer.display_name());
        }
        renamed
    }

    pub fn source_file(&self) -> &Path {
        &self.entry().file
    }

    pub fn thumbnail_file(&self) -> &Path {
        &self.entry().thumbnail_file
    }

    pub fn annotation(&self) -> Option<&AnnotationRender> {
        self.entry().annotation.as_ref()
    }

    pub fn annotation_key(&self) -> Option<&str> {
        self.annotation().map(|render| render.key.as_str())
    }

    pub async fn display_file(&self, widget: &impl IsA<gtk::Widget>) -> Option<PathBuf> {
        match self.annotation() {
            Some(render) => annotated_files::ensure(render, self.source_file(), widget).await,
            None => Some(self.source_file().to_path_buf()),
        }
    }

    pub async fn thumbnail(&self, widget: &impl IsA<gtk::Widget>) -> Option<gdk::Texture> {
        let source = async { self.display_file(widget).await };
        thumbnails::thumbnail_from(source, self.thumbnail_file()).await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pigoune_core::{Asset, AssetFormat, AssetId};

    use super::{AssetEntry, PigouneAssetObject};

    fn asset(name: &str, favorite: bool, byte_size: u64) -> Asset {
        Asset {
            id: AssetId::parse("00000000-0000-7000-8000-000000000001").expect("valid id"),
            display_name: name.to_owned(),
            original_file_name: "logo.png".to_owned(),
            stored_path: PathBuf::from("files/logo.png"),
            format: AssetFormat::Png,
            dimensions: None,
            byte_size,
            content_hash: "hash".to_owned(),
            is_animated: false,
            embedded_sizes: Vec::new(),
            added_at_unix_ms: 1,
            trashed_at_unix_ms: None,
            is_favorite: favorite,
            note: String::new(),
            source_url: String::new(),
            license: String::new(),
            author: String::new(),
            colors: Vec::new(),
        }
    }

    fn object(asset: Asset) -> PigouneAssetObject {
        PigouneAssetObject::new(AssetEntry {
            asset,
            file: PathBuf::from("/library/files/logo.png"),
            thumbnail_file: PathBuf::from("/library/thumbnail.png"),
            annotation: None,
        })
    }

    #[test]
    fn edits_do_not_make_an_asset_a_different_picture() {
        let shown = object(asset("Logo", false, 10));

        assert!(shown.is_unchanged_by(&object(asset("Renamed", true, 10))));
        assert!(!shown.is_unchanged_by(&object(asset("Logo", false, 11))));
        assert!(!shown.is_unchanged_by(&PigouneAssetObject::new(AssetEntry {
            asset: asset("Logo", false, 10),
            file: PathBuf::from("/other-library/files/logo.png"),
            thumbnail_file: PathBuf::from("/library/thumbnail.png"),
            annotation: None,
        })));
    }

    #[test]
    fn edits_of_a_newer_copy_are_adopted_in_place() {
        let shown = object(asset("Logo", false, 10));
        let newer = object(asset("Renamed", true, 10));

        assert!(shown.adopt_edits_of(&newer));

        assert!(shown.favorite());
        assert_eq!(shown.display_name(), "Renamed");
        assert!(!shown.adopt_edits_of(&newer));
    }
}
