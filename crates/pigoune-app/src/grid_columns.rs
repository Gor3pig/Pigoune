pub fn column_count(grid_width: i32, smallest_column: i32) -> u32 {
    if smallest_column <= 0 {
        return 1;
    }
    u32::try_from(grid_width.max(0) / smallest_column + 1).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::column_count;

    #[test]
    fn leaves_room_for_every_column_that_fits() {
        assert_eq!(column_count(1000, 162), 7);
        assert_eq!(column_count(810, 162), 6);
    }

    #[test]
    fn always_keeps_at_least_one_column() {
        assert_eq!(column_count(0, 162), 1);
        assert_eq!(column_count(-20, 162), 1);
        assert_eq!(column_count(500, 0), 1);
    }

    #[test]
    fn a_narrow_grid_still_shows_one_column() {
        assert_eq!(column_count(40, 162), 1);
    }
}
