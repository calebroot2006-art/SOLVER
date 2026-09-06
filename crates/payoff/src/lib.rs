//! Checked terminal chip utilities and the phase 1 scalar type.
use std::fmt;

/// Full precision for storage, accumulation, and accuracy measurements.
pub type Real = f64;

/// Converts committed chips and pot shares into terminal utilities.
/// Phase 1 requires two-player zero-sum payoffs. Rake and tournament equity need
/// separately reviewed metrics.
pub trait Payoff {
    /// Writes each net utility. Invalid inputs write NaN to every output so a
    /// terminal evaluator propagates a checked solver failure.
    fn utilities(&self, stacks_before: &[Real], contributions: &[Real], shares: &[Real], out: &mut [Real]);
}

/// Invalid dimensions or numbers in a payoff request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayoffError(pub String);

impl fmt::Display for PayoffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for PayoffError {}

/// Net chips won: the player's pot allocation minus their contribution.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChipEv;

impl ChipEv {
    /// Checks every input before changing `out`.
    /// Stacks are validated but do not affect chip utility. All vectors must
    /// share one positive length; shares must sum to one.
    pub fn try_utilities(&self, stacks: &[Real], contributions: &[Real], shares: &[Real], out: &mut [Real]) -> Result<(), PayoffError> {
        let n = contributions.len();
        if n == 0 || stacks.len() != n || shares.len() != n || out.len() != n {
            return Err(PayoffError("payoff vector lengths must agree and be nonzero".into()));
        }
        for (name, values) in [("stacks_before", stacks), ("contributions", contributions), ("shares", shares)] {
            if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(PayoffError(format!("{name} must be finite and nonnegative")));
            }
        }
        if shares.iter().any(|v| *v > 1.0) || (shares.iter().sum::<Real>() - 1.0).abs() > 16.0 * Real::EPSILON * n as Real {
            return Err(PayoffError("shares must be probabilities summing to one".into()));
        }
        let pot: Real = contributions.iter().sum();
        if !pot.is_finite() { return Err(PayoffError("total pot must be finite".into())); }
        for ((value, share), contribution) in out.iter_mut().zip(shares).zip(contributions) {
            *value = share * pot - contribution;
        }
        Ok(())
    }
}

impl Payoff for ChipEv {
    fn utilities(&self, stacks: &[Real], contributions: &[Real], shares: &[Real], out: &mut [Real]) {
        if self.try_utilities(stacks, contributions, shares, out).is_err() { out.fill(Real::NAN); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folds_chops_and_splits_conserve_chips() {
        let mut out = [0.0; 2];
        ChipEv.try_utilities(&[10.0; 2], &[2.0, 1.0], &[1.0, 0.0], &mut out).unwrap();
        assert_eq!(out, [1.0, -1.0]);
        ChipEv.try_utilities(&[10.0; 2], &[2.0; 2], &[0.5; 2], &mut out).unwrap();
        assert_eq!(out, [0.0; 2]);
        for split in 0..=100 {
            let share = split as Real / 100.0;
            ChipEv.try_utilities(&[20.0; 2], &[7.0, 11.0], &[share, 1.0 - share], &mut out).unwrap();
            assert!((out[0] + out[1]).abs() < 1e-13);
        }
    }
    #[test]
    fn invalid_inputs_never_return_plausible_utilities() {
        let mut out = [123.0; 2];
        assert!(ChipEv.try_utilities(&[1.0; 2], &[Real::NAN, 1.0], &[0.5; 2], &mut out).is_err());
        assert_eq!(out, [123.0; 2]);
        ChipEv.utilities(&[1.0; 2], &[1.0; 2], &[0.0; 2], &mut out);
        assert!(out.iter().all(|v| v.is_nan()));
        assert!(ChipEv.try_utilities(&[1.0; 2], &[1.0; 2], &[0.5], &mut out).is_err());
    }
}