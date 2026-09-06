//! Three-card Kuhn poker: one-chip antes and a single one-chip bet.

use crate::{Rules, ToyGame};

/// Construct the full two-player Kuhn public tree with uniform private weights.
#[must_use]
pub fn game() -> ToyGame {
    ToyGame::new(Rules::Kuhn)
}
