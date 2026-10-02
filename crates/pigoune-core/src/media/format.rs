const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];
const GIF_SIGNATURES: [&[u8]; 2] = [b"GIF87a", b"GIF89a"];
const ICO_SIGNATURE: &[u8] = &[0, 0, 1, 0];
const UTF8_BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetFormat {
    Svg,
    Png,
    Jpeg,
    Webp,
    Gif,
    Ico,
}

impl AssetFormat {
    pub const ALL: [Self; 6] = [
        Self::Svg,
        Self::Png,
        Self::Jpeg,
        Self::Webp,
        Self::Gif,
        Self::Ico,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Gif => "gif",
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

#[cfg(test)]
mod tests {
    use super::{AssetFormat, detect_binary, looks_like_markup};

    #[test]
    fn binary_formats_are_recognized_by_their_signature() {
        let cases: [(&[u8], AssetFormat); 6] = [
            (b"\x89PNG\r\n\x1a\nrest", AssetFormat::Png),
            (&[0xFF, 0xD8, 0xFF, 0xE0], AssetFormat::Jpeg),
            (b"GIF87a....", AssetFormat::Gif),
            (b"GIF89a....", AssetFormat::Gif),
            (b"RIFF\x10\0\0\0WEBPVP8 ", AssetFormat::Webp),
            (&[0, 0, 1, 0, 2, 0], AssetFormat::Ico),
        ];
        for (header, expected) in cases {
            assert_eq!(detect_binary(header), Some(expected));
        }
    }

    #[test]
    fn other_content_is_not_a_binary_image() {
        let cases: [&[u8]; 6] = [
            b"",
            b"hello world",
            b"RIFF\x10\0\0\0WAVEfmt ",
            &[0, 0, 1, 0, 0, 0],
            &[0, 0, 2, 0, 1, 0],
            b"%PDF-1.7",
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
        assert_eq!(AssetFormat::from_code("bmp"), None);
    }
}
