use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};

use super::{AssetId, LibraryError, layout};

const THUMBNAIL_EXTENSION: &str = ".png";

pub fn record_bucketed_paths(connection: &Connection) -> Result<(), LibraryError> {
    for (id, stored_path) in stored_paths(connection)? {
        if let Some(bucketed) = bucketed_stored_path(id, &stored_path) {
            connection.execute(
                "UPDATE assets SET stored_path = ?1 WHERE id = ?2",
                params![bucketed, id],
            )?;
        }
    }
    Ok(())
}

pub fn move_into_buckets(root: &Path) {
    move_folder_entries(&layout::files_dir(root), asset_of_folder, Leftover::Keep);
    let Ok(sizes) = fs::read_dir(layout::thumbnails_dir(root)) else {
        return;
    };
    for size in sizes.flatten() {
        move_folder_entries(&size.path(), asset_of_thumbnail, Leftover::Remove);
    }
}

#[derive(Clone, Copy)]
enum Leftover {
    Keep,
    Remove,
}

fn move_folder_entries(folder: &Path, asset_of: fn(&str) -> Option<AssetId>, leftover: Leftover) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = asset_of(&name.to_string_lossy()) else {
            continue;
        };
        let bucket = folder.join(layout::bucket_name(id));
        let destination = bucket.join(&name);
        let moved = destination.symlink_metadata().is_err()
            && fs::create_dir_all(&bucket).is_ok()
            && fs::rename(entry.path(), &destination).is_ok();
        if !moved && matches!(leftover, Leftover::Remove) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn asset_of_folder(name: &str) -> Option<AssetId> {
    AssetId::parse(name)
}

fn asset_of_thumbnail(name: &str) -> Option<AssetId> {
    AssetId::parse(name.strip_suffix(THUMBNAIL_EXTENSION)?)
}

fn bucketed_stored_path(id: AssetId, stored_path: &str) -> Option<String> {
    let file_name = stored_path.strip_prefix(&format!("{}/{id}/", layout::FILES_DIR_NAME))?;
    Some(layout::stored_path(id, file_name))
}

fn stored_paths(connection: &Connection) -> Result<Vec<(AssetId, String)>, LibraryError> {
    let mut statement = connection.prepare("SELECT id, stored_path FROM assets")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::{asset_of_thumbnail, bucketed_stored_path};
    use crate::library::AssetId;

    const ID: &str = "01a10769-97bf-7319-b05f-99fc751d637f";

    fn id() -> AssetId {
        AssetId::parse(ID).expect("valid identifier")
    }

    #[test]
    fn an_old_stored_path_gains_the_bucket_of_its_asset() {
        assert_eq!(
            bucketed_stored_path(id(), &format!("files/{ID}/picture.png")).as_deref(),
            Some(format!("files/7f/{ID}/picture.png").as_str())
        );
    }

    #[test]
    fn a_stored_path_already_in_its_bucket_is_left_alone() {
        assert_eq!(
            bucketed_stored_path(id(), &format!("files/7f/{ID}/picture.png")),
            None
        );
    }

    #[test]
    fn only_finished_thumbnails_name_an_asset() {
        assert_eq!(asset_of_thumbnail(&format!("{ID}.png")), Some(id()));
        assert_eq!(asset_of_thumbnail(&format!(".{ID}.png.partial")), None);
        assert_eq!(asset_of_thumbnail("7f"), None);
    }
}
