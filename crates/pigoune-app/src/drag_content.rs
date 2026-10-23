use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetId, CollectionId, SmartCollectionId, TagId};

#[derive(Clone, Debug, Default, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedAssets")]
pub struct DraggedAssets(pub Vec<AssetId>);

#[derive(Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedCollection")]
pub struct DraggedCollection(pub CollectionId);

#[derive(Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedTag")]
pub struct DraggedTag(pub TagId);

#[derive(Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedSmartCollection")]
pub struct DraggedSmartCollection(pub SmartCollectionId);

pub fn documents_mount() -> PathBuf {
    glib::user_runtime_dir().join("doc")
}

pub fn needs_real_paths(copies: &[PathBuf], documents: &Path) -> bool {
    copies.iter().any(|copy| copy.starts_with(documents))
}

pub fn uri_list(copies: &[PathBuf]) -> glib::Bytes {
    let mut text = String::new();
    for copy in copies {
        text.push_str(&gio::File::for_path(copy).uri());
        text.push_str("\r\n");
    }
    glib::Bytes::from_owned(text.into_bytes())
}

pub fn files_provider(copies: &[PathBuf], documents: &Path) -> gdk::ContentProvider {
    if needs_real_paths(copies, documents) {
        gdk::ContentProvider::for_bytes("text/uri-list", &uri_list(copies))
    } else {
        let files: Vec<gio::File> = copies.iter().map(gio::File::for_path).collect();
        gdk::ContentProvider::for_value(&gdk::FileList::from_array(&files).to_value())
    }
}

pub fn drag_provider(
    dragged: Vec<AssetId>,
    copies: &[PathBuf],
    documents: &Path,
) -> gdk::ContentProvider {
    let inside = gdk::ContentProvider::for_value(&DraggedAssets(dragged).to_value());
    if copies.is_empty() {
        return inside;
    }
    gdk::ContentProvider::new_union(&[inside, files_provider(copies, documents)])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gtk::gdk;
    use gtk::gdk::prelude::ContentProviderExt;
    use gtk::glib::prelude::StaticType;
    use pigoune_core::AssetId;

    use super::{DraggedAssets, drag_provider, files_provider, needs_real_paths, uri_list};

    fn documents() -> PathBuf {
        PathBuf::from("/run/user/1000/doc")
    }

    fn inside_documents() -> PathBuf {
        PathBuf::from("/run/user/1000/doc/7eab6aad/Ma bibliothèque.pigoune/cache/export/op/a b.png")
    }

    fn outside_documents() -> PathBuf {
        PathBuf::from("/data/app/cache/export/op/a.png")
    }

    #[test]
    fn copies_inside_the_documents_portal_cannot_go_through_the_file_transfer_portal() {
        assert!(needs_real_paths(&[inside_documents()], &documents()));
        assert!(needs_real_paths(
            &[outside_documents(), inside_documents()],
            &documents()
        ));
        assert!(!needs_real_paths(&[outside_documents()], &documents()));
        assert!(!needs_real_paths(&[], &documents()));
        assert!(!needs_real_paths(
            &[PathBuf::from("/run/user/1000/documents-not/a.png")],
            &documents()
        ));
    }

    #[test]
    fn the_list_of_addresses_has_one_escaped_address_per_line_ending_with_crlf() {
        let bytes = uri_list(&[inside_documents(), outside_documents()]);
        let text = String::from_utf8(bytes.to_vec()).expect("an address list is text");
        assert_eq!(
            text,
            "file:///run/user/1000/doc/7eab6aad/Ma%20biblioth%C3%A8que.pigoune/cache/export/op/a%20b.png\r\n\
             file:///data/app/cache/export/op/a.png\r\n"
        );
    }

    #[test]
    fn real_paths_are_offered_as_an_address_list_and_never_as_a_file_list_for_the_portal() {
        let provider = files_provider(&[inside_documents()], &documents());
        let formats = provider.formats();
        assert!(formats.contain_mime_type("text/uri-list"));
        assert!(!formats.contains_type(gdk::FileList::static_type()));
    }

    #[test]
    fn other_copies_keep_the_file_list_that_lets_the_portal_serve_sandboxed_receivers() {
        let provider = files_provider(&[outside_documents()], &documents());
        assert!(
            provider
                .formats()
                .contains_type(gdk::FileList::static_type())
        );
    }

    #[test]
    fn the_dragged_assets_stay_available_inside_pigoune_whatever_the_files() {
        let assets = vec![AssetId::parse("00000000-0000-7000-8000-000000000001").expect("id")];
        for copies in [
            Vec::new(),
            vec![inside_documents()],
            vec![outside_documents()],
        ] {
            let provider = drag_provider(assets.clone(), &copies, &documents());
            assert!(
                provider
                    .formats()
                    .contains_type(DraggedAssets::static_type())
            );
        }
        let none = drag_provider(assets.clone(), &[], &documents());
        assert!(!none.formats().contain_mime_type("text/uri-list"));
    }
}
