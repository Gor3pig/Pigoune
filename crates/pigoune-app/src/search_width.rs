const NARROWEST_CHARS: i32 = 24;
const WIDEST_CHARS: i32 = 64;
const FIELD_SHARE_NUMERATOR: i32 = 2;
const FIELD_SHARE_DENOMINATOR: i32 = 3;

pub struct EntrySizes {
    pub minimum: i32,
    pub natural: i32,
    pub char_width: i32,
}

fn largest_extra(sizes: &EntrySizes) -> i32 {
    (WIDEST_CHARS - NARROWEST_CHARS) * sizes.char_width.max(0)
}

#[must_use]
pub fn requested_space(sizes: &EntrySizes) -> i32 {
    sizes.natural + largest_extra(sizes) * FIELD_SHARE_DENOMINATOR / FIELD_SHARE_NUMERATOR
}

#[must_use]
pub fn entry_width(available: i32, sizes: &EntrySizes) -> i32 {
    if available <= sizes.natural {
        return available.max(sizes.minimum);
    }
    let extra = (available - sizes.natural) * FIELD_SHARE_NUMERATOR / FIELD_SHARE_DENOMINATOR;
    sizes.natural + extra.min(largest_extra(sizes))
}

#[cfg(test)]
mod tests {
    use super::{EntrySizes, entry_width, requested_space};

    const SIZES: EntrySizes = EntrySizes {
        minimum: 80,
        natural: 254,
        char_width: 10,
    };

    #[test]
    fn little_room_shrinks_the_field_down_to_its_minimum() {
        assert_eq!(entry_width(200, &SIZES), 200);
        assert_eq!(entry_width(50, &SIZES), 80);
    }

    #[test]
    fn two_thirds_of_the_extra_room_go_to_the_field() {
        assert_eq!(entry_width(254 + 150, &SIZES), 254 + 100);
    }

    #[test]
    fn the_field_stops_growing_at_its_widest() {
        assert_eq!(entry_width(5000, &SIZES), 254 + 400);
    }

    #[test]
    fn the_requested_space_lets_the_widest_field_keep_a_third_free() {
        let space = requested_space(&SIZES);
        assert_eq!(space, 254 + 600);
        assert_eq!(entry_width(space, &SIZES), 254 + 400);
    }
}
