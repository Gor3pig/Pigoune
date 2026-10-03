#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLayout {
    pub columns: u32,
    pub thumbnail_side: i32,
}

pub fn fill_width(grid_width: i32, smallest_thumbnail: i32, space_around: i32) -> GridLayout {
    let width = grid_width.max(0);
    let smallest_cell = (smallest_thumbnail + space_around).max(1);
    let columns = (width / smallest_cell).max(1);
    let thumbnail_side = (width / columns - space_around).max(1);
    GridLayout {
        columns: u32::try_from(columns).unwrap_or(1),
        thumbnail_side,
    }
}

#[cfg(test)]
mod tests {
    use super::{GridLayout, fill_width};

    #[test]
    fn thumbnails_grow_to_fill_the_whole_row() {
        assert_eq!(
            fill_width(1000, 128, 34),
            GridLayout {
                columns: 6,
                thumbnail_side: 132,
            }
        );
    }

    #[test]
    fn a_row_that_fits_exactly_keeps_the_chosen_size() {
        assert_eq!(
            fill_width(810, 128, 34),
            GridLayout {
                columns: 5,
                thumbnail_side: 128,
            }
        );
    }

    #[test]
    fn the_filled_row_never_overflows_the_grid() {
        for width in 35..2000 {
            let layout = fill_width(width, 96, 34);
            let row = i32::try_from(layout.columns).unwrap() * (layout.thumbnail_side + 34);
            assert!(row <= width, "width {width}");
        }
    }

    #[test]
    fn thumbnails_never_shrink_below_the_chosen_size_when_a_column_fits() {
        for width in 130..2000 {
            assert!(
                fill_width(width, 96, 34).thumbnail_side >= 96,
                "width {width}"
            );
        }
    }

    #[test]
    fn a_narrow_grid_shrinks_its_single_column() {
        assert_eq!(
            fill_width(100, 128, 34),
            GridLayout {
                columns: 1,
                thumbnail_side: 66,
            }
        );
    }

    #[test]
    fn an_empty_grid_still_shows_one_column() {
        assert_eq!(fill_width(0, 64, 34).columns, 1);
        assert_eq!(fill_width(-20, 64, 34).columns, 1);
    }
}
