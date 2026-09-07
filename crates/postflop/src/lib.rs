//! Two-player zero-sum CFR over public trees and fixed private-state vectors.
//! [`river`] solves one street; [`streets`] solves a flop, turn or river tree by
//! expanding each street transition into one subtree per dealt card.
//! All numerical storage and measurements use f64. Correlated ranges, multiway
//! solving, rake, and tournament equity are outside this phase's contract.

mod allocation;
pub mod best_response;
pub mod cfr;
pub mod config;
pub mod error;
pub mod game;
mod memory;
pub mod progress;
pub mod river;
pub mod solver;
pub mod strategy;
pub mod streets;
pub mod terminal;
mod traversal;

pub use best_response::{Exploitability, best_response, expected_value, exploitability};
pub use cfr::{Cfr, Variant};
pub use config::{DcfrParams, Precision, SolveConfig, SolverConfig};
pub use error::SolveError;
pub use game::{Game, NodeId, NodeKind, Real};
pub use progress::Progress;
pub use river::{DecisionValues, RiverGame, RiverMemory, RiverSolver, RiverStrategy};
pub use solver::{SolveReport, Solver, StopReason, solve};
pub use strategy::Strategy;
pub use streets::{
    PostflopDecisionValues, PostflopGame, PostflopMemory, PostflopNodeView, PostflopOptions,
    PostflopSolver, PostflopStrategy,
};

#[cfg(test)]
mod tests;
