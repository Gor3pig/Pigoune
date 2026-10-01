pub const LIBRARY_FORMAT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::LIBRARY_FORMAT_VERSION;

    #[test]
    fn library_format_starts_at_version_one() {
        assert_eq!(LIBRARY_FORMAT_VERSION, 1);
    }
}
