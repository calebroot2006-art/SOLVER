//! Bot policies. A preflop policy read from range charts, a postflop policy read
//! from the solved-spot library with bet-size translation, a bounded live re-solve when
//! the spot is missing, mixed-strategy sampling, timing variance, and style profiles.
//!
//! Phase 0 skeleton. Phase 8 builds the policy plumbing and the live re-solve path.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "bots"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
