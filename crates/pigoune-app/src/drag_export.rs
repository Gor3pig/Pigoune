use std::path::Path;

use gettextrs::gettext;
use gtk::gio;
use gtk::prelude::*;
use pigoune_core::{AssetId, VolumeSpace};

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
                gettext("To drag “{name}” out of Pigoune, first show its hidden blur or pixelation effects.")
                    .replace("{name}", name)
            }
            _ => gettext(
                "To drag {count} images out of Pigoune, first show their hidden blur or pixelation effects.",
            )
            .replace("{count}", &names.len().to_string()),
        },
        Refusal::NotReady => gettext(
            "The annotated image is still being prepared. Try dragging it again in a moment",
        ),
    }
}

pub fn storage_text() -> String {
    gettext(
        "Not enough free space to drag these files out of Pigoune. Dragging inside Pigoune still works",
    )
}

pub fn preparation_text() -> String {
    gettext(
        "These files could not be prepared to drag out of Pigoune. Dragging inside Pigoune still works",
    )
}

pub fn volume_space(folder: &Path) -> Option<VolumeSpace> {
    let attributes = format!(
        "{},{}",
        gio::FILE_ATTRIBUTE_FILESYSTEM_FREE,
        gio::FILE_ATTRIBUTE_FILESYSTEM_SIZE
    );
    let information = gio::File::for_path(folder)
        .query_filesystem_info(&attributes, gio::Cancellable::NONE)
        .ok()?;
    Some(VolumeSpace {
        free: information.attribute_uint64(gio::FILE_ATTRIBUTE_FILESYSTEM_FREE),
        total: information.attribute_uint64(gio::FILE_ATTRIBUTE_FILESYSTEM_SIZE),
    })
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
    fn the_storage_text_says_the_internal_drag_still_works() {
        let text = storage_text();
        assert!(text.contains("free space"));
        assert!(text.contains("still works"));
    }

    #[test]
    fn opening_with_another_application_asks_the_mask_question_before_the_copy_and_checks_the_space()
     {
        let window = include_str!("window.rs");
        let start = window
            .find("async fn open_asset(")
            .expect("the opening is in the window");
        let body: String = window[start..]
            .lines()
            .take(40)
            .collect::<Vec<_>>()
            .join("\n");
        let question = body.find("output_is_allowed").expect("the opening asks");
        let copy = body
            .find("opening_copy_checking")
            .expect("the copy is made with the space check");
        let launch = body
            .find("launch_future")
            .expect("the application is launched");
        assert!(question < copy && copy < launch);
        assert!(!body.contains(".opening_copy("));
    }

    #[test]
    fn a_failed_preparation_of_the_drag_is_explained_and_says_the_internal_drag_still_works() {
        let text = preparation_text();
        assert!(text.contains("could not be prepared"));
        assert!(text.contains("still works"));
        assert_ne!(text, storage_text());
    }

    #[test]
    fn no_failure_of_the_drag_preparation_is_swallowed_without_a_message() {
        let window = include_str!("window.rs");
        let body = body_of(window, "fn offer_export_copies(", 40);
        assert!(!body.contains("Some(Err(_))"), "every error is reported");
        assert!(body.contains("preparation_text"));
        assert!(body.contains("g_debug!"));
    }

    #[test]
    fn the_reason_why_a_drag_out_was_cancelled_is_traced() {
        let grid = include_str!("asset_grid.rs");
        let body = body_of(grid, "fn make_draggable(", 60);
        assert!(body.contains("connect_drag_cancel"));
        assert!(body.contains("g_debug!"));
    }

    #[test]
    fn a_full_disk_while_opening_is_explained_by_the_usual_message() {
        let text = crate::error_messages::describe(&pigoune_core::LibraryError::StorageFull);
        assert!(text.contains("free space"));
    }

    fn body_of(source: &str, start_marker: &str, lines: usize) -> String {
        let start = source.find(start_marker).expect("the function is there");
        source[start..]
            .lines()
            .take(lines)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn show_original_has_its_own_action_and_the_old_name_is_gone() {
        let window = include_str!("window.rs");
        let details = include_str!("../ui/asset-details.blp");
        assert!(window.contains("\"win.show-original\""));
        assert!(details.contains("\"win.show-original\""));
        assert!(!window.contains("win.view-original") && !details.contains("win.view-original"));
        assert!(!window.contains("View Original") && !details.contains("View Original"));
        assert!(details.contains("Show Original"));
    }

    #[test]
    fn showing_the_original_reveals_the_stored_file_without_copying_or_launching_an_application() {
        let window = include_str!("window.rs");
        let body = body_of(window, "async fn show_selected_original(", 14);
        assert!(body.contains("readable_source"));
        assert!(body.contains("show_in_files"));
        assert!(!body.contains("launch_future(") && !body.contains("fs::copy"));
        let reveal = body_of(window, "async fn show_in_files(", 8);
        assert!(reveal.contains("open_containing_folder_future"));
        assert!(!reveal.contains(".launch_future("));
    }

    #[test]
    fn showing_an_annotated_image_in_files_shows_its_clean_copy_without_the_mask_question() {
        let window = include_str!("window.rs");
        let body = body_of(window, "async fn show_selected_in_files(", 24);
        assert!(body.contains("is_annotated"));
        assert!(body.contains("annotated_copy_to_show"));
        assert!(body.contains("readable_source"));
        assert!(!body.contains("output_is_allowed"));
        let copy = body_of(window, "async fn annotated_copy_to_show(", 16);
        assert!(copy.contains("flat_files_for") && copy.contains("opening_copy_checking"));
        assert!(!copy.contains("output_is_allowed"));
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
