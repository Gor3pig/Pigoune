use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const CELLS: usize = 4;
const COLUMNS: usize = 2;
const CELL_PIXELS: i32 = 72;
const IMAGE_PIXELS: i32 = 60;

#[derive(Debug, PartialEq, Eq)]
struct MosaicPlan {
    shown: usize,
    more: Option<usize>,
}

fn plan(selected: usize) -> MosaicPlan {
    if selected <= CELLS {
        MosaicPlan {
            shown: selected,
            more: None,
        }
    } else {
        MosaicPlan {
            shown: CELLS - 1,
            more: Some(selected - (CELLS - 1)),
        }
    }
}

pub fn fill(
    grid: &gtk::Grid,
    selected: &[PigouneAssetObject],
    cache: &Rc<ThumbnailCache>,
) -> Vec<glib::JoinHandle<()>> {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
    let plan = plan(selected.len());
    let mut loading = Vec::new();
    for (index, asset) in selected.iter().take(plan.shown).enumerate() {
        let image = gtk::Image::builder().pixel_size(IMAGE_PIXELS).build();
        match cache.remembered(asset.id()) {
            Some(texture) => image.set_paintable(Some(&texture)),
            None => loading.push(load(&image, asset, cache)),
        }
        attach(grid, &cell(&image.upcast()), index);
    }
    if let Some(more) = plan.more {
        let label = gtk::Label::builder()
            .label(format!("+{more}"))
            .css_classes(["title-4", "dim-label", "numeric"])
            .build();
        attach(grid, &cell(&label.upcast()), plan.shown);
    }
    loading
}

fn cell(content: &gtk::Widget) -> gtk::Widget {
    let frame = gtk::Box::builder()
        .width_request(CELL_PIXELS)
        .height_request(CELL_PIXELS)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .hexpand(false)
        .vexpand(false)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["mosaic-cell"])
        .build();
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    frame.append(content);
    frame.upcast()
}

fn attach(grid: &gtk::Grid, cell: &gtk::Widget, index: usize) {
    let column = i32::try_from(index % COLUMNS).unwrap_or(0);
    let row = i32::try_from(index / COLUMNS).unwrap_or(0);
    grid.attach(cell, column, row, 1, 1);
}

fn load(
    image: &gtk::Image,
    asset: &PigouneAssetObject,
    cache: &Rc<ThumbnailCache>,
) -> glib::JoinHandle<()> {
    let id = asset.id();
    let file = asset.file().to_path_buf();
    let thumbnail_file = asset.thumbnail_file().to_path_buf();
    glib::spawn_future_local(glib::clone!(
        #[weak]
        image,
        #[strong]
        cache,
        async move {
            if let Some(texture) = thumbnails::thumbnail(&file, &thumbnail_file).await {
                cache.remember(id, texture.clone());
                image.set_paintable(Some(&texture));
            }
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::{MosaicPlan, plan};

    #[test]
    fn up_to_four_resources_are_all_shown() {
        assert_eq!(
            plan(2),
            MosaicPlan {
                shown: 2,
                more: None
            }
        );
        assert_eq!(
            plan(4),
            MosaicPlan {
                shown: 4,
                more: None
            }
        );
    }

    #[test]
    fn beyond_four_the_last_cell_counts_the_others() {
        assert_eq!(
            plan(5),
            MosaicPlan {
                shown: 3,
                more: Some(2)
            }
        );
        assert_eq!(
            plan(120),
            MosaicPlan {
                shown: 3,
                more: Some(117)
            }
        );
    }
}
