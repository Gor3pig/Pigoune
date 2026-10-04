use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};

use gtk::{gio, glib};

const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Documents";
const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/documents";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Documents";
const PORTAL_TIMEOUT_MS: i32 = 1000;
const DOCUMENTS_MOUNT_NAME: &str = "doc";

thread_local! {
    static KNOWN_HOST_PATHS: RefCell<HashMap<PathBuf, PathBuf>> = RefCell::new(HashMap::new());
}

#[must_use]
pub fn shown_path(path: &Path) -> PathBuf {
    let mount = glib::user_runtime_dir().join(DOCUMENTS_MOUNT_NAME);
    let Some(document) = DocumentPath::within(path, &mount) else {
        return path.to_path_buf();
    };
    if let Some(known) = KNOWN_HOST_PATHS.with_borrow(|known| known.get(path).cloned()) {
        return known;
    }
    let shown = host_document_path(&document.id)
        .map_or_else(|| path.to_path_buf(), |host| host.join(&document.inside));
    KNOWN_HOST_PATHS.with_borrow_mut(|known| known.insert(path.to_path_buf(), shown.clone()));
    shown
}

#[must_use]
pub fn shown_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| shown_path(Path::new(path)).to_string_lossy().into_owned())
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct DocumentPath {
    id: String,
    inside: PathBuf,
}

impl DocumentPath {
    fn within(path: &Path, mount: &Path) -> Option<Self> {
        let mut components = path.strip_prefix(mount).ok()?.components();
        let Some(Component::Normal(id)) = components.next() else {
            return None;
        };
        let Some(Component::Normal(_document_name)) = components.next() else {
            return None;
        };
        Some(Self {
            id: id.to_str()?.to_owned(),
            inside: components.as_path().to_path_buf(),
        })
    }
}

fn host_document_path(id: &str) -> Option<PathBuf> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let reply = bus
        .call_sync(
            Some(PORTAL_BUS_NAME),
            PORTAL_OBJECT_PATH,
            PORTAL_INTERFACE,
            "GetHostPaths",
            Some(&(vec![id],).into()),
            Some(glib::VariantTy::new("(a{say})").ok()?),
            gio::DBusCallFlags::NONE,
            PORTAL_TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let (paths,) = reply.get::<(HashMap<String, Vec<u8>>,)>()?;
    paths.get(id).map(|bytes| path_from_bytes(bytes))
}

fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    let without_terminator = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    PathBuf::from(OsString::from_vec(without_terminator.to_vec()))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{DocumentPath, path_from_bytes};

    const MOUNT: &str = "/run/user/1000/doc";

    #[test]
    fn a_document_path_names_its_portal_document() {
        assert_eq!(
            DocumentPath::within(
                Path::new("/run/user/1000/doc/a1b2/Logos.pigoune"),
                Path::new(MOUNT)
            ),
            Some(DocumentPath {
                id: "a1b2".to_owned(),
                inside: PathBuf::new(),
            })
        );
    }

    #[test]
    fn a_path_inside_a_document_keeps_its_inner_part() {
        assert_eq!(
            DocumentPath::within(
                Path::new("/run/user/1000/doc/a1b2/Documents/Logos.pigoune"),
                Path::new(MOUNT)
            ),
            Some(DocumentPath {
                id: "a1b2".to_owned(),
                inside: PathBuf::from("Logos.pigoune"),
            })
        );
    }

    #[test]
    fn an_ordinary_path_is_not_a_document() {
        assert_eq!(
            DocumentPath::within(Path::new("/srv/graphics/Logos.pigoune"), Path::new(MOUNT)),
            None
        );
        assert_eq!(
            DocumentPath::within(Path::new("/run/user/1000/doc/a1b2"), Path::new(MOUNT)),
            None
        );
    }

    #[test]
    fn the_portal_path_loses_its_final_zero_byte() {
        assert_eq!(
            path_from_bytes(b"/srv/graphics/Logos.pigoune\0"),
            PathBuf::from("/srv/graphics/Logos.pigoune")
        );
        assert_eq!(
            path_from_bytes(b"/srv/graphics/Logos.pigoune"),
            PathBuf::from("/srv/graphics/Logos.pigoune")
        );
    }
}
