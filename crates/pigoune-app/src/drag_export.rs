use gettextrs::gettext;
use pigoune_core::AssetId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    HiddenMasks(Vec<String>),
    NotReady,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub files: Vec<AssetId>,
    pub refusal: Option<Refusal>,
}

pub fn plan(dragged: &[AssetId], hidden_masks_in: Vec<String>, not_ready: usize) -> Plan {
    let refusal = if !hidden_masks_in.is_empty() {
        Some(Refusal::HiddenMasks(hidden_masks_in))
    } else if not_ready > 0 {
        Some(Refusal::NotReady)
    } else {
        None
    };
    Plan {
        files: if refusal.is_some() {
            Vec::new()
        } else {
            dragged.to_vec()
        },
        refusal,
    }
}

pub fn refusal_text(refusal: &Refusal) -> String {
    match refusal {
        Refusal::HiddenMasks(names) => match names.as_slice() {
            [name] => {
                gettext("Show the hidden masks of “{name}” before dragging it out of Pigoune")
                    .replace("{name}", name)
            }
            _ => gettext(
                "Show the hidden masks of {count} images before dragging them out of Pigoune",
            )
            .replace("{count}", &names.len().to_string()),
        },
        Refusal::NotReady => gettext(
            "The annotated image is still being prepared. Try dragging it again in a moment",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<AssetId> {
        (0..count)
            .map(|index| {
                AssetId::parse(&format!("00000000-0000-7000-8000-{index:012}")).expect("valid id")
            })
            .collect()
    }

    #[test]
    fn everything_is_dragged_out_when_every_mask_is_visible_and_every_rendering_is_ready() {
        let dragged = ids(3);
        let plan = plan(&dragged, Vec::new(), 0);
        assert_eq!(plan.files, dragged);
        assert_eq!(plan.refusal, None);
    }

    #[test]
    fn a_hidden_mask_refuses_the_whole_drag_and_names_the_image() {
        let plan = plan(&ids(3), vec!["Holiday".to_owned()], 0);
        assert!(plan.files.is_empty(), "no file is handed over at all");
        assert_eq!(
            plan.refusal,
            Some(Refusal::HiddenMasks(vec!["Holiday".to_owned()]))
        );
        let text = refusal_text(plan.refusal.as_ref().expect("refused"));
        assert!(text.contains("Holiday"));
    }

    #[test]
    fn several_hidden_masks_are_counted() {
        let refusal = Refusal::HiddenMasks(vec!["A".to_owned(), "B".to_owned()]);
        assert!(refusal_text(&refusal).contains('2'));
    }

    #[test]
    fn a_rendering_that_is_not_ready_refuses_instead_of_dropping_images_silently() {
        let plan = plan(&ids(2), Vec::new(), 1);
        assert!(plan.files.is_empty());
        assert_eq!(plan.refusal, Some(Refusal::NotReady));
    }

    #[test]
    fn hidden_masks_come_before_a_rendering_that_is_not_ready() {
        let plan = plan(&ids(2), vec!["A".to_owned()], 2);
        assert!(matches!(plan.refusal, Some(Refusal::HiddenMasks(_))));
    }

    #[test]
    fn dragging_out_never_offers_to_continue_with_a_hidden_mask() {
        let window = include_str!("window.rs");
        let start = window
            .find("drag_export::plan(")
            .expect("the drag uses the plan");
        let around: String = window[start..].lines().take(30).collect();
        assert!(!around.contains("mask_alert"), "the drag only refuses");
        assert!(!include_str!("mask_alert.rs").contains("Drag"));
    }
}
