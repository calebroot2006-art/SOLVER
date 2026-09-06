//! Checked, immutable heads-up postflop betting trees with explicit size menus.
//!
//! [`RiverTree`] models one street: wager actions record total river
//! contributions and there are no chance nodes. [`PostflopTree`] models a flop,
//! turn, or river tree: wager actions record total contributions since the root,
//! per-street size menus are configured separately, and one abstract
//! [`PostflopNodeKind::Chance`] node stands for a street transition, leaving the
//! runouts it covers to the solver. A `PostflopTree` starting on the river is
//! node-for-node identical to the `RiverTree` with the same settings.
//!
//! Neither tree models rake, side pots, or translation of actions outside the
//! configured menus.

mod postflop;
mod river;
mod sizing;

use std::fmt;

pub use postflop::{PostflopNode, PostflopNodeKind, PostflopTree, PostflopTreeConfig, Street};
pub use river::{Action, RiverNode, RiverNodeKind, RiverTree, RiverTreeConfig, Terminal};
pub use sizing::{BetSize, BetSizeOptions};

/// Whole-chip amount; public configuration limits amounts to one billion.
pub type Chips = u64;
/// Index into the tree's immutable node storage.
pub type NodeId = u32;

const MAX_CHIPS: Chips = 1_000_000_000;

/// Rejected betting syntax, invalid configuration, or exhausted tree resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeError(String);

impl TreeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TreeError {}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), TreeError> {
    values
        .try_reserve_exact(additional)
        .map_err(|error| TreeError::new(format!("tree allocation failed: {error}")))
}

/// The crate's name, retained for workspace discovery.
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
