//! Terminal payoff models. This crate is the dependency-neutral bottom of the
//! solver stack: it owns the scalar type the rest of the solver accumulates in, and
//! the trait that turns stacks, contributions, and pot shares into per-player
//! utilities. Chip EV comes first. ICM and bounty-adjusted ICM arrive in phase 10
//! and change the payoff without changing the tree.
//!
//! Phase 0 skeleton. Phase 1 puts the `Real` alias and the `Payoff` trait here; phase 10 adds ICM.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "payoff"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
