//! Errors returned before a failed solve can become strategy evidence.
use crate::NodeId;

/// A malformed game, configuration, or non-finite result.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SolveError {
    /// A positive reach could not be represented through the requested operation.
    #[error("arithmetic failure at iteration {iteration}, node {node}, player {player}: {reason}")]
    Arithmetic {
        /// Attempted iteration, or zero for independent measurement.
        iteration: u64,
        /// Public node where the operation failed.
        node: NodeId,
        /// Player whose reach is affected.
        player: usize,
        /// Failed arithmetic operation.
        reason: &'static str,
    },
    /// A requested large buffer could not be allocated.
    #[error("solver allocation failed: {0}")]
    Allocation(String),
    /// The conservative working-set estimate exceeds the configured byte budget.
    #[error("solver needs at most {required} bytes, exceeding the {limit}-byte memory limit")]
    MemoryLimit {
        /// Conservative required byte budget.
        required: usize,
        /// Configured maximum bytes.
        limit: usize,
    },
    /// A checked concrete terminal rejected input or numerical arithmetic.
    #[error("terminal failure at iteration {iteration}, node {node}, player {player}: {reason}")]
    Terminal {
        /// Attempted iteration, or zero for independent measurement.
        iteration: u64,
        /// Public terminal node.
        node: NodeId,
        /// Affected player.
        player: usize,
        /// Checked evaluator diagnostic.
        reason: String,
    },
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

pub(crate) fn reach_product(a: f64, b: f64, checked: bool, iteration: u64, node: NodeId, player: usize) -> Result<f64, SolveError> {
    let value = a * b;
    if checked && a > 0.0 && b > 0.0 && value == 0.0 {
        return Err(SolveError::Arithmetic { iteration, node, player, reason: "positive reach underflow" });
    }
    Ok(value)
}

pub(crate) fn weighted_product(value: f64, weight: f64, checked: bool, iteration: u64, node: NodeId, player: usize) -> Result<f64, SolveError> {
    let product = value * weight;
    if checked && value != 0.0 && weight > 0.0 && product == 0.0 {
        return Err(SolveError::Arithmetic { iteration, node, player, reason: "nonzero weighted value underflow" });
    }
    Ok(product)
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

// (a + b) / (1 + |a| + |b|), evaluated without overflowing the sum or scale.
// Callers must first establish that both values are finite.
pub(crate) fn normalized_sum(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs()).max(1.0);
    let left = a / scale;
    let right = b / scale;
    (left + right) / (1.0 / scale + left.abs() + right.abs())
}
