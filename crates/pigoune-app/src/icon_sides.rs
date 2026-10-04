pub const ICON_SIDES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const CLASSIC_SIDES: [u32; 4] = [16, 32, 48, 256];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconSides(u8);

impl Default for IconSides {
    fn default() -> Self {
        CLASSIC_SIDES
            .into_iter()
            .fold(Self(0), |sides, side| sides.with(side, true))
    }
}

impl IconSides {
    pub fn contains(self, side: u32) -> bool {
        Self::bit_of(side).is_some_and(|bit| self.0 & bit != 0)
    }

    #[must_use]
    pub fn with(self, side: u32, chosen: bool) -> Self {
        let Some(bit) = Self::bit_of(side) else {
            return self;
        };
        if chosen {
            Self(self.0 | bit)
        } else {
            Self(self.0 & !bit)
        }
    }

    pub fn none() -> Self {
        Self(0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn sides(self) -> impl Iterator<Item = u32> {
        ICON_SIDES
            .into_iter()
            .filter(move |side| self.contains(*side))
    }

    fn bit_of(side: u32) -> Option<u8> {
        let position = ICON_SIDES.iter().position(|known| *known == side)?;
        Some(1 << position)
    }
}

#[cfg(test)]
mod tests {
    use super::IconSides;

    #[test]
    fn the_classic_icon_sizes_are_chosen_by_default() {
        let sides: Vec<u32> = IconSides::default().sides().collect();
        assert_eq!(sides, [16, 32, 48, 256]);
    }

    #[test]
    fn sizes_can_be_added_and_removed() {
        let sides = IconSides::default().with(64, true).with(16, false);
        assert!(sides.contains(64));
        assert!(!sides.contains(16));
        assert_eq!(sides.sides().collect::<Vec<_>>(), [32, 48, 64, 256]);
    }

    #[test]
    fn unknown_sizes_are_ignored() {
        assert_eq!(IconSides::default().with(100, true), IconSides::default());
        assert!(!IconSides::default().contains(100));
    }

    #[test]
    fn no_size_at_all_is_noticed() {
        let none = IconSides::default()
            .sides()
            .fold(IconSides::default(), |sides, side| sides.with(side, false));
        assert!(none.is_empty());
    }
}
