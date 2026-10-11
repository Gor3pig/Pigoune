pub fn unchanged_edges<Old, New>(
    old: &[Old],
    new: &[New],
    same: impl Fn(&Old, &New) -> bool,
) -> (usize, usize) {
    let mut start = 0;
    while start < old.len() && start < new.len() && same(&old[start], &new[start]) {
        start += 1;
    }
    let mut end = 0;
    while end < old.len() - start
        && end < new.len() - start
        && same(&old[old.len() - 1 - end], &new[new.len() - 1 - end])
    {
        end += 1;
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::unchanged_edges;

    fn edges(old: &[u32], new: &[u32]) -> (usize, usize) {
        unchanged_edges(old, new, |first, second| first == second)
    }

    #[test]
    fn identical_lists_are_entirely_unchanged() {
        assert_eq!(edges(&[1, 2, 3], &[1, 2, 3]), (3, 0));
    }

    #[test]
    fn an_insertion_in_the_middle_keeps_both_edges() {
        assert_eq!(edges(&[1, 2, 5, 6], &[1, 2, 3, 4, 5, 6]), (2, 2));
    }

    #[test]
    fn a_removal_at_the_start_keeps_the_end() {
        assert_eq!(edges(&[1, 2, 3, 4], &[3, 4]), (0, 2));
    }

    #[test]
    fn nothing_in_common_changes_everything() {
        assert_eq!(edges(&[1, 2], &[3, 4, 5]), (0, 0));
    }

    #[test]
    fn empty_lists_are_handled() {
        assert_eq!(edges(&[], &[1, 2]), (0, 0));
        assert_eq!(edges(&[1, 2], &[]), (0, 0));
        assert_eq!(edges(&[], &[]), (0, 0));
    }

    #[test]
    fn the_two_edges_never_overlap() {
        assert_eq!(edges(&[1, 1, 1], &[1, 1]), (2, 0));
        assert_eq!(edges(&[1, 1], &[1, 1, 1]), (2, 0));
    }
}
