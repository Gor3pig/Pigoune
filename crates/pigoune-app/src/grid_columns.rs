pub fn widest_column_count(grid_width: i32, tile_size: i32) -> u32 {
    if tile_size <= 0 {
        return 1;
    }
    u32::try_from(grid_width.max(0) / tile_size + 1).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::widest_column_count;

    #[test]
    fn leaves_room_for_every_tile_that_fits() {
        assert_eq!(widest_column_count(1000, 64), 16);
        assert_eq!(widest_column_count(1024, 256), 5);
    }

    #[test]
    fn always_keeps_at_least_one_column() {
        assert_eq!(widest_column_count(0, 64), 1);
        assert_eq!(widest_column_count(-20, 64), 1);
        assert_eq!(widest_column_count(500, 0), 1);
    }

    #[test]
    fn a_narrow_grid_still_shows_one_column() {
        assert_eq!(widest_column_count(40, 64), 1);
    }
}
