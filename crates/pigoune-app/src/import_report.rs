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
        || summary.ignored_links > 0
        || !matches!(summary.ending, ImportEnding::Completed)
}

pub fn toast_text(summary: &ImportSummary, collection: Option<&str>) -> String {
    let lines = outcome_lines(summary, collection);
    if lines.is_empty() {
        gettext("No images to import were found")
    } else {
        lines.join(", ")
    }
}

pub fn summary_dialog(
    summary: &ImportSummary,
    chosen: &[PathBuf],
    collection: Option<&str>,
) -> adw::AlertDialog {
    let mut lines = outcome_lines(summary, collection);
    lines.extend(problem_lines(summary));
    if lines.is_empty() {
        lines.push(gettext("Nothing was imported."));
    }
    if summary.unreadable.iter().any(|path| is_heic_path(path)) {
        lines.push(gettext(
            "HEIC photos can only be imported when your system can decode them: the Flatpak package can.",
        ));
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
        ImportEnding::Cancelled => gettext("Import Canceled"),
        ImportEnding::Interrupted(_) => gettext("Import Interrupted"),
    }
}

fn outcome_lines(summary: &ImportSummary, collection: Option<&str>) -> Vec<String> {
    let imported = count_for_plural(summary.imported.len());
    let imported_text = match collection {
        Some(collection) => ngettext(
            "{count} asset imported into “{collection}”",
            "{count} assets imported into “{collection}”",
            imported,
        )
        .replace("{collection}", collection),
        None => ngettext(
            "{count} asset imported",
            "{count} assets imported",
            imported,
        ),
    };
    [
        (summary.imported.len(), imported_text),
        (
            summary.already_present,
            ngettext(
                "{count} asset already in the library",
                "{count} assets already in the library",
                count_for_plural(summary.already_present),
            ),
        ),
        (
            summary.added_to_collection,
            ngettext(
                "{count} asset already in the library, added to its collection",
                "{count} assets already in the library, added to their collections",
                count_for_plural(summary.added_to_collection),
            ),
        ),
        (
            summary.restored_from_trash,
            ngettext(
                "{count} asset restored from the trash",
                "{count} assets restored from the trash",
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
                "{count} large asset imported (over 50 MB): it takes up more disk space and its preview may take longer to appear",
                "{count} large assets imported (over 50 MB): they take up more disk space and their previews may take longer to appear",
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
            summary.ignored_links,
            ngettext(
                "{count} link ignored: links to other files or folders are not followed",
                "{count} links ignored: links to other files or folders are not followed",
                count_for_plural(summary.ignored_links),
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

fn is_heic_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
        })
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

    use super::{display_path, is_heic_path};

    #[test]
    fn heic_files_are_recognized_by_their_extension() {
        assert!(is_heic_path(&PathBuf::from("/photos/IMG_0001.HEIC")));
        assert!(is_heic_path(&PathBuf::from("beach.heif")));
        assert!(!is_heic_path(&PathBuf::from("beach.jpg")));
        assert!(!is_heic_path(&PathBuf::from("heic")));
    }

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
