//! Toy games and the reference comparison. Kuhn and Leduc in the same
//! vector form the postflop solver uses, a scalar per-history CFR that acts as an
//! oracle for that form, and the captured OpenSpiel curves the CFR variants are
//! checked against.
//!
//! Phase 0 skeleton. Phase 1 fills it in: `kuhn`, `leduc`, `nan_game`, and
//! `history_oracle` in `src/`, the integration tests in `tests/`, and the captured
//! reference data under `reference/openspiel/`.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// toy games land.
#[must_use]
pub const fn crate_name() -> &'static str {
    "toygames"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
