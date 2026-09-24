use sha2::{Digest, Sha256};
use std::{
    fmt,
    io::{self, Read},
    str::FromStr,
};

/// SHA-256 identity of the exact bytes of a physical object.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectHash([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseObjectHashError;

impl fmt::Display for ParseObjectHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected exactly 64 hexadecimal SHA-256 characters")
    }
}

impl std::error::Error for ParseObjectHashError {}

impl ObjectHash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn from_reader(mut reader: impl Read) -> io::Result<Self> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        Ok(Self(hasher.finalize().into()))
    }

    pub(crate) fn from_digest(digest: Sha256) -> Self {
        Self(digest.finalize().into())
    }
}

impl fmt::Display for ObjectHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ObjectHash {
    type Err = ParseObjectHashError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 {
            return Err(ParseObjectHashError);
        }
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = (value.as_bytes()[index * 2] as char)
                .to_digit(16)
                .ok_or(ParseObjectHashError)?;
            let low = (value.as_bytes()[index * 2 + 1] as char)
                .to_digit(16)
                .ok_or(ParseObjectHashError)?;
            *byte = ((high << 4) | low) as u8;
        }
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectHash;

    #[test]
    fn sha256_vector_and_text_round_trip() {
        let hash = ObjectHash::from_bytes(b"abc");
        let text = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hash.to_string(), text);
        assert_eq!(hash.to_string().len(), 64);
        assert_eq!(text.parse::<ObjectHash>().unwrap(), hash);
        assert_eq!(ObjectHash::from_reader(&b"abc"[..]).unwrap(), hash);
    }

    #[test]
    fn rejects_invalid_text() {
        assert!("abc".parse::<ObjectHash>().is_err());
        assert!("g".repeat(64).parse::<ObjectHash>().is_err());
        assert!("é".repeat(32).parse::<ObjectHash>().is_err());
    }
}
