use gettextrs::gettext;
use pigoune_core::annotations::Color;

pub const SWATCH_COUNT: usize = 8;

const fn opaque(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub const COLORS: [Color; SWATCH_COUNT] = [
    opaque(0xe0, 0x1b, 0x24),
    opaque(0xff, 0x78, 0x00),
    opaque(0xf6, 0xd3, 0x2d),
    opaque(0x33, 0xd1, 0x7a),
    opaque(0x35, 0x84, 0xe4),
    opaque(0x91, 0x41, 0xac),
    opaque(0x24, 0x1f, 0x31),
    opaque(0xff, 0xff, 0xff),
];

pub fn color_name(index: usize) -> String {
    match index {
        0 => gettext("Red"),
        1 => gettext("Orange"),
        2 => gettext("Yellow"),
        3 => gettext("Green"),
        4 => gettext("Blue"),
        5 => gettext("Purple"),
        6 => gettext("Black"),
        _ => gettext("White"),
    }
}

pub fn index_of(color: Color) -> Option<usize> {
    COLORS
        .iter()
        .position(|swatch| (swatch.r, swatch.g, swatch.b) == (color.r, color.g, color.b))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pigoune_core::annotations::{Color, Style};

    use super::{COLORS, index_of};

    #[test]
    fn the_palette_has_eight_different_opaque_colors() {
        let distinct: HashSet<_> = COLORS.iter().map(|c| (c.r, c.g, c.b)).collect();
        assert_eq!(distinct.len(), COLORS.len());
        assert!(COLORS.iter().all(|color| color.a == 255));
    }

    #[test]
    fn a_color_is_found_again_whatever_its_alpha() {
        for (index, color) in COLORS.iter().enumerate() {
            let faded = Color { a: 40, ..*color };
            assert_eq!(index_of(faded), Some(index));
        }
        assert_eq!(
            index_of(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 255
            }),
            None
        );
    }

    #[test]
    fn the_default_drawing_color_is_the_first_swatch() {
        assert_eq!(index_of(Style::default().color), Some(0));
    }
}
