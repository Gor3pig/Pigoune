use gettextrs::gettext;
use pigoune_core::LibraryExportReport;

use crate::conversion_report::Failure;
use crate::error_messages;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Complete(usize),
    Partial {
        exported: usize,
        missing: Vec<Failure>,
    },
}

pub fn outcome_of(report: &LibraryExportReport) -> Outcome {
    if report.failures.is_empty() {
        return Outcome::Complete(report.exported);
    }
    let missing = report
        .failures
        .iter()
        .map(|failure| Failure {
            name: failure.name.clone(),
            reason: if failure.annotated_copy {
                gettext("its annotated version could not be created")
            } else {
                error_messages::describe(&failure.error)
            },
        })
        .collect();
    Outcome::Partial {
        exported: report.exported,
        missing,
    }
}

pub fn partial_heading() -> String {
    gettext("Export Incomplete")
}

pub fn partial_body(exported: usize, folder: &str, missing: &[Failure]) -> String {
    let list: Vec<String> = missing
        .iter()
        .map(|failure| format!("{}: {}", failure.name, failure.reason))
        .collect();
    gettext(
        "The export is incomplete. {exported} files were exported to “{folder}”, but {count} could not be created:\n\n{list}",
    )
    .replace("{exported}", &exported.to_string())
    .replace("{folder}", folder)
    .replace("{count}", &missing.len().to_string())
    .replace("{list}", &list.join("\n"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pigoune_core::{ExportFailure, LibraryError};

    use super::*;

    fn report(exported: usize, failures: Vec<ExportFailure>) -> LibraryExportReport {
        LibraryExportReport {
            folder: PathBuf::from("/export"),
            exported,
            failures,
        }
    }

    fn failure(name: &str, annotated_copy: bool) -> ExportFailure {
        ExportFailure {
            name: name.to_owned(),
            annotated_copy,
            error: LibraryError::Io(std::io::Error::other("no")),
        }
    }

    #[test]
    fn an_export_without_failure_is_complete() {
        assert_eq!(outcome_of(&report(5, Vec::new())), Outcome::Complete(5));
    }

    #[test]
    fn a_single_missing_annotated_version_makes_the_export_partial_and_names_it() {
        let outcome = outcome_of(&report(4, vec![failure("Holiday", true)]));
        let Outcome::Partial { exported, missing } = outcome else {
            panic!("the export is not complete");
        };
        assert_eq!(exported, 4);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].name, "Holiday");
        assert!(missing[0].reason.contains("annotated version"));
        let body = partial_body(exported, "Library", &missing);
        assert!(body.contains("incomplete") && body.contains("Holiday") && body.contains('4'));
    }

    #[test]
    fn a_failed_original_is_not_described_as_a_missing_annotated_version() {
        let Outcome::Partial { missing, .. } = outcome_of(&report(0, vec![failure("Logo", false)]))
        else {
            panic!("not complete");
        };
        assert!(!missing[0].reason.contains("annotated version"));
    }
}
