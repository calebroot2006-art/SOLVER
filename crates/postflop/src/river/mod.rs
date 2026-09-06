//! Owned heads-up river games, bounded CFR sessions, and bound strategy queries.
//! The payoff origin assigns half the existing root pot to each player. Values
//! are net chips relative to that origin, conditioned on compatible private deals.

mod game;
mod memory;
mod solver;
mod strategy;

pub use game::RiverGame;
pub use memory::RiverMemory;
pub use solver::RiverSolver;
pub use strategy::{DecisionValues, RiverStrategy};
