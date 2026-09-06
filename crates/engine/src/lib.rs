//! The hand engine. Two to nine seats, the button and blinds, antes and big-blind
//! ante, straddles and dead blinds, betting rounds, all-ins with multiple side pots,
//! showdown and chop, blind schedules, single-table tournaments, and hand-history
//! export.
//!
//! Phase 0 skeleton. Phase 6 builds the engine and its PokerKit fixture replay.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "engine"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
