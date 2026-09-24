use std::{error::Error, fmt, io};

use gio::ReadInputStream;
use glycin::{Error as GlycinError, ErrorCtx, Loader, SandboxMechanism};
use pigoune_core::{ImageFormat, ImageMetadata, StagedObject, StagedValidator};

#[derive(Debug)]
pub enum ImportValidationError {
    StagingAccess(io::Error),
    UnsupportedFormat(Box<ErrorCtx>),
    Load(Box<ErrorCtx>),
    Frame(Box<ErrorCtx>),
    InvalidDimensions { width: u32, height: u32 },
    MimeNotAccepted(String),
}

impl fmt::Display for ImportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StagingAccess(error) => write!(f, "cannot read staged image: {error}"),
            Self::UnsupportedFormat(error) => write!(f, "unsupported image format: {error}"),
            Self::Load(error) => write!(f, "cannot load image: {error}"),
            Self::Frame(error) => write!(f, "cannot decode first frame: {error}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid image dimensions: {width} × {height}")
            }
            Self::MimeNotAccepted(mime) => write!(f, "image MIME type is not accepted: {mime}"),
        }
    }
}

impl Error for ImportValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StagingAccess(error) => Some(error),
            Self::UnsupportedFormat(error) | Self::Load(error) | Self::Frame(error) => Some(error),
            Self::InvalidDimensions { .. } | Self::MimeNotAccepted(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct GlycinValidator;

impl StagedValidator for GlycinValidator {
    type Output = ImageMetadata;
    type Error = ImportValidationError;

    /// Call only from a background worker: this waits for Glycin and blocks its caller.
    fn validate(&self, staged: &StagedObject<'_>) -> Result<ImageMetadata, Self::Error> {
        let (validated, _) = validate_staged(staged)?;
        Ok(validated)
    }
}

fn validate_staged(
    staged: &StagedObject<'_>,
) -> Result<(ImageMetadata, SandboxMechanism), ImportValidationError> {
    let file = staged
        .open_read()
        .map_err(ImportValidationError::StagingAccess)?;
    let loader = loader_from_staged_file(file);

    // Glycin's default async-io backend uses the same executor for its own blocking work.
    async_io::block_on(async move {
        let image = loader.load().await.map_err(|error| {
            if is_unsupported(&error) {
                ImportValidationError::UnsupportedFormat(Box::new(error))
            } else {
                ImportValidationError::Load(Box::new(error))
            }
        })?;
        let mime_type = image.mime_type().as_str().to_owned();
        let format = format_from_mime(&mime_type)
            .ok_or(ImportValidationError::MimeNotAccepted(mime_type))?;

        let frame = image
            .next_frame()
            .await
            .map_err(|error| ImportValidationError::Frame(Box::new(error)))?;
        let details = image.details();
        let (width, height) = (details.width(), details.height());
        if width == 0 || height == 0 || frame.width() == 0 || frame.height() == 0 {
            return Err(ImportValidationError::InvalidDimensions { width, height });
        }

        let metadata = ImageMetadata::new(format, width, height, frame.delay().is_some())
            .map_err(|_| ImportValidationError::InvalidDimensions { width, height })?;
        Ok((metadata, image.active_sandbox_mechanism()))
    })
}

fn loader_from_staged_file(file: std::fs::File) -> Loader {
    let stream = ReadInputStream::new_seekable(file);
    // SAFETY: Glycin takes ownership of this stream. No reference or clone is kept,
    // and it is never accessed again after transfer to the loader.
    unsafe { Loader::new_stream(stream) }
}

fn is_unsupported(error: &ErrorCtx) -> bool {
    error.unsupported_format().is_some()
        || matches!(error.error(), GlycinError::UnknownContentType(_))
}

fn format_from_mime(mime: &str) -> Option<ImageFormat> {
    match mime {
        "image/png" | "image/apng" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/svg+xml" | "image/svg+xml-compressed" => Some(ImageFormat::Svg),
        "image/vnd.microsoft.icon" | "image/x-win-bitmap" => Some(ImageFormat::Ico),
        "image/webp" => Some(ImageFormat::WebP),
        "image/avif" => Some(ImageFormat::Avif),
        "image/gif" => Some(ImageFormat::Gif),
        "image/bmp" => Some(ImageFormat::Bmp),
        "image/tiff" => Some(ImageFormat::Tiff),
        "image/x-xpixmap" => Some(ImageFormat::Xpm),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pigoune_core::{AssetId, AssetRecord, Library, ObjectHash, ObjectStore, OriginalFilename};
    use std::{ffi::OsStr, fs};
    use tempfile::tempdir;

    const PNG: &[u8] = include_bytes!("../../tests/fixtures/sample.png");
    const JPEG: &[u8] = include_bytes!("../../tests/fixtures/sample.jpg");
    const WEBP: &[u8] = include_bytes!("../../tests/fixtures/sample.webp");
    const AVIF: &[u8] = include_bytes!("../../tests/fixtures/sample.avif");
    const BMP: &[u8] = include_bytes!("../../tests/fixtures/sample.bmp");
    const TIFF: &[u8] = include_bytes!("../../tests/fixtures/sample.tiff");
    const XPM: &[u8] = include_bytes!("../../tests/fixtures/sample.xpm");
    const ICO: &[u8] = include_bytes!("../../tests/fixtures/sample.ico");
    const GIF: &[u8] = include_bytes!("../../tests/fixtures/animated.gif");
    const APNG: &[u8] = include_bytes!("../../tests/fixtures/animated.apng");
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><rect width="3" height="2" fill="#315b8f"/></svg>"##;

    #[test]
    fn accepted_mime_aliases_have_explicit_domain_formats() {
        for (mime, format) in [
            ("image/png", ImageFormat::Png),
            ("image/apng", ImageFormat::Png),
            ("image/jpeg", ImageFormat::Jpeg),
            ("image/svg+xml", ImageFormat::Svg),
            ("image/svg+xml-compressed", ImageFormat::Svg),
            ("image/vnd.microsoft.icon", ImageFormat::Ico),
            ("image/x-win-bitmap", ImageFormat::Ico),
            ("image/webp", ImageFormat::WebP),
            ("image/avif", ImageFormat::Avif),
            ("image/gif", ImageFormat::Gif),
            ("image/bmp", ImageFormat::Bmp),
            ("image/tiff", ImageFormat::Tiff),
            ("image/x-xpixmap", ImageFormat::Xpm),
        ] {
            assert_eq!(format_from_mime(mime), Some(format));
        }
        assert_eq!(format_from_mime("image/icns"), None);
        assert_eq!(format_from_mime("image/unknown"), None);
    }

    #[test]
    fn staged_glycin_image_persists_with_asset() {
        let directory = tempdir().unwrap();
        let mut library = Library::create(directory.path()).unwrap();
        let validated = library
            .object_store
            .stage_reader(PNG, Some(OsStr::new("original.jpg")))
            .unwrap()
            .validate_with(&GlycinValidator)
            .unwrap();
        let metadata = *validated.validation();
        let published = validated.publish().unwrap();
        let object = published.stored.object;
        let asset = AssetRecord {
            id: AssetId::new(),
            object_hash: object.hash,
            original_filename: OriginalFilename::from_bytes(b"original.jpg".to_vec()).unwrap(),
            display_name: "Original".into(),
            imported_at_utc_us: 1_700_000_000_000_000,
        };
        library
            .database
            .import_published_asset(&object, metadata, &asset)
            .unwrap();
        let stored = library.database.get_object(object.hash).unwrap().unwrap();
        assert_eq!(object.hash, ObjectHash::from_bytes(PNG));
        assert_eq!(
            fs::read(directory.path().join(&object.relative_path)).unwrap(),
            PNG
        );
        assert_eq!(stored.object.size, PNG.len() as u64);
        assert_eq!(stored.metadata.unwrap().format(), ImageFormat::Png);
        assert_eq!(
            (
                stored.metadata.unwrap().width(),
                stored.metadata.unwrap().height()
            ),
            (3, 2)
        );
        assert!(!stored.metadata.unwrap().animated());
        let read_asset = library.database.get_asset(asset.id).unwrap().unwrap();
        assert_eq!(read_asset.id, asset.id);
        assert_eq!(read_asset.original_filename.as_bytes(), b"original.jpg");
        assert_eq!(read_asset, asset);
    }

    fn validate_and_publish(
        bytes: &[u8],
        name: Option<&str>,
        format: ImageFormat,
        size: (u32, u32),
    ) {
        let library = tempdir().expect("temporary library");
        let store = ObjectStore::new(library.path()).expect("object store");
        let staged = store
            .stage_reader(bytes, name.map(OsStr::new))
            .expect("stage bytes");
        let validated = staged
            .validate_with(&GlycinValidator)
            .expect("decode staged image");
        assert_eq!(validated.validation().format(), format);
        assert_eq!(
            (
                validated.validation().width(),
                validated.validation().height()
            ),
            size
        );
        let published = validated.publish().expect("publish image");
        assert_eq!(published.stored.object.hash, ObjectHash::from_bytes(bytes));
        assert_eq!(
            fs::read(library.path().join(published.stored.object.relative_path))
                .expect("published bytes"),
            bytes
        );
    }

    #[test]
    fn png_and_jpeg_decode_and_publish_exact_bytes() {
        validate_and_publish(PNG, Some("sample.png"), ImageFormat::Png, (3, 2));
        validate_and_publish(JPEG, Some("sample.jpg"), ImageFormat::Jpeg, (3, 2));
    }

    #[test]
    fn filename_does_not_determine_format() {
        validate_and_publish(PNG, Some("misnamed.jpg"), ImageFormat::Png, (3, 2));
        validate_and_publish(PNG, None, ImageFormat::Png, (3, 2));
        validate_and_publish(PNG, Some("sample.unknown"), ImageFormat::Png, (3, 2));
    }

    #[test]
    fn invalid_data_is_not_published_and_staging_is_removed() {
        for (bytes, name) in [
            (b"this is text".as_slice(), "fake.png"),
            (&PNG[..20], "truncated.png"),
        ] {
            let library = tempdir().expect("temporary library");
            let store = ObjectStore::new(library.path()).expect("object store");
            let staged = store
                .stage_reader(bytes, Some(OsStr::new(name)))
                .expect("stage bytes");
            assert!(staged.validate_with(&GlycinValidator).is_err());
            assert_eq!(
                fs::read_dir(library.path().join("objects/.tmp"))
                    .expect("staging directory")
                    .count(),
                0
            );
            assert!(
                !store
                    .contains(ObjectHash::from_bytes(bytes))
                    .expect("store lookup")
            );
        }
    }

    #[test]
    fn additional_local_loaders_decode() {
        validate_and_publish(WEBP, Some("sample.webp"), ImageFormat::WebP, (3, 2));
        validate_and_publish(AVIF, Some("sample.avif"), ImageFormat::Avif, (4, 4));
        validate_and_publish(BMP, Some("sample.bmp"), ImageFormat::Bmp, (3, 2));
        validate_and_publish(TIFF, Some("sample.tiff"), ImageFormat::Tiff, (3, 2));
        validate_and_publish(XPM, Some("sample.xpm"), ImageFormat::Xpm, (3, 2));
        validate_and_publish(ICO, Some("sample.ico"), ImageFormat::Ico, (16, 16));
        validate_and_publish(SVG, Some("sample.svg"), ImageFormat::Svg, (3, 2));
    }

    #[test]
    fn first_frame_reports_animation() {
        for (bytes, name, format) in [
            (GIF, "animated.gif", ImageFormat::Gif),
            (APNG, "animated.apng", ImageFormat::Png),
        ] {
            let library = tempdir().expect("temporary library");
            let store = ObjectStore::new(library.path()).expect("object store");
            let staged = store
                .stage_reader(bytes, Some(OsStr::new(name)))
                .expect("stage animation");
            let validated = staged
                .validate_with(&GlycinValidator)
                .expect("decode animation");
            assert_eq!(validated.validation().format(), format);
            assert!(validated.validation().animated());
        }
    }

    #[test]
    fn installed_flatpak_uses_a_loader_sandbox() {
        if std::env::var_os("PIGOUNE_CHECK_GLYCIN_SANDBOX").is_none() {
            return;
        }
        let library = tempdir().expect("temporary library");
        let store = ObjectStore::new(library.path()).expect("object store");
        let staged = store.stage_reader(PNG, None).expect("stage png");
        let (_, mechanism) = validate_staged(&staged).expect("decode png");
        assert_eq!(mechanism, SandboxMechanism::FlatpakSpawn);
    }
}
