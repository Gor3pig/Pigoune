use std::path::{Path, PathBuf};

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use pigoune_core::{ImportEnding, ImportError, ImportSummary};

use crate::error_messages;

const CLOSE_RESPONSE: &str = "close";
const UNREADABLE_LIST_HEIGHT: i32 = 160;

pub fn needs_attention(summary: &ImportSummary) -> bool {
    summary.unsupported > 0
        || summary.large_imported > 0
        || !summary.unreadable.is_empty()
        || !matches!(summary.ending, ImportEnding::Completed)
}

pub fn toast_text(summary: &ImportSummary, destination: Option<&str>) -> String {
    let lines = outcome_lines(summary, destination);
    if lines.is_empty() {
        gettext("No images to import were found")
    } else {
        lines.join(", ")
    }
}

pub fn summary_dialog(
    summary: &ImportSummary,
    chosen: &[PathBuf],
    destination: Option<&str>,
) -> adw::AlertDialog {
    let mut lines = outcome_lines(summary, destination);
    lines.extend(problem_lines(summary));
    if lines.is_empty() {
        lines.push(gettext("Nothing was imported."));
    }
    if let ImportEnding::Interrupted(error) = &summary.ending {
        lines.push(String::new());
        lines.push(describe_interruption(error));
    }

    let alert = adw::AlertDialog::new(Some(&heading(&summary.ending)), Some(&lines.join("\n")));
    alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
    if !summary.unreadable.is_empty() {
        alert.set_extra_child(Some(&unreadable_files_list(&summary.unreadable, chosen)));
    }
    alert
}

pub fn failure_dialog(error: &ImportError) -> adw::AlertDialog {
    let alert = adw::AlertDialog::new(
        Some(&gettext("Unable to Import")),
        Some(&describe_interruption(error)),
    );
    alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
    alert
}

pub fn unexpected_stop_dialog() -> adw::AlertDialog {
    let alert = adw::AlertDialog::new(
        Some(&gettext("Import Stopped Unexpectedly")),
        Some(&gettext(
            "The library was closed as a precaution. Open it again: everything imported so far is safe.",
        )),
    );
    alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
    alert
}

fn heading(ending: &ImportEnding) -> String {
    match ending {
        ImportEnding::Completed => gettext("Import Finished"),
        ImportEnding::Cancelled => gettext("Import Cancelled"),
        ImportEnding::Interrupted(_) => gettext("Import Interrupted"),
    }
}

fn outcome_lines(summary: &ImportSummary, destination: Option<&str>) -> Vec<String> {
    let imported = count_for_plural(summary.imported.len());
    let imported_text = match destination {
        Some(collection) => ngettext(
            "{count} resource imported into “{collection}”",
            "{count} resources imported into “{collection}”",
            imported,
        )
        .replace("{collection}", collection),
        None => ngettext(
            "{count} resource imported",
            "{count} resources imported",
            imported,
        ),
    };
    [
        (summary.imported.len(), imported_text),
        (
            summary.already_present,
            ngettext(
                "{count} resource already in the library",
                "{count} resources already in the library",
                count_for_plural(summary.already_present),
            ),
        ),
        (
            summary.added_to_collection,
            ngettext(
                "{count} resource already in the library, added to its collection",
                "{count} resources already in the library, added to their collections",
                count_for_plural(summary.added_to_collection),
            ),
        ),
        (
            summary.restored_from_trash,
            ngettext(
                "{count} resource restored from the trash",
                "{count} resources restored from the trash",
                count_for_plural(summary.restored_from_trash),
            ),
        ),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, text)| text.replace("{count}", &count.to_string()))
    .collect()
}

fn problem_lines(summary: &ImportSummary) -> Vec<String> {
    let unreadable = summary.unreadable.len();
    [
        (
            summary.large_imported,
            ngettext(
                "{count} large resource imported (over 50 MB): it takes up more disk space and its preview may take longer to appear",
                "{count} large resources imported (over 50 MB): they take up more disk space and their previews may take longer to appear",
                count_for_plural(summary.large_imported),
            ),
        ),
        (
            summary.unsupported,
            ngettext(
                "{count} file ignored: not a supported image format",
                "{count} files ignored: not a supported image format",
                count_for_plural(summary.unsupported),
            ),
        ),
        (
            unreadable,
            ngettext(
                "{count} file ignored: damaged or unreadable image",
                "{count} files ignored: damaged or unreadable images",
                count_for_plural(unreadable),
            ),
        ),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, text)| text.replace("{count}", &count.to_string()))
    .collect()
}

fn describe_interruption(error: &ImportError) -> String {
    match error {
        ImportError::Library(error) => error_messages::describe(error),
        ImportError::CollectionNotFound(_) => {
            gettext("The destination collection no longer exists.")
        }
        ImportError::UnsupportedFormat(_) | ImportError::Unreadable(_) => {
            gettext("An unexpected error occurred: {error}").replace("{error}", &error.to_string())
        }
    }
}

fn unreadable_files_list(unreadable: &[PathBuf], chosen: &[PathBuf]) -> gtk::Widget {
    let names: Vec<String> = unreadable
        .iter()
        .map(|path| display_path(path, chosen))
        .collect();
    let list = gtk::Label::builder()
        .label(names.join("\n"))
        .selectable(true)
        .wrap(true)
        .xalign(0.0)
        .valign(gtk::Align::Start)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(UNREADABLE_LIST_HEIGHT)
        .build();
    gtk::Expander::builder()
        .label(gettext("Show Unreadable Files"))
        .child(&scrolled)
        .build()
        .upcast()
}

fn display_path(path: &Path, chosen: &[PathBuf]) -> String {
    chosen
        .iter()
        .find_map(|root| {
            let inside = path.strip_prefix(root).ok()?;
            let root_name = Path::new(root.file_name()?);
            Some(if inside.as_os_str().is_empty() {
                root_name.to_path_buf()
            } else {
                root_name.join(inside)
            })
        })
        .unwrap_or_else(|| {
            path.file_name()
                .map_or_else(|| path.to_path_buf(), PathBuf::from)
        })
        .display()
        .to_string()
}

fn count_for_plural(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::display_path;

    #[test]
    fn a_file_inside_a_chosen_folder_is_shown_from_that_folder() {
        let chosen = [PathBuf::from("/run/user/1000/doc/ab12/Marques")];
        let path = PathBuf::from("/run/user/1000/doc/ab12/Marques/Tech/bad.png");
        assert_eq!(display_path(&path, &chosen), "Marques/Tech/bad.png");
    }

    #[test]
    fn a_chosen_file_is_shown_by_its_name() {
        let path = PathBuf::from("/media/usb-key/Images/bad.png");
        assert_eq!(display_path(&path, std::slice::from_ref(&path)), "bad.png");
        assert_eq!(display_path(&path, &[]), "bad.png");
    }
}
