//! The three things every owned hold'em game does to its two ranges before it
//! builds anything.
//!
//! The river module and the street-aware module both take a pair of ranges and a
//! set of dead board cards, scale each range, check that no compatible pair's
//! product underflows, and compute the root normaliser the values divide by.
//! They used to hold a copy each, which is one place for the two to drift apart
//! on a rule that decides what every number in a solve is relative to.

use crate::{SolveError, allocation::collect, terminal::evaluate_fold};
use cards::{CardSet, Combo, Range};

/// Combos per player, which is what every vector here is indexed by.
const STATES: usize = 1326;

/// Board-filtered inclusion weights, divided by the range's own maximum.
///
/// Scaling by a constant changes no strategy: it divides out of every reach and
/// every value alike. It is done so a range written with small weights cannot
/// underflow a compatible pair's product.
pub(crate) fn scaled(range: &Range, dead: CardSet) -> Result<Vec<f64>, SolveError> {
    let mut result = collect(range.weights().iter().copied())?;
    for combo in Combo::all() {
        if combo.mask() & dead.bits() != 0 {
            result[usize::from(combo.id())] = 0.0;
        }
    }
    let maximum = result.iter().copied().fold(0.0_f64, f64::max);
    if maximum == 0.0 {
        return Err(SolveError::EmptyGame);
    }
    for value in &mut result {
        *value /= maximum;
    }
    Ok(result)
}

/// Refuses a pair of ranges holding a positive compatible product that `f64`
/// multiplication cannot represent.
///
/// One check on the known board, not one per runout: a runout only removes
/// combos, so a product that survives here survives everywhere below.
pub(crate) fn check_pair_underflow(weights: &[Vec<f64>; 2]) -> Result<(), SolveError> {
    let combos: Vec<_> = collect(Combo::all())?;
    for (a, &wa) in combos.iter().zip(&weights[0]) {
        if wa == 0.0 {
            continue;
        }
        for (b, &wb) in combos.iter().zip(&weights[1]) {
            if wb > 0.0 && a.mask() & b.mask() == 0 && wa * wb == 0.0 {
                return Err(SolveError::InvalidGame(
                    "positive compatible pair weight underflows".into(),
                ));
            }
        }
    }
    Ok(())
}

/// The surviving joint mass of the two scaled ranges, which every reported
/// value is divided by.
pub(crate) fn root_normalizer(weights: &[Vec<f64>; 2], dead: CardSet) -> Result<f64, SolveError> {
    let mut opposing_mass = [0.0; STATES];
    let opposing: &[f64; STATES] = weights[1]
        .as_slice()
        .try_into()
        .map_err(|_| SolveError::InvalidGame("a range is 1326 combos wide".into()))?;
    evaluate_fold(dead, opposing, 1.0, &mut opposing_mass)
        .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
    let normalizer: f64 = weights[0]
        .iter()
        .zip(opposing_mass)
        .map(|(a, b)| a * b)
        .sum();
    if !normalizer.is_finite() || normalizer <= 0.0 {
        return Err(SolveError::EmptyGame);
    }
    Ok(normalizer)
}
