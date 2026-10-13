use std::path::PathBuf;

use gtk::glib;

use crate::thumbnails;

const SIDE: i32 = 48;

pub fn picture(file: PathBuf, thumbnail_file: PathBuf) -> gtk::Picture {
    let picture = gtk::Picture::builder()
        .width_request(SIDE)
        .height_request(SIDE)
        .content_fit(gtk::ContentFit::Contain)
        .can_shrink(true)
        .build();
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
