//! Cards, decks, combos, hand evaluation, and ranges. Card and combo types, the
//! Pio range-string parser and printer, the 13x13 grid mapping, card removal, and a
//! 7-card evaluator that is property-tested against a brute-force reference over
//! random hands.
//!
//! Phase 0 skeleton. Phase 2 builds the card types, the range parser, and the 7-card evaluator.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "cards"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
