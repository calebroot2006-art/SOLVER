//! Errors returned before a failed solve can become strategy evidence.
use crate::NodeId;

/// A malformed game, configuration, or non-finite result.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SolveError {
    /// Terminal evaluation or arithmetic produced an invalid number.
    #[error("non-finite value at iteration {iteration}, node {node}, player {player}")]
    NonFinite {
        /// Attempted one-based iteration, or zero for independent evaluation.
        iteration: u64,
        /// Public node where the failure was observed.
        node: NodeId,
        /// Zero-based affected player.
        player: usize,
    },
    /// No finite positive mass remains over compatible private deals.
    #[error("game has no finite positive compatible deal weight")]
    EmptyGame,
    /// Invalid tree, probability, strategy, or game-use contract.
    #[error("invalid game: {0}")]
    InvalidGame(String),
    /// Missing or invalid configuration value.
    #[error("invalid config: {0}")]
    Config(String),
}

pub(crate) fn finite(
    values: &[f64],
    iteration: u64,
    node: NodeId,
    player: usize,
) -> Result<(), SolveError> {
    if values.iter().any(|v| !v.is_finite()) {
        Err(SolveError::NonFinite {
            iteration,
            node,
            player,
        })
    } else {
        Ok(())
    }
}
