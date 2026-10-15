const SEPARATORS: [char; 4] = ['-', '_', ' ', '.'];
const SHORTEST_COMMON_NAME: usize = 3;

pub fn common_name(names: &[&str], fallback: &str) -> String {
    let Some((first, others)) = names.split_first() else {
        return fallback.to_string();
    };
    let shared = others.iter().fold(first.to_string(), |shared, name| {
        shared
            .chars()
            .zip(name.chars())
            .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
            .map(|(left, _)| left)
            .collect()
    });
    let trimmed = shared
        .trim_end_matches(SEPARATORS)
        .trim_start_matches(SEPARATORS);
    if trimmed.chars().count() >= SHORTEST_COMMON_NAME {
        trimmed.to_string()
    } else {
        fallback.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::common_name;

    #[test]
    fn names_sharing_a_beginning_give_that_beginning() {
        assert_eq!(
            common_name(&["croissant-3d", "croissant-sticker"], "croissant-3d"),
            "croissant"
        );
        assert_eq!(
            common_name(
                &["gem-stone-3d", "gem-stone-sticker", "gem-stone-flat"],
                "gem-stone-3d"
            ),
            "gem-stone"
        );
    }

    #[test]
    fn a_name_that_is_the_beginning_of_the_others_is_the_common_name() {
        assert_eq!(common_name(&["fox-3d", "fox"], "fox-3d"), "fox");
    }

    #[test]
    fn names_with_nothing_in_common_give_the_fallback() {
        assert_eq!(common_name(&["logo", "marque-2024"], "logo"), "logo");
        assert_eq!(common_name(&["ab-1", "ab-2"], "ab-1"), "ab-1");
    }

    #[test]
    fn the_comparison_ignores_the_case() {
        assert_eq!(common_name(&["Logo-A", "logo-b"], "Logo-A"), "Logo");
    }

    #[test]
    fn no_names_give_the_fallback() {
        assert_eq!(common_name(&[], "fallback"), "fallback");
    }
}
