use gettextrs::{gettext, ngettext};

use crate::image_conversion::{ConversionError, TargetFormat};

pub struct Failure {
    pub name: String,
    pub reason: String,
}

pub fn success_text(count: usize, format: TargetFormat, folder_name: &str) -> String {
    ngettext(
        "{count} asset exported as {format} to “{name}”",
        "{count} assets exported as {format} to “{name}”",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
    .replace("{format}", format.name())
    .replace("{name}", folder_name)
}

pub fn progress_text(count: usize, format: TargetFormat) -> String {
    ngettext(
        "Exporting {count} asset as {format}…",
        "Exporting {count} assets as {format}…",
        u32::try_from(count).unwrap_or(u32::MAX),
    )
    .replace("{count}", &count.to_string())
    .replace("{format}", format.name())
}

pub fn failures_heading(count: usize) -> String {
    ngettext(
        "{count} Asset Was Not Exported",
        "{count} Assets Were Not Exported",
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
        ConversionError::TooSmallForIcon => {
            gettext("smaller than every chosen icon size, and pixel images are never enlarged")
        }
        ConversionError::TooLarge => gettext("the requested size is too large"),
        ConversionError::EncodingFailed => gettext("the conversion failed"),
    }
}

pub fn copy_failure_reason(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => gettext("its file is missing from the library"),
        _ => gettext("the file could not be copied"),
    }
}

pub fn name_suffix(frame_number: Option<usize>, size: Option<&str>) -> Option<String> {
    let frame = frame_number
        .map(|number| gettext("frame-{number}").replace("{number}", &number.to_string()));
    let parts: Vec<String> = frame.into_iter().chain(size.map(str::to_owned)).collect();
    (!parts.is_empty()).then(|| parts.join("-"))
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::{copy_failure_reason, name_suffix};

    #[test]
    fn a_missing_file_is_told_apart_from_other_copy_failures() {
        assert_eq!(
            copy_failure_reason(&Error::from(ErrorKind::NotFound)),
            "its file is missing from the library"
        );
        assert_eq!(
            copy_failure_reason(&Error::from(ErrorKind::PermissionDenied)),
            "the file could not be copied"
        );
    }

    #[test]
    fn the_frame_and_the_size_both_appear_in_the_name() {
        assert_eq!(name_suffix(None, None), None);
        assert_eq!(name_suffix(None, Some("64x64")).as_deref(), Some("64x64"));
        assert_eq!(name_suffix(Some(3), None).as_deref(), Some("frame-3"));
        assert_eq!(
            name_suffix(Some(3), Some("64x64")).as_deref(),
            Some("frame-3-64x64")
        );
    }
}
