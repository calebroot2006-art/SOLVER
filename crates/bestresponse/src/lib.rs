//! Exploitability. The best-response calculator over the public-state tree, and
//! the metric built from it: `nash_conv` in chips per hand, its half, and that half
//! as a percentage of the fixed root pot.
//!
//! Public entry points share the checked numerical core with the CFR solver.
//! Legacy callback games and immutable river and postflop policies retain their
//! own bindings.

pub use postflop::{Exploitability, SolveError, best_response, expected_value, exploitability};

/// Queries permanently bound to an immutable river game's ranges and payoffs.
pub mod river {
    use postflop::{Exploitability, RiverStrategy, SolveError};

    /// Expected net chips under the supplied complete profile.
    pub fn expected_value(strategy: &RiverStrategy, player: usize) -> Result<f64, SolveError> {
        strategy.expected_value(player)
    }

    /// Maximum net chips, choosing actions per own information set.
    pub fn best_response(strategy: &RiverStrategy, player: usize) -> Result<f64, SolveError> {
        strategy.best_response(player)
    }

    /// Both best responses and the root-pot percentage certificate for this game.
    pub fn exploitability(strategy: &RiverStrategy) -> Result<Exploitability, SolveError> {
        strategy.exploitability()
    }
}

/// Queries permanently bound to an immutable street-aware postflop game.
///
/// The walk runs over every dealt runout in f64, whatever precision the solve
/// stored, so the number these return measures the whole tree.
pub mod streets {
    use postflop::{Exploitability, PostflopStrategy, SolveError};

    /// Expected net chips under the supplied complete profile.
    pub fn expected_value(strategy: &PostflopStrategy, player: usize) -> Result<f64, SolveError> {
        strategy.expected_value(player)
    }

    /// Maximum net chips, choosing actions per own information set.
    pub fn best_response(strategy: &PostflopStrategy, player: usize) -> Result<f64, SolveError> {
        strategy.best_response(player)
    }

    /// Both best responses and the root-pot percentage certificate for this game.
    pub fn exploitability(strategy: &PostflopStrategy) -> Result<Exploitability, SolveError> {
        strategy.exploitability()
    }
}

/// The crate's name, retained for workspace discovery.
#[must_use]
pub const fn crate_name() -> &'static str {
    "bestresponse"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
