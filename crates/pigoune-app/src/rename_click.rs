pub const RENAME_WINDOW_MS: u32 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameClick {
    pub layer: u32,
    pub at_ms: u32,
}

#[must_use]
pub fn starts_renaming(
    was_selected: bool,
    presses: i32,
    previous: Option<NameClick>,
    current: NameClick,
    double_click_ms: u32,
) -> bool {
    if presses >= 2 {
        return true;
    }
    let Some(previous) = previous.filter(|previous| previous.layer == current.layer) else {
        return false;
    };
    let elapsed = current.at_ms.wrapping_sub(previous.at_ms);
    was_selected && elapsed > double_click_ms && elapsed <= RENAME_WINDOW_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOUBLE: u32 = 400;

    fn click(layer: u32, at_ms: u32) -> NameClick {
        NameClick { layer, at_ms }
    }

    #[test]
    fn a_first_click_on_a_layer_only_selects_it() {
        assert!(!starts_renaming(false, 1, None, click(1, 5000), DOUBLE));
    }

    #[test]
    fn a_click_on_the_selected_layer_with_no_click_before_does_nothing() {
        assert!(!starts_renaming(true, 1, None, click(1, 5000), DOUBLE));
    }

    #[test]
    fn a_second_click_spaced_after_the_double_click_time_renames() {
        let previous = Some(click(1, 5000));
        assert!(starts_renaming(true, 1, previous, click(1, 5800), DOUBLE));
    }

    #[test]
    fn a_second_click_at_the_edge_of_the_double_click_time_does_not_rename() {
        let previous = Some(click(1, 5000));
        assert!(!starts_renaming(true, 1, previous, click(1, 5400), DOUBLE));
        assert!(starts_renaming(true, 1, previous, click(1, 5401), DOUBLE));
    }

    #[test]
    fn a_second_click_that_comes_too_late_does_not_rename() {
        let previous = Some(click(1, 5000));
        assert!(starts_renaming(true, 1, previous, click(1, 7000), DOUBLE));
        assert!(!starts_renaming(true, 1, previous, click(1, 7001), DOUBLE));
    }

    #[test]
    fn a_classic_double_click_renames_even_on_a_layer_that_was_not_selected() {
        assert!(starts_renaming(false, 2, None, click(1, 5000), DOUBLE));
    }

    #[test]
    fn a_spaced_click_after_a_click_on_another_layer_does_not_rename() {
        let previous = Some(click(2, 5000));
        assert!(!starts_renaming(true, 1, previous, click(1, 5800), DOUBLE));
    }

    #[test]
    fn a_spaced_click_on_a_layer_that_was_not_selected_does_not_rename() {
        let previous = Some(click(1, 5000));
        assert!(!starts_renaming(false, 1, previous, click(1, 5800), DOUBLE));
    }

    #[test]
    fn the_event_clock_wrapping_around_is_handled() {
        let previous = Some(click(1, u32::MAX - 100));
        assert!(starts_renaming(true, 1, previous, click(1, 700), DOUBLE));
    }
}
