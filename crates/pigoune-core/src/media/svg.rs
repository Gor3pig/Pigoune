use std::fs;
use std::path::Path;

use roxmltree::{Document, Node, ParsingOptions};

use super::{AssetFormat, Dimensions, InspectError, MediaInfo};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const PIXELS_PER_INCH: f64 = 96.0;

pub fn inspect(path: &Path) -> Result<MediaInfo, InspectError> {
    let bytes = fs::read(path).map_err(|_| InspectError::Unreadable)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| InspectError::Unsupported)?;
    Ok(MediaInfo {
        format: AssetFormat::Svg,
        dimensions: read_dimensions(text)?,
        is_animated: false,
        embedded_sizes: Vec::new(),
    })
}

fn read_dimensions(text: &str) -> Result<Option<Dimensions>, InspectError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let document = match Document::parse_with_options(text, options) {
        Ok(document) => document,
        Err(_) if text.contains("<svg") => return Err(InspectError::Unreadable),
        Err(_) => return Err(InspectError::Unsupported),
    };

    let root = document.root_element();
    if !is_svg_element(root) {
        return Err(InspectError::Unsupported);
    }
    Ok(declared_size(root).or_else(|| view_box_size(root)))
}

fn is_svg_element(node: Node) -> bool {
    node.tag_name().name() == "svg"
        && matches!(node.tag_name().namespace(), None | Some(SVG_NAMESPACE))
}

fn declared_size(root: Node) -> Option<Dimensions> {
    let width = length_in_pixels(root.attribute("width")?)?;
    let height = length_in_pixels(root.attribute("height")?)?;
    Dimensions::new(whole_pixels(width)?, whole_pixels(height)?)
}

fn view_box_size(root: Node) -> Option<Dimensions> {
    let numbers: Vec<f64> = root
        .attribute("viewBox")?
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let [_, _, width, height] = numbers.as_slice() else {
        return None;
    };
    Dimensions::new(whole_pixels(*width)?, whole_pixels(*height)?)
}

fn length_in_pixels(value: &str) -> Option<f64> {
    let value = value.trim();
    let number = value
        .trim_end_matches(|character: char| character.is_ascii_alphabetic() || character == '%');
    let unit = &value[number.len()..];
    let pixels_per_unit = match unit {
        "" | "px" => 1.0,
        "in" => PIXELS_PER_INCH,
        "cm" => PIXELS_PER_INCH / 2.54,
        "mm" => PIXELS_PER_INCH / 25.4,
        "pt" => PIXELS_PER_INCH / 72.0,
        "pc" => PIXELS_PER_INCH / 6.0,
        _ => return None,
    };
    Some(number.trim().parse::<f64>().ok()? * pixels_per_unit)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and checked to fit in u32 beforehand"
)]
fn whole_pixels(value: f64) -> Option<u32> {
    let rounded = value.round();
    (rounded >= 1.0 && rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
}

#[cfg(test)]
mod tests {
    use super::read_dimensions;
    use crate::media::{Dimensions, InspectError};

    fn dimensions(width: u32, height: u32) -> Option<Dimensions> {
        Dimensions::new(width, height)
    }

    #[test]
    fn width_and_height_in_pixels_are_used() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16px"/>"#;
        assert_eq!(read_dimensions(svg), Ok(dimensions(24, 16)));
    }

    #[test]
    fn physical_units_are_converted_to_pixels() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="72pt"/>"#;
        assert_eq!(read_dimensions(svg), Ok(dimensions(96, 96)));
    }

    #[test]
    fn the_view_box_is_used_when_the_size_is_relative() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0,0 120.4 80"/>"#;
        assert_eq!(read_dimensions(svg), Ok(dimensions(120, 80)));
    }

    #[test]
    fn a_drawing_without_any_size_has_unknown_dimensions() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="4"/></svg>"#;
        assert_eq!(read_dimensions(svg), Ok(None));
    }

    #[test]
    fn a_declaration_doctype_and_byte_order_mark_are_accepted() {
        let svg = "\u{FEFF}<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"/>";
        assert_eq!(read_dimensions(svg), Ok(dimensions(8, 8)));
    }

    #[test]
    fn other_markup_is_not_supported() {
        assert_eq!(
            read_dimensions("<html><body/></html>"),
            Err(InspectError::Unsupported)
        );
        assert_eq!(
            read_dimensions("<p>not xml"),
            Err(InspectError::Unsupported)
        );
    }

    #[test]
    fn a_broken_drawing_is_unreadable() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8"><g></svg>"#;
        assert_eq!(read_dimensions(svg), Err(InspectError::Unreadable));
    }
}
