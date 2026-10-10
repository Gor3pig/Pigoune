const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];
const GIF_SIGNATURES: [&[u8]; 2] = [b"GIF87a", b"GIF89a"];
const ICO_SIGNATURE: &[u8] = &[0, 0, 1, 0];
const TIFF_SIGNATURES: [&[u8]; 4] = [b"II*\0", b"MM\0*", b"II+\0", b"MM\0+"];
const JXL_SIGNATURES: [&[u8]; 2] = [&[0xFF, 0x0A], b"\0\0\0\x0cJXL \r\n\x87\n"];
const BMP_SIGNATURE: &[u8] = b"BM";
const BMP_INFO_HEADER_SIZES: [u32; 7] = [12, 40, 52, 56, 64, 108, 124];
const AVIF_BRANDS: [&[u8]; 2] = [b"avif", b"avis"];
const BRAND_LENGTH: usize = 4;
const UTF8_BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetFormat {
    Svg,
    Png,
    Jpeg,
    Webp,
    Avif,
    Jxl,
    Gif,
    Tiff,
    Bmp,
    Ico,
}

impl AssetFormat {
    pub const ALL: [Self; 10] = [
        Self::Svg,
        Self::Png,
        Self::Jpeg,
        Self::Webp,
        Self::Avif,
        Self::Jxl,
        Self::Gif,
        Self::Tiff,
        Self::Bmp,
        Self::Ico,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
            Self::Bmp => "bmp",
            Self::Ico => "ico",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.code() == code)
    }
}

pub fn detect_binary(header: &[u8]) -> Option<AssetFormat> {
    if header.starts_with(PNG_SIGNATURE) {
        Some(AssetFormat::Png)
    } else if header.starts_with(JPEG_SIGNATURE) {
        Some(AssetFormat::Jpeg)
    } else if GIF_SIGNATURES
        .iter()
        .any(|signature| header.starts_with(signature))
    {
        Some(AssetFormat::Gif)
    } else if is_webp(header) {
        Some(AssetFormat::Webp)
    } else if is_ico(header) {
        Some(AssetFormat::Ico)
    } else if is_avif(header) {
        Some(AssetFormat::Avif)
    } else if JXL_SIGNATURES
        .iter()
        .any(|signature| header.starts_with(signature))
    {
        Some(AssetFormat::Jxl)
    } else if TIFF_SIGNATURES
        .iter()
        .any(|signature| header.starts_with(signature))
    {
        Some(AssetFormat::Tiff)
    } else if is_bmp(header) {
        Some(AssetFormat::Bmp)
    } else {
        None
    }
}

pub fn looks_like_markup(header: &[u8]) -> bool {
    let content = header.strip_prefix(UTF8_BYTE_ORDER_MARK).unwrap_or(header);
    content.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'<')
}

fn is_webp(header: &[u8]) -> bool {
    header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WEBP")
}

fn is_ico(header: &[u8]) -> bool {
    header.starts_with(ICO_SIGNATURE) && header.get(4..6).is_some_and(|count| count != [0, 0])
}

fn is_avif(header: &[u8]) -> bool {
    if header.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let Some(box_length) = header
        .first_chunk::<4>()
        .map(|bytes| u32::from_be_bytes(*bytes))
    else {
        return false;
    };
    let box_end = usize::try_from(box_length)
        .unwrap_or(usize::MAX)
        .min(header.len());
    let major_brand = header.get(8..12);
    let (compatible_brands, _) = header
        .get(16..box_end)
        .unwrap_or_default()
        .as_chunks::<BRAND_LENGTH>();
    major_brand.is_some_and(|brand| AVIF_BRANDS.contains(&brand))
        || compatible_brands
            .iter()
            .any(|brand| AVIF_BRANDS.contains(&brand.as_slice()))
}

fn is_bmp(header: &[u8]) -> bool {
    header.starts_with(BMP_SIGNATURE)
        && header.get(14..18).is_some_and(|size| {
            BMP_INFO_HEADER_SIZES
                .contains(&u32::from_le_bytes([size[0], size[1], size[2], size[3]]))
        })
}

#[cfg(test)]
mod tests {
    use super::{AssetFormat, detect_binary, looks_like_markup};

    #[test]
    fn binary_formats_are_recognized_by_their_signature() {
        let cases: [(&[u8], AssetFormat); 15] = [
            (b"\x89PNG\r\n\x1a\nrest", AssetFormat::Png),
            (&[0xFF, 0xD8, 0xFF, 0xE0], AssetFormat::Jpeg),
            (b"GIF87a....", AssetFormat::Gif),
            (b"GIF89a....", AssetFormat::Gif),
            (b"RIFF\x10\0\0\0WEBPVP8 ", AssetFormat::Webp),
            (&[0, 0, 1, 0, 2, 0], AssetFormat::Ico),
            (b"\0\0\0\x1cftypavif\0\0\0\0mif1avifmiaf", AssetFormat::Avif),
            (b"\0\0\0\x18ftypmif1\0\0\0\0mif1avif", AssetFormat::Avif),
            (b"\0\0\0\x18ftypavis\0\0\0\0msf1miaf", AssetFormat::Avif),
            (&[0xFF, 0x0A, 0x10, 0x00], AssetFormat::Jxl),
            (
                b"\0\0\0\x0cJXL \r\n\x87\n\0\0\0\x14ftypjxl ",
                AssetFormat::Jxl,
            ),
            (b"II*\0\x08\0\0\0", AssetFormat::Tiff),
            (b"MM\0*\0\0\0\x08", AssetFormat::Tiff),
            (b"II+\0\x08\0\0\0", AssetFormat::Tiff),
            (
                b"BM\xf6\0\0\0\0\0\0\0\x36\0\0\0\x28\0\0\0",
                AssetFormat::Bmp,
            ),
        ];
        for (header, expected) in cases {
            assert_eq!(detect_binary(header), Some(expected));
        }
    }

    #[test]
    fn other_content_is_not_a_binary_image() {
        let cases: [&[u8]; 10] = [
            b"",
            b"hello world",
            b"RIFF\x10\0\0\0WAVEfmt ",
            &[0, 0, 1, 0, 0, 0],
            &[0, 0, 2, 0, 1, 0],
            b"%PDF-1.7",
            b"\0\0\0\x18ftypheic\0\0\0\0mif1heic",
            b"\0\0\0\x10ftypisom\0\0\0\0avif",
            b"\0\0\0\x18ftypmp42\0\0\0\0isomavc1",
            b"BMW is a car maker",
        ];
        for header in cases {
            assert_eq!(detect_binary(header), None);
        }
    }

    #[test]
    fn markup_may_start_after_spaces_or_a_byte_order_mark() {
        assert!(looks_like_markup(b"<svg/>"));
        assert!(looks_like_markup(b"  \n\t<?xml version=\"1.0\"?>"));
        assert!(looks_like_markup(b"\xEF\xBB\xBF<svg/>"));
        assert!(!looks_like_markup(b"plain text"));
        assert!(!looks_like_markup(b""));
    }

    #[test]
    fn each_format_code_leads_back_to_its_format() {
        for format in AssetFormat::ALL {
            assert_eq!(AssetFormat::from_code(format.code()), Some(format));
        }
        assert_eq!(AssetFormat::from_code("psd"), None);
    }
}
