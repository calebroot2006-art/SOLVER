//! The solver core. Vanilla CFR, CFR+, and Discounted CFR behind one `Solver`
//! trait, over a public-state tree with per-node vectors across each player's private
//! states. Phase 1 solves Kuhn and Leduc in that form; phases 3 and 4 add the real
//! postflop tree, the terminal sweep, suit isomorphism, and compressed storage.
//!
//! Phase 0 skeleton. Phase 1 builds the CFR core here; phases 3 and 4 grow it into the real solver.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "postflop"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
