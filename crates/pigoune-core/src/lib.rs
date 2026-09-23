pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn exposes_the_crate_name() {
        assert_eq!(CRATE_NAME, "pigoune-core");
    }
}
