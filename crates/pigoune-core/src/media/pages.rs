use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::{AssetFormat, Dimensions, InspectError, LARGEST_SIDE, MediaInfo};

const HEADER_LENGTH: u64 = 4096;
const PDF_SIGNATURE: &[u8] = b"%PDF-";
const PDF_SIGNATURE_WINDOW: usize = 1024;
const POSTSCRIPT_SIGNATURE: &[u8] = b"%!PS-Adobe-";
const DOS_EPS_SIGNATURE: [u8; 4] = [0xC5, 0xD0, 0xD3, 0xC6];
const DOS_EPS_HEADER_LENGTH: usize = 30;
const ILLUSTRATOR_CREATOR: &[u8] = b"%%Creator: Adobe Illustrator";
const ILLUSTRATOR_PDF_KEY: &[u8] = b"/Illustrator";
const BOUNDING_BOX: &[u8] = b"%%BoundingBox:";
const HIRES_BOUNDING_BOX: &[u8] = b"%%HiResBoundingBox:";
const DEFERRED_BOX: &[u8] = b"(atend)";
const MEDIA_BOX: &[u8] = b"/MediaBox";
const PAGE_COUNT: &[u8] = b"/Count";
const ROTATE: &[u8] = b"/Rotate";
const OBJECT_STREAM: &[u8] = b"/ObjStm";
const STREAM_KEYWORD: &[u8] = b"stream";
const END_STREAM_KEYWORD: &[u8] = b"endstream";
const POSTSCRIPT_WINDOW: u64 = 64 * 1024;
const PDF_WINDOW: u64 = 8 * 1024 * 1024;
const MOST_OBJECT_STREAMS: usize = 256;
const MOST_STREAM_ATTEMPTS: usize = 512;
const MOST_STREAM_BYTES: usize = 4 * 1024 * 1024;
const MOST_STREAM_SEARCH_BYTES: usize = 32 * 1024 * 1024;
const MOST_BOX_CANDIDATES: usize = 1024;
const MOST_INFLATE_FAILURES: usize = 8;
const MOST_INFLATED_BYTES: usize = 16 * 1024 * 1024;
const MOST_DICTIONARY_BYTES: usize = 16 * 1024;
const MOST_DICTIONARY_DEPTH: usize = 64;
const LINE_END: [u8; 2] = *b"\r\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFile {
    pub format: AssetFormat,
    pub dimensions: Option<Dimensions>,
    pub is_pdf: bool,
}

#[must_use]
pub fn page_file(path: &Path) -> Option<PageFile> {
    let head = read_window(path, 0, HEADER_LENGTH).ok()?;
    let detected = detect(&head)?;
    let info = inspect(path, detected).ok()?;
    Some(PageFile {
        format: info.format,
        dimensions: info.dimensions,
        is_pdf: is_pdf(&head),
    })
}

#[must_use]
pub(super) fn detect(header: &[u8]) -> Option<AssetFormat> {
    if is_pdf(header) {
        return Some(AssetFormat::Pdf);
    }
    if header.starts_with(&DOS_EPS_SIGNATURE) {
        let postscript = dos_postscript_range(header)?;
        let start = postscript.0.min(header.len());
        return postscript_format(&header[start..]);
    }
    postscript_format(header)
}

fn is_pdf(header: &[u8]) -> bool {
    let window = &header[..header.len().min(PDF_SIGNATURE_WINDOW)];
    find(window, PDF_SIGNATURE).is_some_and(|at| {
        let starts_a_line = at == 0 || matches!(window[at - 1], b'\n' | b'\r');
        starts_a_line
            && window
                .get(at + PDF_SIGNATURE.len())
                .is_some_and(u8::is_ascii_digit)
    })
}

fn postscript_format(bytes: &[u8]) -> Option<AssetFormat> {
    if !bytes.starts_with(POSTSCRIPT_SIGNATURE) {
        return None;
    }
    let first_line = bytes.split(|byte| LINE_END.contains(byte)).next()?;
    let is_encapsulated = find(first_line, b"EPSF").is_some();
    if find(bytes, ILLUSTRATOR_CREATOR).is_some() {
        return Some(AssetFormat::Ai);
    }
    is_encapsulated.then_some(AssetFormat::Eps)
}

fn dos_postscript_range(header: &[u8]) -> Option<(usize, usize)> {
    let fields = header.get(4..12)?;
    let start = usize::try_from(u32::from_le_bytes(fields[..4].try_into().ok()?)).ok()?;
    let length = usize::try_from(u32::from_le_bytes(fields[4..].try_into().ok()?)).ok()?;
    (start >= DOS_EPS_HEADER_LENGTH && length > 0).then_some((start, length))
}

pub(super) fn inspect(path: &Path, format: AssetFormat) -> Result<MediaInfo, InspectError> {
    let (format, dimensions) = match format {
        AssetFormat::Pdf | AssetFormat::Ai if is_pdf_file(path) => pdf_facts(path, format)?,
        _ => (format, postscript_dimensions(path)),
    };
    Ok(MediaInfo {
        format,
        dimensions,
        is_animated: false,
        embedded_sizes: Vec::new(),
    })
}

#[must_use]
pub(super) fn dimensions_of(path: &Path, format: AssetFormat) -> Option<Dimensions> {
    match format {
        AssetFormat::Pdf => pdf_facts(path, format).ok()?.1,
        AssetFormat::Ai if is_pdf_file(path) => pdf_facts(path, format).ok()?.1,
        _ => postscript_dimensions(path),
    }
}

fn is_pdf_file(path: &Path) -> bool {
    read_window(path, 0, PDF_SIGNATURE_WINDOW as u64).is_ok_and(|head| is_pdf(&head))
}

fn read_window(path: &Path, start: u64, length: u64) -> Result<Vec<u8>, InspectError> {
    let mut file = File::open(path).map_err(|_| InspectError::Unreadable)?;
    file.seek(SeekFrom::Start(start))
        .map_err(|_| InspectError::Unreadable)?;
    let mut bytes = Vec::new();
    file.take(length)
        .read_to_end(&mut bytes)
        .map_err(|_| InspectError::Unreadable)?;
    Ok(bytes)
}

fn file_length(path: &Path) -> Result<u64, InspectError> {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|_| InspectError::Unreadable)
}

fn pdf_facts(
    path: &Path,
    format: AssetFormat,
) -> Result<(AssetFormat, Option<Dimensions>), InspectError> {
    let length = file_length(path)?;
    let head = read_window(path, 0, PDF_WINDOW)?;
    let tail = if length > PDF_WINDOW {
        read_window(
            path,
            length.saturating_sub(PDF_WINDOW).max(PDF_WINDOW),
            PDF_WINDOW,
        )?
    } else {
        Vec::new()
    };
    let has_illustrator_data = [&head, &tail]
        .iter()
        .any(|window| find(window, ILLUSTRATOR_PDF_KEY).is_some());
    let named_as_illustrator = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ai"));
    let detected = if has_illustrator_data || (named_as_illustrator && format == AssetFormat::Ai) {
        AssetFormat::Ai
    } else {
        AssetFormat::Pdf
    };
    let dimensions = [&head, &tail]
        .iter()
        .find_map(|window| first_page_size(window));
    Ok((detected, dimensions))
}

#[must_use]
pub fn page_count(path: &Path) -> Option<u32> {
    if !is_pdf_file(path) {
        return None;
    }
    let length = file_length(path).ok()?;
    let head = read_window(path, 0, PDF_WINDOW).ok()?;
    let tail = if length > PDF_WINDOW {
        read_window(
            path,
            length.saturating_sub(PDF_WINDOW).max(PDF_WINDOW),
            PDF_WINDOW,
        )
        .ok()?
    } else {
        Vec::new()
    };
    [&tail, &head]
        .iter()
        .find_map(|window| root_page_count(window))
}

fn root_page_count(window: &[u8]) -> Option<u32> {
    let mut candidates = 0;
    for source in sources_to_scan(window) {
        let source = source.as_slice();
        let mut from = 0;
        while let Some(at) = find(&source[from..], PAGE_COUNT).map(|offset| from + offset) {
            from = at + PAGE_COUNT.len();
            candidates += 1;
            if candidates > MOST_BOX_CANDIDATES {
                return None;
            }
            let Some(dictionary) = enclosing_dictionary(source, at) else {
                continue;
            };
            if is_pages_root(dictionary)
                && let Some(count) = integer_after(&source[from..])
            {
                return Some(count);
            }
        }
    }
    None
}

fn is_pages_root(dictionary: &[u8]) -> bool {
    find(dictionary, b"/Parent").is_none() && is_pages_dictionary(dictionary)
}

fn is_pages_dictionary(dictionary: &[u8]) -> bool {
    let mut from = 0;
    while let Some(at) = find(&dictionary[from..], b"/Type").map(|offset| from + offset) {
        from = at + b"/Type".len();
        let rest = dictionary[from..]
            .iter()
            .skip_while(|byte| byte.is_ascii_whitespace())
            .copied()
            .take(b"/Pages".len())
            .collect::<Vec<u8>>();
        if rest == b"/Pages" {
            return true;
        }
    }
    false
}

fn integer_after(bytes: &[u8]) -> Option<u32> {
    let digits: Vec<u8> = bytes
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .take_while(|byte| byte.is_ascii_digit())
        .copied()
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

fn first_page_size(window: &[u8]) -> Option<Dimensions> {
    let mut fallback = None;
    let mut candidates = 0;
    for source in sources_to_scan(window) {
        let source = source.as_slice();
        let mut from = 0;
        while let Some(at) = find(&source[from..], MEDIA_BOX).map(|offset| from + offset) {
            from = at + MEDIA_BOX.len();
            candidates += 1;
            if candidates > MOST_BOX_CANDIDATES {
                return fallback.and_then(|size| page_dimensions(size, false));
            }
            let Some(size) = box_after(&source[from..]) else {
                continue;
            };
            let dictionary = enclosing_dictionary(source, at);
            if dictionary.is_some_and(is_page_dictionary) {
                let rotated = dictionary.is_some_and(is_quarter_turn);
                return page_dimensions(size, rotated);
            }
            if fallback.is_none() {
                fallback = Some(size);
            }
        }
    }
    fallback.and_then(|size| page_dimensions(size, false))
}

fn sources_to_scan(window: &[u8]) -> Vec<Vec<u8>> {
    let mut sources = vec![window.to_vec()];
    let mut from = 0;
    let mut inflated = 0;
    let mut attempts = 0;
    let mut searched = 0;
    let mut failures = 0;
    while sources.len() <= MOST_OBJECT_STREAMS && attempts < MOST_STREAM_ATTEMPTS {
        let Some(at) = find(&window[from..], OBJECT_STREAM).map(|offset| from + offset) else {
            break;
        };
        from = at + OBJECT_STREAM.len();
        attempts += 1;
        let Some(data) = stream_data_after(window, at, &mut searched) else {
            if searched >= MOST_STREAM_SEARCH_BYTES {
                break;
            }
            continue;
        };
        let remaining = MOST_INFLATED_BYTES.saturating_sub(inflated);
        if remaining == 0 {
            break;
        }
        if let Some(text) = inflate(data, remaining) {
            inflated += text.len();
            sources.push(text);
        } else {
            failures += 1;
            if failures >= MOST_INFLATE_FAILURES {
                break;
            }
        }
    }
    sources
}

fn stream_data_after<'a>(window: &'a [u8], from: usize, searched: &mut usize) -> Option<&'a [u8]> {
    let dictionary = window.get(from..)?;
    let dictionary = &dictionary[..dictionary.len().min(MOST_DICTIONARY_BYTES)];
    let dictionary_end = find(dictionary, STREAM_KEYWORD)? + from;
    let mut start = dictionary_end + STREAM_KEYWORD.len();
    if window.get(start) == Some(&b'\r') {
        start += 1;
    }
    if window.get(start) == Some(&b'\n') {
        start += 1;
    }
    let body = window.get(start..)?;
    let body = &body[..body.len().min(MOST_STREAM_BYTES)];
    *searched += body.len();
    let end = find(body, END_STREAM_KEYWORD)? + start;
    window.get(start..end)
}

fn inflate(data: &[u8], limit: usize) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, limit).ok()
}

fn box_after(bytes: &[u8]) -> Option<[f64; 4]> {
    let skipped = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let rest = bytes.get(skipped..)?.strip_prefix(b"[")?;
    let end = rest.iter().position(|byte| *byte == b']')?;
    let numbers: Vec<f64> = rest[..end]
        .split(u8::is_ascii_whitespace)
        .filter(|part| !part.is_empty())
        .map(number)
        .collect::<Option<_>>()?;
    <[f64; 4]>::try_from(numbers).ok()
}

fn number(text: &[u8]) -> Option<f64> {
    let parsed: f64 = std::str::from_utf8(text).ok()?.parse().ok()?;
    parsed.is_finite().then_some(parsed)
}

fn enclosing_dictionary(source: &[u8], at: usize) -> Option<&[u8]> {
    let mut depth = 0_usize;
    let mut start = None;
    let mut position = at;
    let floor = at.saturating_sub(MOST_DICTIONARY_BYTES);
    while position > floor + 1 {
        position -= 1;
        match source.get(position..position + 2) {
            Some(b">>") => depth += 1,
            Some(b"<<") if depth == 0 => {
                start = Some(position);
                break;
            }
            Some(b"<<") => depth -= 1,
            _ => {}
        }
        if depth > MOST_DICTIONARY_DEPTH {
            return None;
        }
    }
    let start = start?;
    let mut level = 0_usize;
    let mut cursor = start;
    let ceiling = (start + MOST_DICTIONARY_BYTES).min(source.len());
    while cursor + 1 < ceiling {
        match &source[cursor..cursor + 2] {
            b"<<" => {
                level += 1;
                cursor += 2;
            }
            b">>" => {
                level = level.checked_sub(1)?;
                cursor += 2;
                if level == 0 {
                    return source.get(start..cursor);
                }
            }
            _ => cursor += 1,
        }
    }
    None
}

fn is_page_dictionary(dictionary: &[u8]) -> bool {
    let mut from = 0;
    while let Some(at) = find(&dictionary[from..], b"/Type").map(|offset| from + offset) {
        from = at + b"/Type".len();
        let rest = &dictionary[from..];
        let skipped = rest
            .iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count();
        if let Some(name) = rest
            .get(skipped..)
            .and_then(|rest| rest.strip_prefix(b"/Page"))
            && name
                .first()
                .is_none_or(|next| !next.is_ascii_alphanumeric())
        {
            return true;
        }
    }
    false
}

fn is_quarter_turn(dictionary: &[u8]) -> bool {
    let Some(at) = find(dictionary, ROTATE) else {
        return false;
    };
    let rest = &dictionary[at + ROTATE.len()..];
    let digits: Vec<u8> = rest
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .take_while(|byte| byte.is_ascii_digit() || **byte == b'-')
        .copied()
        .collect();
    let turns = std::str::from_utf8(&digits)
        .ok()
        .and_then(|text| text.parse::<i64>().ok());
    turns.is_some_and(|degrees| degrees.rem_euclid(180) == 90)
}

fn page_dimensions(size: [f64; 4], rotated: bool) -> Option<Dimensions> {
    let (width, height) = side_lengths(size)?;
    let (width, height) = if rotated {
        (height, width)
    } else {
        (width, height)
    };
    checked_dimensions(width, height)
}

fn side_lengths(size: [f64; 4]) -> Option<(f64, f64)> {
    let width = (size[2] - size[0]).abs();
    let height = (size[3] - size[1]).abs();
    (width >= 1.0 && height >= 1.0).then_some((width.ceil(), height.ceil()))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn checked_dimensions(width: f64, height: f64) -> Option<Dimensions> {
    let limit = f64::from(LARGEST_SIDE);
    if width > limit || height > limit {
        return None;
    }
    Dimensions::new(width as u32, height as u32)
}

fn postscript_dimensions(path: &Path) -> Option<Dimensions> {
    let head = read_window(path, 0, POSTSCRIPT_WINDOW).ok()?;
    let (start, length) = if head.starts_with(&DOS_EPS_SIGNATURE) {
        dos_postscript_range(&head)?
    } else {
        (0, usize::MAX)
    };
    let start_at = u64::try_from(start).ok()?;
    let span = u64::try_from(length).unwrap_or(u64::MAX);
    let front = read_window(path, start_at, POSTSCRIPT_WINDOW.min(span)).ok()?;
    let mut candidates = vec![front];
    if candidates[0]
        .windows(DEFERRED_BOX.len())
        .any(|part| part == DEFERRED_BOX)
    {
        let total = file_length(path).ok()?;
        let end = start_at.saturating_add(span).min(total);
        let back_start = end.saturating_sub(POSTSCRIPT_WINDOW).max(start_at);
        candidates.push(read_window(path, back_start, end - back_start).ok()?);
    }
    candidates
        .iter()
        .find_map(|bytes| bounding_box(bytes))
        .and_then(|size| {
            let width = (size[2].ceil() - size[0].floor()).max(0.0);
            let height = (size[3].ceil() - size[1].floor()).max(0.0);
            (width >= 1.0 && height >= 1.0).then_some((width, height))
        })
        .and_then(|(width, height)| checked_dimensions(width, height))
}

fn bounding_box(bytes: &[u8]) -> Option<[f64; 4]> {
    box_in_comment(bytes, HIRES_BOUNDING_BOX).or_else(|| box_in_comment(bytes, BOUNDING_BOX))
}

fn box_in_comment(bytes: &[u8], key: &[u8]) -> Option<[f64; 4]> {
    let mut from = 0;
    while let Some(at) = find(&bytes[from..], key).map(|offset| from + offset) {
        from = at + key.len();
        let line = bytes[from..]
            .split(|byte| LINE_END.contains(byte))
            .next()
            .unwrap_or_default();
        let numbers: Option<Vec<f64>> = line
            .split(u8::is_ascii_whitespace)
            .filter(|part| !part.is_empty())
            .map(number)
            .collect();
        if let Some(numbers) = numbers
            && let Ok(size) = <[f64; 4]>::try_from(numbers)
        {
            return Some(size);
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let (first, rest) = needle.split_first()?;
    let mut from = 0;
    while let Some(offset) = haystack.get(from..)?.iter().position(|byte| byte == first) {
        let at = from + offset;
        if haystack.get(at + 1..at + needle.len()) == Some(rest) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{detect, dimensions_of, inspect, page_count};
    use crate::media::{AssetFormat, Dimensions};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn dimensions(width: u32, height: u32) -> Option<Dimensions> {
        Dimensions::new(width, height)
    }

    fn written(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        (directory, path)
    }

    fn pdf_with_page(page: &str) -> Vec<u8> {
        format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n{page}\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"
        )
        .into_bytes()
    }

    fn facts(name: &str, bytes: &[u8]) -> (AssetFormat, Option<Dimensions>) {
        let (_directory, path) = written(name, bytes);
        let detected = detect(bytes).expect("recognised");
        let info = inspect(&path, detected).unwrap();
        (info.format, info.dimensions)
    }

    #[test]
    fn real_pdf_files_report_their_first_page_size() {
        let cases = [
            ("pages-cairo.pdf", dimensions(842, 595)),
            ("pages-small.pdf", dimensions(64, 64)),
            ("pages-cups.pdf", dimensions(596, 842)),
            ("pages-huge.pdf", dimensions(14400, 14400)),
        ];
        for (name, expected) in cases {
            let path = fixture(name);
            let info = inspect(&path, AssetFormat::Pdf).unwrap();
            assert_eq!(info.format, AssetFormat::Pdf, "{name}");
            assert_eq!(info.dimensions, expected, "{name}");
            assert_eq!(dimensions_of(&path, AssetFormat::Pdf), expected, "{name}");
        }
    }

    #[test]
    fn encapsulated_postscript_reports_its_bounding_box() {
        let plain = inspect(&fixture("pages-small.eps"), AssetFormat::Eps).unwrap();
        assert_eq!(
            (plain.format, plain.dimensions),
            (AssetFormat::Eps, dimensions(64, 40))
        );
        let dos = inspect(&fixture("pages-dos.eps"), AssetFormat::Eps).unwrap();
        assert_eq!(
            (dos.format, dos.dimensions),
            (AssetFormat::Eps, dimensions(64, 40))
        );
    }

    #[test]
    fn an_old_illustrator_file_is_postscript_made_by_illustrator() {
        let bytes = std::fs::read(fixture("pages-old-illustrator.ai")).unwrap();
        assert_eq!(detect(&bytes), Some(AssetFormat::Ai));
        let info = inspect(&fixture("pages-old-illustrator.ai"), AssetFormat::Ai).unwrap();
        assert_eq!(
            (info.format, info.dimensions),
            (AssetFormat::Ai, dimensions(320, 200))
        );
    }

    #[test]
    fn recognises_the_three_families_by_their_content() {
        assert_eq!(detect(b"%PDF-1.7\n"), Some(AssetFormat::Pdf));
        assert_eq!(detect(b"junk\n\n%PDF-2.0\n"), Some(AssetFormat::Pdf));
        assert_eq!(
            detect(b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 1 1\n"),
            Some(AssetFormat::Eps)
        );
        assert_eq!(
            detect(b"%!PS-Adobe-2.0 EPSF-1.2\r%%BoundingBox: 0 0 1 1\r"),
            Some(AssetFormat::Eps)
        );
        assert_eq!(
            detect(b"%!PS-Adobe-3.0 EPSF-3.0\n%%Creator: Adobe Illustrator(R) 9.0\n"),
            Some(AssetFormat::Ai)
        );
    }

    #[test]
    fn things_that_only_look_alike_are_not_recognised() {
        assert_eq!(detect(b"%PDF-"), None);
        assert_eq!(detect(b"%PDF-x"), None);
        assert_eq!(detect(b"%!PS-Adobe-3.0\n%%Pages: 3\n"), None);
        assert_eq!(detect(b"%!PS\n1 2 add\n"), None);
        assert_eq!(detect(b"PDF is a format"), None);
        assert_eq!(detect(b"\xC5\xD0\xD3\xC6"), None);
        assert_eq!(detect(b""), None);
        let late = [vec![b' '; 2000], b"%PDF-1.4".to_vec()].concat();
        assert_eq!(detect(&late), None);
    }

    #[test]
    fn a_rotated_page_swaps_its_sides() {
        let bytes =
            pdf_with_page("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Rotate 90 >>");
        assert_eq!(
            facts("a.pdf", &bytes),
            (AssetFormat::Pdf, dimensions(200, 300))
        );
        let turned = pdf_with_page("<< /Type /Page /MediaBox [0 0 300 200] /Rotate -270 >>");
        assert_eq!(facts("b.pdf", &turned).1, dimensions(200, 300));
        let half = pdf_with_page("<< /Type /Page /MediaBox [0 0 300 200] /Rotate 180 >>");
        assert_eq!(facts("c.pdf", &half).1, dimensions(300, 200));
    }

    #[test]
    fn a_page_without_its_own_box_inherits_the_one_of_its_parent() {
        let bytes = b"%PDF-1.4\n2 0 obj\n<< /Type /Pages /MediaBox [0 0 612 792] /Kids [3 0 R] >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n";
        assert_eq!(facts("inherit.pdf", bytes).1, dimensions(612, 792));
    }

    #[test]
    fn the_box_may_start_away_from_the_origin_and_use_decimals() {
        let bytes = pdf_with_page("<< /Type /Page /MediaBox [-10.5 -20.25 289.75 179.5] >>");
        assert_eq!(facts("shifted.pdf", &bytes).1, dimensions(301, 200));
        let reversed = pdf_with_page("<< /Type /Page /MediaBox [300 200 0 0] >>");
        assert_eq!(facts("reversed.pdf", &reversed).1, dimensions(300, 200));
    }

    #[test]
    fn the_first_page_wins_over_later_ones() {
        let bytes = b"%PDF-1.4\n3 0 obj\n<< /Type /Page /MediaBox [0 0 100 50] >>\nendobj\n4 0 obj\n<< /Type /Page /MediaBox [0 0 900 900] >>\nendobj\n";
        assert_eq!(facts("two.pdf", bytes).1, dimensions(100, 50));
    }

    #[test]
    fn a_page_inside_a_nested_dictionary_is_still_found() {
        let bytes = pdf_with_page(
            "<< /Resources << /Font << /F1 << /Type /Font >> >> >> /Type /Page /MediaBox [0 0 420 297] /Contents 5 0 R >>",
        );
        assert_eq!(facts("nested.pdf", &bytes).1, dimensions(420, 297));
    }

    #[test]
    fn a_page_stored_inside_a_compressed_object_stream_is_found() {
        let objects = b"3 0 << /Type /Page /Parent 2 0 R /MediaBox [0 0 250 125] >>";
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(objects, 6);
        let mut bytes =
            b"%PDF-1.5\n5 0 obj\n<< /Type /ObjStm /N 1 /First 4 /Filter /FlateDecode >>\nstream\n"
                .to_vec();
        bytes.extend_from_slice(&packed);
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        assert_eq!(facts("compressed.pdf", &bytes).1, dimensions(250, 125));
    }

    #[test]
    fn an_illustrator_file_is_a_pdf_with_illustrator_private_data() {
        let plain = pdf_with_page("<< /Type /Page /MediaBox [0 0 100 100] >>");
        assert_eq!(facts("plain.pdf", &plain).0, AssetFormat::Pdf);
        let mut illustrator = plain.clone();
        illustrator.extend_from_slice(
            b"1 0 obj\n<< /PieceInfo << /Illustrator << /Private 9 0 R >> >> >>\nendobj\n",
        );
        assert_eq!(
            facts("drawing.pdf", &illustrator),
            (AssetFormat::Ai, dimensions(100, 100))
        );
        assert_eq!(facts("drawing.ai", &plain).0, AssetFormat::Pdf);
    }

    #[test]
    fn a_pdf_named_like_an_illustrator_file_stays_what_its_content_says() {
        let plain = pdf_with_page("<< /Type /Page /MediaBox [0 0 100 100] >>");
        let (_directory, path) = written("export.ai", &plain);
        let info = inspect(&path, AssetFormat::Ai).unwrap();
        assert_eq!(info.format, AssetFormat::Ai);
        let info = inspect(&path, AssetFormat::Pdf).unwrap();
        assert_eq!(info.format, AssetFormat::Pdf);
    }

    fn finishes_quickly(bytes: &[u8]) {
        let (_directory, path) = written("hostile.pdf", bytes);
        let started = std::time::Instant::now();
        let _ = inspect(&path, AssetFormat::Pdf);
        let _ = page_count(&path);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn endless_object_streams_without_an_end_do_not_stall_the_scan() {
        let mut bytes = b"%PDF-1.5\n".to_vec();
        while bytes.len() < 8 * 1024 * 1024 {
            bytes.extend_from_slice(b"/ObjStm stream\n");
        }
        finishes_quickly(&bytes);
    }

    #[test]
    fn a_flood_of_unreadable_object_streams_does_not_stall_the_scan() {
        let mut bytes = b"%PDF-1.5\n".to_vec();
        while bytes.len() < 8 * 1024 * 1024 {
            bytes.extend_from_slice(b"/ObjStm stream\nnot zlib data endstream\n");
        }
        finishes_quickly(&bytes);
    }

    #[test]
    fn a_flood_of_boxes_and_counts_does_not_stall_the_scan() {
        let mut bytes = b"%PDF-1.5\n".to_vec();
        while bytes.len() < 8 * 1024 * 1024 {
            bytes.extend_from_slice(b"/MediaBox [0 0 1 1] /Count 3 ");
        }
        finishes_quickly(&bytes);
    }

    #[test]
    fn text_quoting_a_pdf_header_in_the_middle_of_a_line_is_not_a_pdf() {
        assert_eq!(detect(b"see the %PDF-1.4 header"), None);
        assert_eq!(detect(b"notes\n%PDF-1.4\n"), Some(AssetFormat::Pdf));
    }

    #[test]
    fn one_unreadable_page_count_does_not_hide_the_real_one() {
        let mut bytes = b"%PDF-1.4\n<< /Count 7 \n".to_vec();
        bytes.extend_from_slice(
            &pdf_with_page("<< /Type /Page /MediaBox [0 0 10 10] >>")["%PDF-1.4\n".len()..],
        );
        let (_directory, path) = written("odd.pdf", &bytes);
        assert_eq!(page_count(&path), Some(1));
    }

    #[test]
    fn a_bounding_box_declared_at_the_end_is_found() {
        let bytes = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: (atend)\n%%EndComments\nshowpage\n%%Trailer\n%%BoundingBox: 5 5 205 105\n%%EOF\n";
        assert_eq!(
            facts("late.eps", bytes),
            (AssetFormat::Eps, dimensions(200, 100))
        );
    }

    #[test]
    fn the_precise_box_is_rounded_outwards() {
        let bytes = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 100 50\n%%HiResBoundingBox: 0.4 0.4 99.2 49.1\n";
        assert_eq!(facts("hires.eps", bytes).1, dimensions(100, 50));
    }

    #[test]
    fn an_empty_or_missing_box_gives_no_size_but_keeps_the_format() {
        for (name, bytes) in [
            (
                "empty.eps",
                b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 0 0\n".as_slice(),
            ),
            (
                "none.eps",
                b"%!PS-Adobe-3.0 EPSF-3.0\nshowpage\n".as_slice(),
            ),
            (
                "words.eps",
                b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: a b c d\n".as_slice(),
            ),
            (
                "negative.eps",
                b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 10 10 5 5\n".as_slice(),
            ),
        ] {
            assert_eq!(facts(name, bytes), (AssetFormat::Eps, None), "{name}");
        }
    }

    #[test]
    fn damaged_and_cut_files_never_panic() {
        for name in [
            "pages-cairo.pdf",
            "pages-small.eps",
            "pages-dos.eps",
            "pages-old-illustrator.ai",
        ] {
            let bytes = std::fs::read(fixture(name)).unwrap();
            for length in (0..bytes.len()).step_by(11) {
                let (_directory, path) = written(name, &bytes[..length]);
                if let Some(format) = detect(&bytes[..length]) {
                    let _ = inspect(&path, format);
                }
            }
            for position in (0..bytes.len()).step_by(7) {
                let mut damaged = bytes.clone();
                damaged[position] ^= 0xFF;
                let (_directory, path) = written(name, &damaged);
                if let Some(format) = detect(&damaged) {
                    let _ = inspect(&path, format);
                }
            }
        }
    }

    #[test]
    fn a_swollen_object_stream_cannot_exhaust_memory() {
        let bomb = miniz_oxide::deflate::compress_to_vec_zlib(&vec![b' '; 64 * 1024 * 1024], 9);
        let mut bytes = b"%PDF-1.5\n5 0 obj\n<< /Type /ObjStm >>\nstream\n".to_vec();
        bytes.extend_from_slice(&bomb);
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        assert_eq!(facts("bomb.pdf", &bytes), (AssetFormat::Pdf, None));
    }

    #[test]
    fn counts_the_pages_of_real_pdf_files() {
        for (name, expected) in [
            ("pages-cairo.pdf", Some(2)),
            ("pages-small.pdf", Some(1)),
            ("pages-cups.pdf", Some(1)),
            ("pages-locked.pdf", Some(1)),
            ("pages-small.eps", None),
            ("pages-old-illustrator.ai", None),
        ] {
            assert_eq!(page_count(&fixture(name)), expected, "{name}");
        }
    }

    #[test]
    fn counts_the_pages_of_the_root_of_a_nested_tree() {
        let bytes = b"%PDF-1.4\n1 0 obj\n<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 5 >>\nendobj\n2 0 obj\n<< /Type /Pages /Parent 1 0 R /Kids [4 0 R] /Count 3 >>\nendobj\n3 0 obj\n<< /Type /Pages /Parent 1 0 R /Count 2 >>\nendobj\n";
        let (_directory, path) = written("tree.pdf", bytes);
        assert_eq!(page_count(&path), Some(5));
        let reversed = b"%PDF-1.4\n2 0 obj\n<< /Type /Pages /Parent 1 0 R /Count 3 >>\nendobj\n1 0 obj\n<< /Kids [2 0 R] /Type /Pages /Count 3 >>\nendobj\n";
        let (_other, path) = written("reversed.pdf", reversed);
        assert_eq!(page_count(&path), Some(3));
    }

    #[test]
    fn counts_the_pages_of_a_tree_stored_in_an_object_stream() {
        let objects = b"1 0 << /Type /Pages /Kids [2 0 R] /Count 12 >>";
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(objects, 6);
        let mut bytes = b"%PDF-1.5\n5 0 obj\n<< /Type /ObjStm /N 1 >>\nstream\n".to_vec();
        bytes.extend_from_slice(&packed);
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        let (_directory, path) = written("packed.pdf", &bytes);
        assert_eq!(page_count(&path), Some(12));
    }

    #[test]
    fn no_count_is_given_when_the_file_does_not_say() {
        let (_directory, path) = written("none.pdf", b"%PDF-1.4\nnothing useful\n");
        assert_eq!(page_count(&path), None);
        let (_other, path) = written("words.pdf", b"%PDF-1.4\n<< /Type /Pages /Count many >>\n");
        assert_eq!(page_count(&path), None);
        assert_eq!(page_count(Path::new("/nonexistent/none.pdf")), None);
    }
}
