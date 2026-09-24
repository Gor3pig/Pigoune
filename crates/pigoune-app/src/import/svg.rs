use std::{error::Error, fmt, io::BufReader};

use cssparser::{Parser, Token};
use pigoune_core::StagedObject;
use quick_xml::{escape, events::Event, name::ResolveResult, reader::NsReader};
use xmlparser::{EntityDefinition, Token as XmlToken, Tokenizer};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

#[derive(Debug)]
pub enum SvgAnalysisError {
    Read(std::io::Error),
    Xml,
    Css,
    Structure,
}

impl fmt::Display for SvgAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "staging read failed: {error}"),
            Self::Xml => f.write_str("invalid XML or unsupported DTD/entity syntax"),
            Self::Css => f.write_str("invalid CSS resource syntax"),
            Self::Structure => f.write_str("invalid SVG document structure"),
        }
    }
}

impl Error for SvgAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            _ => None,
        }
    }
}

fn external(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.starts_with('#')
        && !value
            .get(..5)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
}

// These href contexts refer to rendering or document resources. An <a> href is navigation.
fn resource_element(name: &str) -> bool {
    matches!(
        name,
        "image"
            | "use"
            | "feImage"
            | "script"
            | "textPath"
            | "mpath"
            | "linearGradient"
            | "radialGradient"
            | "pattern"
            | "filter"
            | "clipPath"
            | "mask"
            | "marker"
            | "animate"
            | "animateMotion"
            | "animateTransform"
            | "set"
    )
}

fn presentation_attribute(name: &str) -> bool {
    matches!(
        name,
        "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "cursor"
            | "shape-inside"
            | "shape-subtract"
            | "background-image"
    )
}

fn css_references(css: &str) -> Result<bool, SvgAnalysisError> {
    scan_css(&mut Parser::new(css))
}

fn scan_css(parser: &mut Parser<'_>) -> Result<bool, SvgAnalysisError> {
    let mut found = false;
    let mut import = false;
    while !parser.is_exhausted() {
        let token = parser.next().map_err(|_| SvgAnalysisError::Css)?.clone();
        if token.is_parse_error() {
            return Err(SvgAnalysisError::Css);
        }
        match token {
            Token::AtKeyword(name) if name.eq_ignore_ascii_case("import") => import = true,
            Token::QuotedString(value) if import => {
                found |= external(&value);
                import = false;
            }
            Token::UnquotedUrl(value) => {
                found |= external(&value);
                import = false;
            }
            Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                let value = parser
                    .parse_nested_block(|nested| {
                        nested
                            .expect_string_cloned()
                            .map_err(cssparser::ParseError::<()>::from)
                    })
                    .map_err(|_| SvgAnalysisError::Css)?;
                found |= external(&value);
                import = false;
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                found |= parser
                    .parse_nested_block(|nested| {
                        scan_css(nested).map_err(|_| nested.new_error_for_next_token::<()>())
                    })
                    .map_err(|_| SvgAnalysisError::Css)?;
                import = false;
            }
            _ => import = false,
        }
    }
    if import {
        return Err(SvgAnalysisError::Css);
    }
    Ok(found)
}

fn doctype_references(raw: &str) -> Result<bool, SvgAnalysisError> {
    let document = format!("<!DOCTYPE {raw}><svg/>");
    let dtd_end = document.len() - "<svg/>".len();
    let mut found = false;
    let mut saw_doctype = false;
    let mut covered = 0;
    for token in Tokenizer::from(document.as_str()) {
        let token = token.map_err(|_| SvgAnalysisError::Xml)?;
        let span = token.span();
        if span.start() < dtd_end {
            // xmlparser skips some DTD declaration kinds. Reject those gaps rather than
            // claiming a complete analysis of identifiers it did not expose.
            if !document[covered..span.start()]
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
            {
                return Err(SvgAnalysisError::Xml);
            }
            covered = span.end();
        }
        match token {
            XmlToken::DtdStart { external_id, .. } | XmlToken::EmptyDtd { external_id, .. } => {
                saw_doctype = true;
                found |= external_id.is_some();
            }
            XmlToken::EntityDeclaration {
                definition: EntityDefinition::ExternalId(_),
                ..
            } => found = true,
            _ => {}
        }
    }
    if !saw_doctype || covered != dtd_end {
        return Err(SvgAnalysisError::Xml);
    }
    Ok(found)
}

pub fn has_external_references(staged: &StagedObject<'_>) -> Result<bool, SvgAnalysisError> {
    let file = staged.open_read().map_err(SvgAnalysisError::Read)?;
    scan(BufReader::new(file))
}

fn scan(source: impl std::io::BufRead) -> Result<bool, SvgAnalysisError> {
    let mut reader = NsReader::from_reader(source);
    let mut buf = Vec::new();
    let mut found = false;
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut doctype_seen = false;
    let mut style_depth = None;
    let mut style = String::new();
    loop {
        let (namespace, event) = reader
            .read_resolved_event_into(&mut buf)
            .map_err(|_| SvgAnalysisError::Xml)?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if style_depth.is_some() {
                    return Err(SvgAnalysisError::Structure);
                }
                if matches!(namespace, ResolveResult::Unknown(_)) {
                    return Err(SvgAnalysisError::Xml);
                }
                if depth == 0 {
                    if root_seen || element.local_name().as_ref() != "svg" {
                        return Err(SvgAnalysisError::Structure);
                    }
                    root_seen = true;
                }
                let svg_element = matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == SVG_NS)
                    || matches!(namespace, ResolveResult::Unbound);
                if !svg_element && depth == 0 {
                    return Err(SvgAnalysisError::Structure);
                }
                let name = element.local_name();
                for attr in element.attributes() {
                    let attr = attr.map_err(|_| SvgAnalysisError::Xml)?;
                    let (attr_ns, local) = reader.resolver().resolve_attribute(attr.key);
                    if matches!(attr_ns, ResolveResult::Unknown(_)) {
                        return Err(SvgAnalysisError::Xml);
                    }
                    let value = attr
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| SvgAnalysisError::Xml)?;
                    let local = local.as_ref();
                    let href = local == "href"
                        && (matches!(attr_ns, ResolveResult::Unbound)
                            || matches!(attr_ns, ResolveResult::Bound(ns) if ns.as_ref() == XLINK_NS));
                    if svg_element && href && resource_element(name.as_ref()) {
                        found |= external(&value);
                    } else if svg_element && (local == "style" || presentation_attribute(local)) {
                        found |= css_references(&value)?;
                    }
                }
                if !empty {
                    depth += 1;
                    if svg_element && name.as_ref() == "style" {
                        style_depth = Some(depth);
                        style.clear();
                    }
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    return Err(SvgAnalysisError::Structure);
                }
                if style_depth == Some(depth) {
                    found |= css_references(&style)?;
                    style.clear();
                    style_depth = None;
                }
                depth -= 1;
            }
            Event::Text(text) => {
                let decoded = text.xml10_content();
                if depth == 0 && !decoded.trim().is_empty() {
                    return Err(SvgAnalysisError::Structure);
                }
                if style_depth.is_some() {
                    style.push_str(&escape::unescape(&decoded).map_err(|_| SvgAnalysisError::Xml)?);
                }
            }
            Event::CData(text) => {
                if depth == 0 {
                    return Err(SvgAnalysisError::Structure);
                }
                if style_depth.is_some() {
                    style.push_str(&text.xml10_content());
                }
            }
            Event::PI(pi) if pi.target() == "xml-stylesheet" => {
                for attr in pi.attributes() {
                    let attr = attr.map_err(|_| SvgAnalysisError::Xml)?;
                    if attr.key.as_ref() == "href" {
                        let value = attr
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|_| SvgAnalysisError::Xml)?;
                        found |= external(&value);
                    }
                }
            }
            Event::DocType(dtd) => {
                if root_seen || doctype_seen {
                    return Err(SvgAnalysisError::Structure);
                }
                doctype_seen = true;
                found |= doctype_references(dtd.as_ref())?;
            }
            Event::GeneralRef(reference) => {
                let character = if let Some(character) = reference
                    .resolve_char_ref()
                    .map_err(|_| SvgAnalysisError::Xml)?
                {
                    character.to_string()
                } else {
                    escape::resolve_predefined_entity(reference.as_ref())
                        .ok_or(SvgAnalysisError::Xml)?
                        .to_owned()
                };
                if style_depth.is_some() {
                    style.push_str(&character);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if !root_seen || depth != 0 || style_depth.is_some() {
        return Err(SvgAnalysisError::Structure);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn references(body: &str) -> Result<bool, SvgAnalysisError> {
        scan(Cursor::new(body.as_bytes()))
    }

    fn svg(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{body}</svg>"#)
    }

    #[test]
    fn href_resource_contexts_and_namespaces() {
        for body in [
            r#"<rect width="3" height="2"/>"#,
            r##"<use href="#symbol"/>"##,
            r#"<image href="data:image/png;base64,aGVsbG8="/>"#,
            r#"<image href="DATA:image/png;base64,aGVsbG8="/>"#,
            r#"<a href="https://example.invalid"><text>Website</text></a>"#,
            r#"<g xml:base="https://example.invalid/"/>"#,
            r#"<text>A &amp; B</text>"#,
        ] {
            assert!(!references(&svg(body)).unwrap(), "{body}");
        }
        for body in [
            r#"<image href="image.png"/>"#,
            r#"<image href="https://example.invalid/image.png"/>"#,
            r#"<use href="/assets/symbols.svg#icon"/>"#,
            r#"<script href="script.js"/>"#,
            r#"<textPath href="text.svg#path"/>"#,
            r#"<mpath href="motion.svg#path"/>"#,
            r#"<image xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="image.png"/>"#,
            r#"<image xmlns:alt="http://www.w3.org/1999/xlink" alt:href="image.png"/>"#,
        ] {
            assert!(references(&svg(body)).unwrap(), "{body}");
        }
    }

    #[test]
    fn css_urls_and_imports() {
        for body in [
            r##"<rect fill="url(#gradient)"/>"##,
            r#"<rect fill="url(data:image/png;base64,aGVsbG8=)"/>"#,
            r#"<style>.a { fill: url('data:image/png;base64,aGVsbG8=') }</style>"#,
            r#"<style>.a { fill: url( '  #gradient  ' ) }</style>"#,
            r#"<style>.a { fill: url(\64 ata:image/png;base64,aGVsbG8=) }</style>"#,
            r#"<style>@import "da\74 a:text/css,body{}";</style>"#,
        ] {
            assert!(!references(&svg(body)).unwrap(), "{body}");
        }
        for body in [
            r#"<rect fill="url(texture.svg#pattern)"/>"#,
            r#"<rect filter="url(filters.svg#blur)"/>"#,
            r#"<rect style="background-image:url(image.png)"/>"#,
            r#"<style>.a { background: url(image.png) }</style>"#,
            r#"<style>@import url(theme.css);</style>"#,
            r#"<style>@import "theme.css";</style>"#,
            r#"<style>.a { fill: url(  'tex\\74 ure.svg'  ) }</style>"#,
            r#"<style><![CDATA[.a { fill: url(image.png) }]]></style>"#,
        ] {
            assert!(references(&svg(body)).unwrap(), "{body}");
        }
    }

    #[test]
    fn stylesheet_and_external_dtd() {
        assert!(
            references(&format!(
                "<?xml-stylesheet href=\"theme.css\"?>{}",
                svg("<rect/>")
            ))
            .unwrap()
        );
        assert!(
            references(&format!(
                "<!DOCTYPE svg SYSTEM \"https://example.invalid/dtd\">{}",
                svg("<rect/>")
            ))
            .unwrap()
        );
        assert!(
            references(&format!(
                "<!DOCTYPE svg PUBLIC \"id\" \"https://example.invalid/dtd\">{}",
                svg("<rect/>")
            ))
            .unwrap()
        );
        assert!(
            references(&format!(
                "<!DOCTYPE svg [<!ENTITY remote SYSTEM \"file:///tmp/remote\">]>{}",
                svg("<rect/>")
            ))
            .unwrap()
        );
        assert!(
            !references(&format!(
                "<!DOCTYPE svg [<!ENTITY local \"safe\">]>{}",
                svg("<rect/>")
            ))
            .unwrap()
        );
    }

    #[test]
    fn incomplete_analysis_is_an_error() {
        assert!(references("<svg><image href='file.png'></svg>").is_err());
        assert!(references("<svg><style>.a { fill: url('bad' }</style></svg>").is_err());
        assert!(references(&svg("<text>&unknown;</text>")).is_err());
        assert!(references(&svg("<wrong:image href='file.png'/>")).is_err());
        assert!(
            references(&format!(
                "<!DOCTYPE svg [<!NOTATION n SYSTEM 'resource'>]>{}",
                svg("<rect/>")
            ))
            .is_err()
        );
    }
}
