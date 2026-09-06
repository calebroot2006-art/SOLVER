//! Physical-card Leduc matching OpenSpiel's two-player default rules.

use crate::{Rules, ToyGame};

/// Construct Leduc with six private states, ante one, and two raises per round.
#[must_use]
pub fn game() -> ToyGame {
    ToyGame::new(Rules::Leduc)
}
