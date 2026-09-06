//! Two-player zero-sum CFR over public trees and fixed private-state vectors.
//! All numerical storage and measurements use f64. Correlated ranges, multiway
//! solving, rake, and tournament equity are outside this phase's contract.

pub mod best_response;
pub mod cfr;
pub mod config;
pub mod error;
pub mod game;
pub mod progress;
pub mod solver;
pub mod strategy;
pub mod terminal;
mod traversal;

pub use best_response::{Exploitability, best_response, expected_value, exploitability};
pub use cfr::{Cfr, Variant};
pub use config::{DcfrParams, SolveConfig, SolverConfig};
pub use error::SolveError;
pub use game::{Game, NodeId, NodeKind, Real};
pub use progress::Progress;
pub use solver::{SolveReport, Solver, StopReason, solve};
pub use strategy::Strategy;

#[cfg(test)]
mod tests;
