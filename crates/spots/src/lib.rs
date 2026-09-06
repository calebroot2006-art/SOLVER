//! The solved-spot format. The binary and JSON representations of a solved spot and
//! a range chart, the library index, and the CLI that generates a library. This format
//! is the contract between the solver and the app; the UI never reaches past it into
//! solver internals.
//!
//! Phase 0 skeleton. Phase 5 fixes the formats and builds the library generator.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "spots"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
