//! Action trees and bet sizing. The bet-size DSL, the action tree it builds, the
//! raise cap, the all-in threshold, and the pseudo-harmonic translation that maps a
//! human bet size onto the nearest size the tree actually contains.
//!
//! Phase 0 skeleton. Phase 3 builds the bet-size DSL and the action tree;
//! phase 8 uses the translator.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "tree"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
