//! Checked hold'em terminal values in stable 1326-combo order.
//!
//! Inputs are opponent counterfactual reach, already including their range,
//! actions, and chance factors. These functions neither normalize nor apply the
//! hero's range. Public output arrays are changed only after complete success.
//! Full hold'em game-tree integration is a separate concern.

mod mass;
mod showdown;

use cards::{CardError, CardSet, Combo};
use std::fmt;

pub use showdown::{ShowdownScratch, ShowdownTable};

use mass::{Bucket, weighted_value};

/// A rejected terminal input or an unrepresentable numerical result.
#[derive(Debug)]
pub enum TerminalError {
    /// An invalid or repeated card.
    Card(CardError),
    /// A negative or nonfinite opponent reach at this combo ID.
    InvalidReach(u16),
    /// A nonfinite terminal utility.
    InvalidUtility,
    /// Arithmetic cannot produce a checked finite result.
    Arithmetic(&'static str),
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Card(error) => write!(f, "invalid terminal cards: {error}"),
            Self::InvalidReach(id) => write!(f, "reach for combo {id} must be finite and nonnegative"),
            Self::InvalidUtility => f.write_str("terminal utilities must be finite"),
            Self::Arithmetic(message) => write!(f, "terminal arithmetic: {message}"),
        }
    }
}

impl std::error::Error for TerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Card(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CardError> for TerminalError {
    fn from(error: CardError) -> Self {
        Self::Card(error)
    }
}

/// Finite utility for each of a hero's three showdown outcomes.
#[derive(Clone, Copy, Debug)]
pub struct OutcomeUtilities {
    win: f64,
    tie: f64,
    loss: f64,
}

impl OutcomeUtilities {
    /// Checks all three utilities; these are net payoffs, not pot shares.
    pub fn new(win: f64, tie: f64, loss: f64) -> Result<Self, TerminalError> {
        if [win, tie, loss].iter().any(|value| !value.is_finite()) {
            return Err(TerminalError::InvalidUtility);
        }
        Ok(Self { win, tie, loss })
    }

    fn values(self) -> [f64; 3] {
        [self.win, self.tie, self.loss]
    }
}

fn validate_reach(reach: &[f64; 1326]) -> Result<(), TerminalError> {
    for (id, value) in reach.iter().enumerate() {
        if !value.is_finite() || *value < 0.0 {
            return Err(TerminalError::InvalidReach(id as u16));
        }
    }
    Ok(())
}

/// Weights a fixed fold utility by compatible opponent reach.
///
/// `dead` may contain any checked set of cards, including an empty set. Both
/// players' blocked combos are excluded. No reach factors are added here.
/// Negative/nonfinite reach and unrepresentable arithmetic return an error and
/// leave `output` unchanged. Finite signed zero reach is accepted.
pub fn evaluate_fold(
    dead: CardSet,
    opponent_reach: &[f64; 1326],
    utility: f64,
    output: &mut [f64; 1326],
) -> Result<(), TerminalError> {
    if !utility.is_finite() {
        return Err(TerminalError::InvalidUtility);
    }
    validate_reach(opponent_reach)?;
    let mut bucket = Bucket::default();
    for combo in Combo::all() {
        if combo.mask() & dead.bits() == 0 {
            bucket.add(combo, opponent_reach[usize::from(combo.id())])?;
        }
    }
    let mut result = [0.0; 1326];
    for combo in Combo::all() {
        if combo.mask() & dead.bits() == 0 {
            let id = usize::from(combo.id());
            let mass = bucket.compatible(combo, opponent_reach[id])?;
            result[id] = weighted_value([mass, 0.0, 0.0], [utility, 0.0, 0.0])?;
        }
    }
    output.copy_from_slice(&result);
    Ok(())
}
