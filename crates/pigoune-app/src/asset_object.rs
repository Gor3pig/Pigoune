use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::annotated_files;
use crate::thumbnails;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::{AnnotationRender, Asset, AssetId, OriginalFile, TextField};

pub struct AssetEntry {
    pub asset: Asset,
    pub file: Option<OriginalFile>,
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
        let object: Self = glib::Object::new();
        let imp = object.imp();
        let asset = &entry.asset;
        imp.favorite.set(asset.is_favorite);
        imp.display_name.replace(asset.display_name.clone());
        imp.note.replace(asset.note.clone());
        imp.source_url.replace(asset.source_url.clone());
        imp.license.replace(asset.license.clone());
        imp.author.replace(asset.author.clone());
        if imp.entry.set(entry).is_err() {
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
        let (mine, theirs) = (self.imp(), newer.imp());
        if mine.favorite.get() != theirs.favorite.get() {
            self.set_favorite(theirs.favorite.get());
        }
        if *mine.note.borrow() != *theirs.note.borrow() {
            self.set_note(newer.note());
        }
        if *mine.source_url.borrow() != *theirs.source_url.borrow() {
            self.set_source_url(newer.source_url());
        }
        if *mine.license.borrow() != *theirs.license.borrow() {
            self.set_license(newer.license());
        }
        if *mine.author.borrow() != *theirs.author.borrow() {
            self.set_author(newer.author());
        }
        let renamed = *mine.display_name.borrow() != *theirs.display_name.borrow();
        if renamed {
            self.set_display_name(newer.display_name());
        }
        renamed
    }

    pub fn source_file(&self) -> Option<&Path> {
        self.entry().file.as_ref().map(OriginalFile::path)
    }

    pub fn original(&self) -> Option<&OriginalFile> {
        self.entry().file.as_ref()
    }

    pub fn readable_source(&self) -> Option<PathBuf> {
        let original = self.entry().file.as_ref()?;
        original.check().ok()?;
        Some(original.path().to_path_buf())
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
            Some(render) => {
                let source = self.readable_source()?;
                annotated_files::ensure(render, &source, widget).await
            }
            None => self.readable_source(),
        }
    }

    pub async fn thumbnail(&self, widget: &impl IsA<gtk::Widget>) -> Option<gdk::Texture> {
        let Some(render) = self.annotation() else {
            return thumbnails::thumbnail_of(self.original(), self.thumbnail_file()).await;
        };
        let drawn = async {
            if pigoune_core::cache_file(&render.file) == pigoune_core::CacheFile::Plain {
                return thumbnails::render(&render.file, thumbnails::THUMBNAIL_PIXELS).await;
            }
            let source = self.readable_source()?;
            annotated_files::reduced(render, &source, widget, thumbnails::side_of_thumbnails())
                .await
        };
        thumbnails::thumbnail_drawn(drawn, self.thumbnail_file()).await
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pigoune_core::{Asset, AssetFormat, AssetId, OriginalFile};

    use super::{AssetEntry, PigouneAssetObject};

    const ID: &str = "00000000-0000-7000-8000-000000000001";

    fn original_in(root: &str) -> Option<OriginalFile> {
        let id = AssetId::parse(ID).expect("valid id");
        OriginalFile::new(
            std::path::Path::new(root),
            id,
            &PathBuf::from(format!("files/01/{ID}/logo.png")),
        )
        .ok()
    }

    fn asset(name: &str, favorite: bool, byte_size: u64) -> Asset {
        Asset {
            id: AssetId::parse(ID).expect("valid id"),
            display_name: name.to_owned(),
            original_file_name: "logo.png".to_owned(),
            stored_path: PathBuf::from(format!("files/01/{ID}/logo.png")),
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
            file: original_in("/library"),
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
            file: original_in("/other-library"),
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

    fn counting_notifications(shown: &PigouneAssetObject) -> std::rc::Rc<std::cell::Cell<u32>> {
        let notified = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = notified.clone();
        gtk::prelude::ObjectExt::connect_notify_local(shown, None, move |_, _| {
            counter.set(counter.get() + 1);
        });
        notified
    }

    #[test]
    fn adopting_identical_edits_notifies_nothing() {
        let mut source = asset("Logo", true, 10);
        source.note = "a note".to_owned();
        let shown = object(source.clone());
        let notified = counting_notifications(&shown);

        assert!(!shown.adopt_edits_of(&object(source)));

        assert_eq!(notified.get(), 0);
    }

    #[test]
    fn every_edited_field_is_adopted_and_notified_once() {
        let shown = object(asset("Logo", false, 10));
        let mut edited = asset("Logo", true, 10);
        edited.note = "a note".to_owned();
        edited.source_url = "https://example.org/logo".to_owned();
        edited.license = "CC0".to_owned();
        edited.author = "Someone".to_owned();
        let notified = counting_notifications(&shown);

        assert!(!shown.adopt_edits_of(&object(edited)));

        assert_eq!(notified.get(), 5);
        assert!(shown.favorite());
        assert_eq!(shown.note(), "a note");
        assert_eq!(shown.source_url(), "https://example.org/logo");
        assert_eq!(shown.license(), "CC0");
        assert_eq!(shown.author(), "Someone");
        assert_eq!(shown.display_name(), "Logo");
    }

    #[test]
    fn a_rename_is_reported_and_refreshes_the_sort_key() {
        let shown = object(asset("Logo", false, 10));
        let before = shown.name_key();

        assert!(shown.adopt_edits_of(&object(asset("Zebra", false, 10))));

        assert!(!std::rc::Rc::ptr_eq(&before, &shown.name_key()));
        assert_eq!(
            *shown.name_key(),
            gtk::glib::FilenameCollationKey::from("Zebra")
        );
    }

    #[test]
    fn a_new_object_carries_every_edit_of_its_asset_and_still_notifies_later_changes() {
        let mut source = asset("Logo", true, 10);
        source.note = "a note".to_owned();
        source.source_url = "https://example.org/logo".to_owned();
        source.license = "CC0".to_owned();
        source.author = "Someone".to_owned();
        let shown = object(source);

        assert!(shown.favorite());
        assert_eq!(shown.display_name(), "Logo");
        assert_eq!(shown.note(), "a note");
        assert_eq!(shown.source_url(), "https://example.org/logo");
        assert_eq!(shown.license(), "CC0");
        assert_eq!(shown.author(), "Someone");
        assert_eq!(shown.id(), shown.asset().id);
        let before = shown.name_key();
        assert_eq!(*before, gtk::glib::FilenameCollationKey::from("Logo"));

        let notified = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = notified.clone();
        shown.connect_display_name_notify(move |_| counter.set(counter.get() + 1));
        shown.set_display_name("Renamed");

        assert_eq!(notified.get(), 1);
        assert_eq!(
            *shown.name_key(),
            gtk::glib::FilenameCollationKey::from("Renamed")
        );
    }

    fn annotated(asset: Asset, key: &str) -> PigouneAssetObject {
        PigouneAssetObject::new(AssetEntry {
            asset,
            file: original_in("/library"),
            thumbnail_file: PathBuf::from("/library/thumbnail.png"),
            annotation: Some(pigoune_core::AnnotationRender {
                key: key.to_owned(),
                document: "{}".to_owned(),
                file: PathBuf::from(format!("/library/cache/annotated/{key}.png")),
            }),
        })
    }

    #[test]
    fn a_changed_annotation_makes_the_picture_different_and_the_source_stays_the_file() {
        let shown = annotated(asset("Logo", false, 10), "one");

        assert!(shown.is_unchanged_by(&annotated(asset("Renamed", true, 10), "one")));
        assert!(!shown.is_unchanged_by(&annotated(asset("Logo", false, 10), "two")));
        assert!(!shown.is_unchanged_by(&object(asset("Logo", false, 10))));
        assert_eq!(
            shown.source_file(),
            Some(PathBuf::from(format!("/library/files/01/{ID}/logo.png")).as_path())
        );
        assert_eq!(shown.annotation_key(), Some("one"));
        assert_eq!(object(asset("Logo", false, 10)).annotation_key(), None);
    }

    #[test]
    fn an_asset_without_a_valid_original_has_no_source_and_nothing_is_read() {
        let shown = PigouneAssetObject::new(AssetEntry {
            asset: asset("Logo", false, 10),
            file: None,
            thumbnail_file: PathBuf::from("/library/thumbnail.png"),
            annotation: None,
        });

        assert_eq!(shown.source_file(), None);
        assert_eq!(shown.readable_source(), None);
        assert!(original_in("/library").is_some());
        let id = AssetId::parse(ID).expect("valid id");
        assert!(
            OriginalFile::new(
                std::path::Path::new("/library"),
                id,
                &PathBuf::from("/etc/passwd")
            )
            .is_err()
        );
        assert!(
            OriginalFile::new(
                std::path::Path::new("/library"),
                id,
                &PathBuf::from(format!("files/01/{ID}/../../../x"))
            )
            .is_err()
        );
    }
}
