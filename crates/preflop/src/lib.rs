//! The multiway preflop solver. External-sampling MCCFR over the preflop tree with
//! bucketed postflop rollouts, and the chart generator that turns its equilibrium into
//! the range charts the app ships.
//!
//! Phase 0 skeleton. Phase 11 builds the bucketing, the MCCFR, and the chart generator.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "preflop"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
