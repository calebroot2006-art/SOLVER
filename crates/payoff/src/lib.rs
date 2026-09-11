//! Checked chip utilities, validated payouts and an explicit unsupported ICM model.
use std::fmt;

/// Full precision for storage, accumulation, and accuracy measurements.
pub type Real = f64;

/// Converts committed chips and pot shares into terminal utilities.
/// Phase 1 requires two-player zero-sum payoffs. Rake and tournament equity need
/// separately reviewed metrics.
pub trait Payoff {
    /// Writes each net utility. Invalid inputs write NaN to every output so a
    /// terminal evaluator propagates a checked solver failure.
    fn utilities(
        &self,
        stacks_before: &[Real],
        contributions: &[Real],
        shares: &[Real],
        out: &mut [Real],
    );
}

/// Invalid payoff data or an unsupported payoff operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayoffError(pub String);

impl fmt::Display for PayoffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PayoffError {}

/// Net chips won: the player's pot allocation minus their contribution.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChipEv;

impl ChipEv {
    /// Checks every input before changing `out`.
    /// Stacks are validated but do not affect chip utility. All vectors must
    /// share one positive length; shares must sum to one.
    pub fn try_utilities(
        &self,
        stacks: &[Real],
        contributions: &[Real],
        shares: &[Real],
        out: &mut [Real],
    ) -> Result<(), PayoffError> {
        let n = contributions.len();
        if n == 0 || stacks.len() != n || shares.len() != n || out.len() != n {
            return Err(PayoffError(
                "payoff vector lengths must agree and be nonzero".into(),
            ));
        }
        for (name, values) in [
            ("stacks_before", stacks),
            ("contributions", contributions),
            ("shares", shares),
        ] {
            if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(PayoffError(format!(
                    "{name} must be finite and nonnegative"
                )));
            }
        }
        if shares.iter().any(|v| *v > 1.0)
            || (shares.iter().sum::<Real>() - 1.0).abs() > 16.0 * Real::EPSILON * n as Real
        {
            return Err(PayoffError(
                "shares must be probabilities summing to one".into(),
            ));
        }
        let pot: Real = contributions.iter().sum();
        if !pot.is_finite() {
            return Err(PayoffError("total pot must be finite".into()));
        }
        for ((value, share), contribution) in out.iter_mut().zip(shares).zip(contributions) {
            *value = share * pot - contribution;
        }
        Ok(())
    }
}

impl Payoff for ChipEv {
    fn utilities(
        &self,
        stacks: &[Real],
        contributions: &[Real],
        shares: &[Real],
        out: &mut [Real],
    ) {
        if self
            .try_utilities(stacks, contributions, shares, out)
            .is_err()
        {
            out.fill(Real::NAN);
        }
    }
}

/// Tournament payouts in finishing-place order, starting with first place.
///
/// At least one place must be named. Every amount must be finite and
/// nonnegative, and later places cannot pay more than earlier places. Equal
/// payouts and zeros, including an all-zero structure, are preserved. This type
/// imposes no seat-count limit and does not calculate tournament equity.
#[derive(Clone, Debug, PartialEq)]
pub struct PayoutStructure {
    amounts: Vec<Real>,
}

impl PayoutStructure {
    /// Validates explicit payouts without adding or removing finishing places.
    pub fn new(amounts: Vec<Real>) -> Result<Self, PayoffError> {
        if amounts.is_empty() {
            return Err(PayoffError(
                "payout structure must name at least one place".into(),
            ));
        }
        if amounts
            .iter()
            .any(|amount| !amount.is_finite() || *amount < 0.0)
        {
            return Err(PayoffError(
                "payout amounts must be finite and nonnegative".into(),
            ));
        }
        if amounts.windows(2).any(|pair| pair[1] > pair[0]) {
            return Err(PayoffError(
                "payout amounts must be non-increasing by finishing place".into(),
            ));
        }
        Ok(Self { amounts })
    }

    /// Read-only payouts, from first place onward, exactly as supplied.
    #[must_use]
    pub fn amounts(&self) -> &[Real] {
        &self.amounts
    }
}

/// Explicit placeholder for tournament equity, unsupported in phase 6.
///
/// Construction records validated payouts only. It supplies no equity formula:
/// checked requests return an error and the Payoff hook fills output with NaN.
#[derive(Clone, Debug, PartialEq)]
pub struct Icm {
    payouts: PayoutStructure,
}

impl Icm {
    /// Records an explicitly selected, validated payout structure.
    #[must_use]
    pub fn new(payouts: PayoutStructure) -> Self {
        Self { payouts }
    }

    /// The payout structure selected when this model was constructed.
    #[must_use]
    pub fn payouts(&self) -> &PayoutStructure {
        &self.payouts
    }

    /// Refuses every request with the named unsupported error, preserving out.
    /// Request dimensions and values do not change this phase 6 refusal.
    pub fn try_utilities(
        &self,
        _stacks: &[Real],
        _contributions: &[Real],
        _shares: &[Real],
        _out: &mut [Real],
    ) -> Result<(), PayoffError> {
        Err(PayoffError("ICM not implemented in phase 6".into()))
    }
}

impl Payoff for Icm {
    fn utilities(
        &self,
        _stacks_before: &[Real],
        _contributions: &[Real],
        _shares: &[Real],
        out: &mut [Real],
    ) {
        out.fill(Real::NAN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folds_chops_and_splits_conserve_chips() {
        let mut out = [0.0; 2];
        ChipEv
            .try_utilities(&[10.0; 2], &[2.0, 1.0], &[1.0, 0.0], &mut out)
            .unwrap();
        assert_eq!(out, [1.0, -1.0]);
        ChipEv
            .try_utilities(&[10.0; 2], &[2.0; 2], &[0.5; 2], &mut out)
            .unwrap();
        assert_eq!(out, [0.0; 2]);
        for split in 0..=100 {
            let share = split as Real / 100.0;
            ChipEv
                .try_utilities(&[20.0; 2], &[7.0, 11.0], &[share, 1.0 - share], &mut out)
                .unwrap();
            assert!((out[0] + out[1]).abs() < 1e-13);
        }
    }
    #[test]
    fn invalid_inputs_never_return_plausible_utilities() {
        let mut out = [123.0; 2];
        assert!(
            ChipEv
                .try_utilities(&[1.0; 2], &[Real::NAN, 1.0], &[0.5; 2], &mut out)
                .is_err()
        );
        assert_eq!(out, [123.0; 2]);
        ChipEv.utilities(&[1.0; 2], &[1.0; 2], &[0.0; 2], &mut out);
        assert!(out.iter().all(|v| v.is_nan()));
        assert!(
            ChipEv
                .try_utilities(&[1.0; 2], &[1.0; 2], &[0.5], &mut out)
                .is_err()
        );
    }
    #[test]
    fn payout_structure_rejects_missing_nonfinite_negative_and_increasing_amounts() {
        for amounts in [
            vec![],
            vec![Real::NAN],
            vec![Real::INFINITY],
            vec![Real::NEG_INFINITY],
            vec![-1.0],
            vec![10.0, -0.01],
            vec![10.0, 11.0],
            vec![10.0, 1.0, 2.0],
        ] {
            assert!(
                PayoutStructure::new(amounts.clone()).is_err(),
                "accepted {amounts:?}"
            );
        }
    }

    #[test]
    fn payout_structure_preserves_ties_zeros_and_explicit_place_count() {
        for amounts in [
            vec![100.0],
            vec![100.0, 50.0, 50.0, 0.0, -0.0],
            vec![0.0; 32],
            vec![Real::MAX, Real::MAX],
        ] {
            let payouts = PayoutStructure::new(amounts.clone()).unwrap();
            assert_eq!(
                payouts
                    .amounts()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                amounts.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            let icm = Icm::new(payouts.clone());
            assert_eq!(icm.payouts(), &payouts);
        }
    }

    #[test]
    fn icm_always_refuses_without_checked_mutation_and_poisons_unchecked_output() {
        let icm = Icm::new(PayoutStructure::new(vec![100.0, 50.0, 0.0]).unwrap());
        type Request<'a> = (&'a [Real], &'a [Real], &'a [Real], usize);
        let cases: [Request<'_>; 6] = [
            (&[10.0; 2], &[2.0; 2], &[0.5; 2], 2),
            (&[], &[], &[], 3),
            (&[10.0; 3], &[2.0], &[0.5; 2], 4),
            (&[-1.0, Real::INFINITY], &[Real::NAN], &[2.0], 1),
            (&[], &[], &[], 0),
            (&[10.0; 2], &[2.0; 2], &[0.5; 2], 0),
        ];
        for (case, (stacks, contributions, shares, length)) in cases.into_iter().enumerate() {
            let sentinels = [123.0, -0.0, Real::from_bits(0x7ff8_0000_0000_0001)];
            let mut out: Vec<_> = (0..length)
                .map(|i| sentinels[i % sentinels.len()])
                .collect();
            let before: Vec<_> = out.iter().map(|v| v.to_bits()).collect();
            assert_eq!(
                icm.try_utilities(stacks, contributions, shares, &mut out),
                Err(PayoffError("ICM not implemented in phase 6".into())),
                "request {case}"
            );
            assert_eq!(
                out.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                before,
                "checked request {case} changed output"
            );
            let payoff: &dyn Payoff = &icm;
            payoff.utilities(stacks, contributions, shares, &mut out);
            assert!(
                out.iter().all(|v| v.is_nan()),
                "unchecked request {case} left finite output"
            );
        }
    }
}
