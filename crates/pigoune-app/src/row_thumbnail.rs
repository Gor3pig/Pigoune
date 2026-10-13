use std::path::PathBuf;
use std::rc::Rc;

use gtk::glib;
use pigoune_core::AssetId;

use crate::thumbnails::{self, ThumbnailCache};

const SIDE: i32 = 48;

pub fn blank() -> gtk::Picture {
    gtk::Picture::builder()
        .width_request(SIDE)
        .height_request(SIDE)
        .content_fit(gtk::ContentFit::Contain)
        .can_shrink(true)
        .build()
}

pub fn picture(file: PathBuf, thumbnail_file: PathBuf) -> gtk::Picture {
    let picture = blank();
    glib::spawn_future_local(glib::clone!(
        #[weak]
        picture,
        async move {
            if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file).await {
                picture.set_paintable(Some(&texture));
            }
        }
    ));
    picture
}

pub fn load(
    picture: &gtk::Picture,
    id: AssetId,
    file: PathBuf,
    thumbnail_file: PathBuf,
    cache: &Rc<ThumbnailCache>,
) {
    glib::spawn_future_local(glib::clone!(
        #[weak]
        picture,
        #[strong]
        cache,
        async move {
            if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file).await {
                cache.remember(id, texture.clone());
                picture.set_paintable(Some(&texture));
            }
        }
    ));
}
