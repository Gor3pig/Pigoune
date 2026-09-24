use std::{fmt, str::FromStr};
use uuid::{Error, Uuid};

/// Identity of a logical asset, independent of its stored content.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssetId(Uuid);

impl AssetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn to_bytes(self) -> [u8; 16] {
        *self.0.as_bytes()
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for AssetId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::AssetId;

    #[test]
    fn generated_ids_are_distinct_and_round_trip() {
        let first = AssetId::new();
        let second = AssetId::new();
        assert_ne!(first, second);
        assert_eq!(first.to_string().parse::<AssetId>().unwrap(), first);
    }
}
