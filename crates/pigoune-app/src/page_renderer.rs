use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, Once, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pigoune_core::{AssetFormat, Dimensions, PageFile};

const PROGRAM: &str = "gs";
const TIME_LIMIT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(15);
const STOP_WAIT: Duration = Duration::from_secs(2);
const MOST_AT_ONCE: usize = 2;
const MOST_MESSAGE_BYTES: u64 = 256 * 1024;
const MOST_PNG_BYTES: u64 = 256 * 1024 * 1024;
const LARGEST_RENDER_SIDE: u32 = 4096;
const POINTS_PER_INCH: f64 = 72.0;
const SMALLEST_RESOLUTION: f64 = 0.01;
const LEFTOVER_PREFIX: &str = "pigoune-page-";
const LEFTOVER_AGE: Duration = Duration::from_secs(600);
const FREE_NAME_ATTEMPTS: usize = 16;
const PASSWORD_MESSAGE: &str = "requires a password";
const MEDIA_BOX_MARK: &str = "Page 1 MediaBox:";
const PRECISE_BOX_MARK: &str = "%%HiResBoundingBox:";
const BOX_MARK: &str = "%%BoundingBox:";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const INHERITED_VARIABLES: [&str; 3] = ["GS_OPTIONS", "GS_LIB", "GS_DEVICE"];

static RENDER_COUNTER: AtomicU64 = AtomicU64::new(0);
static STOPPING: AtomicBool = AtomicBool::new(false);
static RUNNING: Mutex<usize> = Mutex::new(0);
static FREED: Condvar = Condvar::new();
static CLEANED: Once = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageProblem {
    Protected,
    Damaged,
    Unavailable,
}

struct Outcome {
    output: Vec<u8>,
    errors: Vec<u8>,
    succeeded: bool,
    mentions_password: bool,
}

struct Slot;

impl Slot {
    fn take() -> Option<Self> {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if STOPPING.load(Ordering::SeqCst) {
                return None;
            }
            if *running < MOST_AT_ONCE {
                *running += 1;
                return Some(Self);
            }
            running = FREED
                .wait_timeout(running, POLL_INTERVAL)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        *running = running.saturating_sub(1);
        FREED.notify_all();
    }
}

pub fn stop_all() {
    STOPPING.store(true, Ordering::SeqCst);
    let started = Instant::now();
    let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    while *running > 0 && started.elapsed() < STOP_WAIT {
        running = FREED
            .wait_timeout(running, POLL_INTERVAL)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
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
    let output = temporary_file().ok_or(PageProblem::Unavailable)?;
    let arguments = render_arguments(path, file, size, target, &output);
    let outcome = run(&arguments, MOST_MESSAGE_BYTES);
    let png = read_png(&output);
    let _ = std::fs::remove_file(&output);
    let outcome = outcome?;
    if outcome.mentions_password {
        return Err(PageProblem::Protected);
    }
    eprintln!(
        "DEBUG ok={} png={} err={} out={}",
        outcome.succeeded,
        png.len(),
        String::from_utf8_lossy(&outcome.errors),
        String::from_utf8_lossy(&outcome.output)
    );
    if !outcome.succeeded || !png.starts_with(PNG_SIGNATURE) {
        return Err(PageProblem::Damaged);
    }
    Ok(png)
}

fn read_png(path: &Path) -> Vec<u8> {
    let small_enough = std::fs::metadata(path).is_ok_and(|data| data.len() <= MOST_PNG_BYTES);
    if small_enough {
        std::fs::read(path).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn scratch_directory() -> Option<PathBuf> {
    [
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        Some(std::env::temp_dir()),
    ]
    .into_iter()
    .flatten()
    .find(|directory| directory.to_str().is_some() && directory.is_dir())
}

fn temporary_file() -> Option<PathBuf> {
    let directory = scratch_directory()?;
    CLEANED.call_once(|| remove_leftovers(&directory));
    (0..FREE_NAME_ATTEMPTS).find_map(|_| {
        let number = RENDER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.subsec_nanos());
        let path = directory.join(format!(
            "{LEFTOVER_PREFIX}{}-{number}-{nanos}.png",
            std::process::id()
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .is_ok()
            .then_some(path)
    })
}

fn remove_leftovers(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_ours = name.starts_with(LEFTOVER_PREFIX) && name.ends_with(".png");
        let is_old = entry
            .metadata()
            .and_then(|data| data.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > LEFTOVER_AGE);
        if is_ours && is_old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn common_arguments() -> Vec<OsString> {
    ["-q", "-dSAFER", "-dBATCH", "-dNOPAUSE"]
        .map(OsString::from)
        .to_vec()
}

fn path_argument(path: &Path) -> OsString {
    if path.is_absolute() {
        path.into()
    } else {
        Path::new(".").join(path).into_os_string()
    }
}

fn info_arguments(path: &Path) -> Vec<OsString> {
    let mut arguments = common_arguments();
    arguments.extend(["-dNODISPLAY", "-dPDFINFO"].map(OsString::from));
    arguments.push(path_argument(path));
    arguments
}

fn bounding_arguments(path: &Path) -> Vec<OsString> {
    let mut arguments = common_arguments();
    arguments.push("-sDEVICE=bbox".into());
    arguments.push(path_argument(path));
    arguments
}

fn render_arguments(
    path: &Path,
    file: &PageFile,
    size: Dimensions,
    target: u32,
    output: &Path,
) -> Vec<OsString> {
    let mut arguments = common_arguments();
    let device = if file.format == AssetFormat::Pdf {
        "png16m"
    } else {
        "pngalpha"
    };
    arguments.push(format!("-sDEVICE={device}").into());
    arguments.extend(["-dTextAlphaBits=4", "-dGraphicsAlphaBits=4"].map(OsString::from));
    let (width, height) = fitted(size, target);
    if file.is_pdf {
        arguments.extend(
            [
                "-dFirstPage=1".to_string(),
                "-dLastPage=1".to_string(),
                "-dPDFFitPage".to_string(),
                format!("-g{width}x{height}"),
            ]
            .map(OsString::from),
        );
    } else {
        arguments.push("-dEPSCrop".into());
        arguments.push(format!("-r{}", resolution(size, target)).into());
    }
    let escaped = output.to_string_lossy().replace('%', "%%");
    arguments.push(format!("-sOutputFile={escaped}").into());
    arguments.push(path_argument(path));
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
    let dpi = (POINTS_PER_INCH * f64::from(target) / longest).max(SMALLEST_RESOLUTION);
    format!("{dpi:.4}")
}

fn measured_size(path: &Path, file: &PageFile) -> Result<Dimensions, PageProblem> {
    if !file.is_pdf {
        let outcome = run(&bounding_arguments(path), MOST_MESSAGE_BYTES)?;
        let text = String::from_utf8_lossy(&outcome.errors);
        return bounding_box_size(&text).ok_or(PageProblem::Damaged);
    }
    let outcome = run(&info_arguments(path), MOST_MESSAGE_BYTES)?;
    if outcome.mentions_password {
        return Err(PageProblem::Protected);
    }
    let text = String::from_utf8_lossy(&outcome.output);
    media_box_size(&text).ok_or(PageProblem::Damaged)
}

fn numbers_between(line: &str, start: usize, end: usize) -> Option<[f64; 4]> {
    let numbers: Vec<f64> = line
        .get(start..end)?
        .split_whitespace()
        .filter_map(|part| part.parse().ok())
        .collect();
    <[f64; 4]>::try_from(numbers).ok()
}

fn media_box_size(text: &str) -> Option<Dimensions> {
    let line = text.lines().find(|line| line.contains(MEDIA_BOX_MARK))?;
    let start = line.find('[')? + 1;
    let end = line.find(']')?;
    let [left, bottom, right, top] = numbers_between(line, start, end)?;
    pixel_size(span(left, right), span(bottom, top))
}

fn bounding_box_size(text: &str) -> Option<Dimensions> {
    [PRECISE_BOX_MARK, BOX_MARK].iter().find_map(|mark| {
        let line = text.lines().find(|line| line.starts_with(mark))?;
        let [left, bottom, right, top] = numbers_between(line, mark.len(), line.len())?;
        let has_area = (right - left).abs() >= 1.0 && (top - bottom).abs() >= 1.0;
        has_area.then_some(())?;
        pixel_size(span(left, right), span(bottom, top))
    })
}

fn span(low: f64, high: f64) -> f64 {
    (high - low).abs().ceil().clamp(1.0, f64::from(u16::MAX))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_size(width: f64, height: f64) -> Option<Dimensions> {
    Dimensions::new(width as u32, height as u32)
}

fn run(arguments: &[OsString], limit: u64) -> Result<Outcome, PageProblem> {
    let _slot = Slot::take().ok_or(PageProblem::Unavailable)?;
    run_program(PROGRAM, arguments, limit, TIME_LIMIT)
}

fn run_program(
    program: &str,
    arguments: &[OsString],
    limit: u64,
    time_limit: Duration,
) -> Result<Outcome, PageProblem> {
    let mut command = Command::new(program);
    for variable in INHERITED_VARIABLES {
        command.env_remove(variable);
    }
    let mut child = command
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
            Ok(None) if started.elapsed() < time_limit && !STOPPING.load(Ordering::SeqCst) => {
                thread::sleep(POLL_INTERVAL);
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let output = reader.join().unwrap_or_default();
    let errors = error_reader.join().unwrap_or_default();
    let mentions_password = [&output, &errors]
        .iter()
        .any(|text| String::from_utf8_lossy(text).contains(PASSWORD_MESSAGE));
    Ok(Outcome {
        succeeded: status.is_some_and(|status| status.success()),
        output,
        errors,
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

    use super::{
        PageProblem, bounding_box_size, fitted, media_box_size, path_argument, render_first_page,
        temporary_file,
    };

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
    fn reads_the_precise_bounding_box_from_the_messages_of_ghostscript() {
        let text =
            "%%BoundingBox: 0 0 65 41\n%%HiResBoundingBox: 0.000000 0.000000 64.007998 40.013999\n";
        let size = bounding_box_size(text).expect("a size");
        assert_eq!((size.width(), size.height()), (65, 41));
        let text = "%%BoundingBox: 10 10 110 60\n";
        let size = bounding_box_size(text).expect("a size");
        assert_eq!((size.width(), size.height()), (100, 50));
        assert!(bounding_box_size("%%BoundingBox: 0 0 0 0\n").is_none());
        assert!(bounding_box_size("nothing").is_none());
    }

    #[test]
    fn a_relative_file_name_cannot_pass_for_an_option() {
        assert_eq!(path_argument(Path::new("-dNOSAFER.pdf")), "./-dNOSAFER.pdf");
        assert_eq!(path_argument(Path::new("@list")), "./@list");
        assert_eq!(path_argument(Path::new("/tmp/a.pdf")), "/tmp/a.pdf");
    }

    #[test]
    fn every_temporary_file_is_new_and_ours() {
        let first = temporary_file().expect("a file");
        let second = temporary_file().expect("a file");
        assert_ne!(first, second);
        assert!(first.exists() && second.exists());
        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
    }

    #[test]
    fn an_encapsulated_file_without_a_declared_box_is_measured_by_ghostscript() {
        if !ghostscript_is_installed() {
            return;
        }
        let path =
            std::env::temp_dir().join(format!("pigoune-undeclared-box-{}.eps", std::process::id()));
        std::fs::write(
            &path,
            "%!PS-Adobe-3.0 EPSF-3.0\n0 0 moveto 100 0 lineto 100 50 lineto 0 50 lineto fill\nshowpage\n",
        )
        .unwrap();
        let file = page_file(&path).expect("a page file");
        let png = render_first_page(&path, &file, 128);
        let _ = std::fs::remove_file(&path);
        let (width, height) = png_size(&png.expect("the page is drawn"));
        assert!(width >= 120 && height >= 60, "{width}x{height}");
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
            &[std::ffi::OsString::from("30")],
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
