use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use pigoune_core::{AssetFormat, Dimensions, PageFile};

const PROGRAM: &str = "gs";
const TIME_LIMIT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(15);
const MOST_MESSAGE_BYTES: u64 = 256 * 1024;
const LARGEST_RENDER_SIDE: u32 = 4096;
const POINTS_PER_INCH: f64 = 72.0;
const PASSWORD_MESSAGE: &str = "requires a password";
const MEDIA_BOX_MARK: &str = "Page 1 MediaBox:";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

static RENDER_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageProblem {
    Protected,
    Damaged,
    Unavailable,
}

struct Outcome {
    output: Vec<u8>,
    succeeded: bool,
    mentions_password: bool,
}

pub fn render_first_page(
    path: &Path,
    file: &PageFile,
    longest_side: u32,
) -> Result<Vec<u8>, PageProblem> {
    let target = longest_side.clamp(1, LARGEST_RENDER_SIDE);
    let size = match file.dimensions {
        Some(size) => size,
        None => measured_size(path, file)?,
    };
    let output = temporary_file();
    let arguments = render_arguments(path, file, size, target, &output);
    let outcome = run(&arguments, MOST_MESSAGE_BYTES);
    let png = std::fs::read(&output).unwrap_or_default();
    let _ = std::fs::remove_file(&output);
    let outcome = outcome?;
    if outcome.mentions_password {
        return Err(PageProblem::Protected);
    }
    if !outcome.succeeded || !png.starts_with(PNG_SIGNATURE) {
        return Err(PageProblem::Damaged);
    }
    Ok(png)
}

fn temporary_file() -> PathBuf {
    let number = RENDER_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pigoune-page-{}-{number}.png", std::process::id()))
}

fn common_arguments() -> Vec<String> {
    ["-q", "-dSAFER", "-dBATCH", "-dNOPAUSE"]
        .map(str::to_string)
        .to_vec()
}

fn info_arguments(path: &Path) -> Vec<String> {
    let mut arguments = common_arguments();
    arguments.extend(["-dNODISPLAY", "-dPDFINFO"].map(str::to_string));
    arguments.push(path.display().to_string());
    arguments
}

fn render_arguments(
    path: &Path,
    file: &PageFile,
    size: Dimensions,
    target: u32,
    output: &Path,
) -> Vec<String> {
    let mut arguments = common_arguments();
    let device = if file.format == AssetFormat::Pdf {
        "png16m"
    } else {
        "pngalpha"
    };
    arguments.push(format!("-sDEVICE={device}"));
    arguments.extend(["-dTextAlphaBits=4", "-dGraphicsAlphaBits=4"].map(str::to_string));
    let (width, height) = fitted(size, target);
    if file.is_pdf {
        arguments.extend([
            "-dFirstPage=1".to_string(),
            "-dLastPage=1".to_string(),
            "-dPDFFitPage".to_string(),
            format!("-g{width}x{height}"),
        ]);
    } else {
        arguments.push("-dEPSCrop".to_string());
        arguments.push(format!("-r{}", resolution(size, target)));
    }
    arguments.push(format!("-sOutputFile={}", output.display()));
    arguments.push(path.display().to_string());
    arguments
}

fn fitted(size: Dimensions, target: u32) -> (u32, u32) {
    let longest = size.width().max(size.height()).max(1);
    let scaled = |value: u32| {
        let product = u64::from(value) * u64::from(target);
        u32::try_from((product + u64::from(longest) / 2) / u64::from(longest))
            .unwrap_or(target)
            .max(1)
    };
    (scaled(size.width()), scaled(size.height()))
}

fn resolution(size: Dimensions, target: u32) -> String {
    let longest = f64::from(size.width().max(size.height()).max(1));
    format!("{:.4}", POINTS_PER_INCH * f64::from(target) / longest)
}

fn measured_size(path: &Path, file: &PageFile) -> Result<Dimensions, PageProblem> {
    if !file.is_pdf {
        return Err(PageProblem::Damaged);
    }
    let outcome = run(&info_arguments(path), MOST_MESSAGE_BYTES)?;
    if outcome.mentions_password {
        return Err(PageProblem::Protected);
    }
    let text = String::from_utf8_lossy(&outcome.output);
    media_box_size(&text).ok_or(PageProblem::Damaged)
}

fn media_box_size(text: &str) -> Option<Dimensions> {
    let line = text.lines().find(|line| line.contains(MEDIA_BOX_MARK))?;
    let start = line.find('[')? + 1;
    let end = line.find(']')?;
    let numbers: Vec<f64> = line
        .get(start..end)?
        .split_whitespace()
        .filter_map(|part| part.parse().ok())
        .collect();
    let [left, bottom, right, top] = <[f64; 4]>::try_from(numbers).ok()?;
    let span = |low: f64, high: f64| ((high - low).abs().ceil()).clamp(1.0, f64::from(u16::MAX));
    pixel_size(span(left, right), span(bottom, top))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_size(width: f64, height: f64) -> Option<Dimensions> {
    Dimensions::new(width as u32, height as u32)
}

fn run(arguments: &[String], limit: u64) -> Result<Outcome, PageProblem> {
    run_program(PROGRAM, arguments, limit, TIME_LIMIT)
}

fn run_program(
    program: &str,
    arguments: &[String],
    limit: u64,
    time_limit: Duration,
) -> Result<Outcome, PageProblem> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| PageProblem::Unavailable)?;
    let output = child.stdout.take().ok_or(PageProblem::Unavailable)?;
    let errors = child.stderr.take().ok_or(PageProblem::Unavailable)?;
    let reader = thread::spawn(move || read_up_to(output, limit));
    let error_reader = thread::spawn(move || read_up_to(errors, MOST_MESSAGE_BYTES));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < time_limit => thread::sleep(POLL_INTERVAL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let output = reader.join().unwrap_or_default();
    let errors = error_reader.join().unwrap_or_default();
    let within_limit = u64::try_from(output.len()).is_ok_and(|length| length <= limit);
    let mentions_password = [&output, &errors]
        .iter()
        .any(|text| String::from_utf8_lossy(text).contains(PASSWORD_MESSAGE));
    Ok(Outcome {
        succeeded: status.is_some_and(|status| status.success()) && within_limit,
        output,
        mentions_password,
    })
}

fn read_up_to(mut stream: impl Read, limit: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = stream.by_ref().take(limit + 1).read_to_end(&mut bytes);
    let _ = std::io::copy(&mut stream, &mut std::io::sink());
    bytes
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use pigoune_core::page_file;

    use super::{PageProblem, fitted, media_box_size, render_first_page};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    fn ghostscript_is_installed() -> bool {
        std::process::Command::new("gs")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn png_size(png: &[u8]) -> (u32, u32) {
        let at = |start: usize| {
            u32::from_be_bytes([png[start], png[start + 1], png[start + 2], png[start + 3]])
        };
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        (at(16), at(20))
    }

    fn render(name: &str, target: u32) -> Result<Vec<u8>, PageProblem> {
        let path = fixture(name);
        let file = page_file(&path).expect("a page file");
        render_first_page(&path, &file, target)
    }

    #[test]
    fn fits_a_page_inside_the_requested_side() {
        let size = pigoune_core::Dimensions::new(842, 595).unwrap();
        assert_eq!(fitted(size, 2048), (2048, 1447));
        let size = pigoune_core::Dimensions::new(64, 64).unwrap();
        assert_eq!(fitted(size, 256), (256, 256));
        let size = pigoune_core::Dimensions::new(1, 4000).unwrap();
        assert_eq!(fitted(size, 100), (1, 100));
    }

    #[test]
    fn reads_the_first_media_box_from_the_information_text() {
        let text = "\n  File has 2 pages\n\nPage 1 MediaBox: [0 0 612 792]   Page uses transparency\nPage 2 MediaBox: [0 0 10 10]\n";
        let size = media_box_size(text).expect("a size");
        assert_eq!((size.width(), size.height()), (612, 792));
        assert!(media_box_size("nothing here").is_none());
        assert!(media_box_size("Page 1 MediaBox: [0 0 612]").is_none());
    }

    #[test]
    fn draws_a_pdf_page_at_the_requested_size() {
        if !ghostscript_is_installed() {
            return;
        }
        let png = render("pages-cairo.pdf", 400).expect("the page is drawn");
        assert_eq!(png_size(&png), (400, 283));
        let png = render("pages-small.pdf", 256).expect("the page is drawn");
        assert_eq!(png_size(&png), (256, 256));
    }

    #[test]
    fn draws_encapsulated_postscript_and_old_illustrator_files() {
        if !ghostscript_is_installed() {
            return;
        }
        for name in ["pages-small.eps", "pages-dos.eps"] {
            let png = render(name, 128).expect(name);
            let (width, height) = png_size(&png);
            assert!((127..=128).contains(&width), "{name}: {width}");
            assert!((79..=81).contains(&height), "{name}: {height}");
        }
    }

    #[test]
    fn a_damaged_or_empty_file_is_reported_as_damaged() {
        if !ghostscript_is_installed() {
            return;
        }
        assert_eq!(render("pages-damaged.pdf", 128), Err(PageProblem::Damaged));
    }

    #[test]
    fn a_password_protected_pdf_is_reported_as_protected() {
        if !ghostscript_is_installed() {
            return;
        }
        assert_eq!(render("pages-locked.pdf", 128), Err(PageProblem::Protected));
    }

    #[test]
    fn a_program_that_runs_too_long_is_stopped() {
        let started = std::time::Instant::now();
        let outcome = super::run_program(
            "sleep",
            &["30".to_string()],
            1024,
            std::time::Duration::from_millis(200),
        )
        .expect("the program starts");
        assert!(!outcome.succeeded);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn a_missing_program_is_reported_as_unavailable() {
        let outcome = super::run_program(
            "pigoune-no-such-program",
            &[],
            1024,
            std::time::Duration::from_secs(1),
        );
        assert!(matches!(outcome, Err(PageProblem::Unavailable)));
    }

    #[test]
    fn a_hostile_file_cannot_read_or_write_files() {
        if !ghostscript_is_installed() {
            return;
        }
        let target = PathBuf::from("/tmp/pigoune-hostile-eps-test");
        let _ = std::fs::remove_file(&target);
        let path = fixture("pages-hostile.eps");
        let file = page_file(&path).expect("a page file");
        assert_eq!(
            render_first_page(&path, &file, 128),
            Err(PageProblem::Damaged)
        );
        assert!(!target.exists());
    }
}
