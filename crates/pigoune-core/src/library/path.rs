use crate::ObjectHash;
use std::path::Path;

pub(crate) fn valid_object_path(hash: ObjectHash, path: &Path) -> bool {
    let Some(value) = path.to_str() else {
        return false;
    };
    let hash_text = hash.to_string();
    let mut parts = value.split('/');
    if parts.next() != Some("objects") || parts.next() != Some(&hash_text[..2]) {
        return false;
    }
    let Some(filename) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    if filename == hash_text {
        return true;
    }
    let Some(extension) = filename.strip_prefix(&(hash_text + ".")) else {
        return false;
    };
    !extension.is_empty()
        && extension.len() <= 16
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}
