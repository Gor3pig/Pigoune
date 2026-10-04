use gettextrs::{gettext, ngettext};

use crate::image_conversion::{ConversionError, TargetFormat};

pub struct Failure {
    pub name: String,
    pub reason: String,
}

pub fn success_text(count: usize, format: TargetFormat, folder_name: &str) -> String {
    ngettext(
        "{count} resource exported as {format} to “{name}”",
        "{count} resources exported as {format} to “{name}”",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
    .replace("{format}", format.name())
    .replace("{name}", folder_name)
}

pub fn progress_text(count: usize, format: TargetFormat) -> String {
    ngettext(
        "Exporting {count} resource as {format}…",
        "Exporting {count} resources as {format}…",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
    .replace("{format}", format.name())
}

pub fn failures_heading(count: usize) -> String {
    ngettext(
        "{count} Resource Was Not Exported",
        "{count} Resources Were Not Exported",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
}

pub fn failures_body(failures: &[Failure]) -> String {
    failures
        .iter()
        .map(|failure| {
            gettext("{name}: {reason}")
                .replace("{name}", &failure.name)
                .replace("{reason}", &failure.reason)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn reason(error: ConversionError) -> String {
    match error {
        ConversionError::Unreadable => gettext("the image could not be read"),
        ConversionError::TooLargeForIcon => {
            gettext("too large for ICO, which allows 256 × 256 pixels at most")
        }
        ConversionError::EncodingFailed => gettext("the conversion failed"),
    }
}
