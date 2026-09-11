//! Allocation-free quantization of probabilities, optional EVs and reach.
//!
//! Errors describe representation, not solver accuracy. Slice outputs are
//! caller-owned and remain unchanged on every error. Probability quantization
//! uses fixed stack scratch; EV quantization validates in passes with scalar
//! scratch. Callers must account for their input, output and scratch storage.

use std::fmt;

/// Maximum actions in one probability row.
pub const MAX_ACTIONS: usize = 255;
/// Maximum optional EV entries in one node (1326 combos by 255 actions).
pub const MAX_EV_ENTRIES: usize = 1326 * MAX_ACTIONS;
/// Integer units representing probability one.
pub const PROBABILITY_TOTAL: u16 = u16::MAX;

struct ProbabilityScratch {
    codes: [u16; MAX_ACTIONS],
    remainders: [f64; MAX_ACTIONS],
}

/// Size of the fixed probability scratch object, excluding ordinary scalar
/// call-frame state. EV and reach quantization use only scalar scratch.
pub const PROBABILITY_SCRATCH_BYTES: usize = std::mem::size_of::<ProbabilityScratch>();

/// A refused quantization, with static messages and no owned allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantizeError {
    /// A probability row has no actions or exceeds 255 actions.
    ProbabilityLength,
    /// The caller's output length differs from the input length.
    OutputLength,
    /// A probability is not finite or is outside [0, 1].
    ProbabilityValue,
    /// Probabilities do not sum to one within the specified summation allowance.
    ProbabilitySum,
    /// Exact integer total or per-entry probability error could not be met.
    ProbabilityBound,
    /// An EV block exceeds 1326 * 255 entries.
    EvLength,
    /// A present EV is not finite.
    EvValue,
    /// The required EV scale exceeds finite f32 storage.
    EvScaleOverflow,
    /// A nonzero required EV scale casts to zero.
    EvScaleUnderflow,
    /// An EV code would exceed [-32767, 32767].
    EvRange,
    /// A decoded EV differs from its source by more than half the stored scale.
    EvBound,
    /// Reach is not finite or is outside [0, 1].
    ReachValue,
    /// Positive reach would become zero in f32.
    ReachUnderflow,
}

impl fmt::Display for QuantizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProbabilityLength => "probability row must have 1..255 actions",
            Self::OutputLength => "quantization input and output lengths must agree",
            Self::ProbabilityValue => "probabilities must be finite and in [0, 1]",
            Self::ProbabilitySum => {
                "probabilities do not sum to one within the summation allowance"
            }
            Self::ProbabilityBound => {
                "probability quantization cannot meet its exact total and error bound"
            }
            Self::EvLength => "EV block exceeds 1326 * 255 entries",
            Self::EvValue => "present EVs must be finite",
            Self::EvScaleOverflow => "EV scale cannot be stored as a finite f32",
            Self::EvScaleUnderflow => "nonzero EV scale would become zero in f32",
            Self::EvRange => "EV quantization would exceed [-32767, 32767]",
            Self::EvBound => "EV quantization error exceeds half the stored scale",
            Self::ReachValue => "reach must be finite and in [0, 1]",
            Self::ReachUnderflow => "positive reach would become zero in f32",
        })
    }
}

impl std::error::Error for QuantizeError {}

/// Quantize one probability row by largest remainder, breaking equal remainders
/// in original action order. Returns the measured maximum absolute error.
///
/// The source must sum to one within `8 * f64::EPSILON * action_count`.
/// Values are not renormalized. The integer sum must be 65535 and every decoded
/// value must differ from the original input by at most `1 / 65535`.
pub fn quantize_probabilities(input: &[f64], out: &mut [u16]) -> Result<f64, QuantizeError> {
    let count = input.len();
    if !(1..=MAX_ACTIONS).contains(&count) {
        return Err(QuantizeError::ProbabilityLength);
    }
    if out.len() != count {
        return Err(QuantizeError::OutputLength);
    }
    if input
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(QuantizeError::ProbabilityValue);
    }
    let sum: f64 = input.iter().sum();
    if (sum - 1.0).abs() > 8.0 * f64::EPSILON * count as f64 {
        return Err(QuantizeError::ProbabilitySum);
    }
    let denominator = f64::from(PROBABILITY_TOTAL);
    let mut scratch = ProbabilityScratch {
        codes: [0; MAX_ACTIONS],
        remainders: [0.0; MAX_ACTIONS],
    };
    let mut total = 0_u32;
    for (index, value) in input.iter().enumerate() {
        let scaled = value * denominator;
        let floor = scaled.floor();
        // The finite [0, 1] input makes this cast exact and in range.
        scratch.codes[index] = floor as u16;
        scratch.remainders[index] = scaled - floor;
        total += u32::from(scratch.codes[index]);
    }
    let remaining = u32::from(PROBABILITY_TOTAL)
        .checked_sub(total)
        .filter(|remaining| *remaining <= count as u32)
        .ok_or(QuantizeError::ProbabilityBound)?;
    for _ in 0..remaining {
        let mut largest = 0;
        for index in 1..count {
            if scratch.remainders[index] > scratch.remainders[largest] {
                largest = index;
            }
        }
        scratch.codes[largest] = scratch.codes[largest]
            .checked_add(1)
            .ok_or(QuantizeError::ProbabilityBound)?;
        scratch.remainders[largest] = -1.0;
    }
    let mut max_error = 0.0_f64;
    let mut checked_total = 0_u32;
    for (original, code) in input.iter().zip(&scratch.codes) {
        let error = (f64::from(*code) / denominator - original).abs();
        if error > 1.0 / denominator {
            return Err(QuantizeError::ProbabilityBound);
        }
        max_error = max_error.max(error);
        checked_total += u32::from(*code);
    }
    if checked_total != u32::from(PROBABILITY_TOTAL) {
        return Err(QuantizeError::ProbabilityBound);
    }
    out.copy_from_slice(&scratch.codes[..count]);
    Ok(max_error)
}

/// Stored EV scale and the measured maximum error against the original inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvQuantization {
    /// Decode a present code as `f64::from(code) * f64::from(scale)`.
    pub scale: f32,
    /// Representation error only; missing entries contribute no error.
    pub max_absolute_error: f64,
}

/// Quantize optional EVs to signed codes without clamping. Missing entries stay
/// missing. Empty, all-missing and all-zero blocks use scale one and error zero.
///
/// The scale is the smallest positive finite f32 at least `max_abs / 32767`.
/// A nonzero scale that casts to zero refuses. Codes round to nearest, with
/// ties away from zero. Every decoded error is checked against half the stored
/// scale before any output changes; no extra floating-point allowance is added.
pub fn quantize_evs(
    input: &[Option<f64>],
    out: &mut [Option<i16>],
) -> Result<EvQuantization, QuantizeError> {
    if input.len() > MAX_EV_ENTRIES {
        return Err(QuantizeError::EvLength);
    }
    if input.len() != out.len() {
        return Err(QuantizeError::OutputLength);
    }
    let mut max_abs = 0.0_f64;
    for value in input.iter().flatten() {
        if !value.is_finite() {
            return Err(QuantizeError::EvValue);
        }
        max_abs = max_abs.max(value.abs());
    }
    let scale = ev_scale(max_abs)?;
    let scale64 = f64::from(scale);
    let mut max_error = 0.0_f64;
    for value in input.iter().flatten() {
        let code = (value / scale64).round();
        if !(-32767.0..=32767.0).contains(&code) {
            return Err(QuantizeError::EvRange);
        }
        let error = (code * scale64 - value).abs();
        if error > scale64 * 0.5 {
            return Err(QuantizeError::EvBound);
        }
        max_error = max_error.max(error);
    }
    // All entries passed above. No fallible operation follows the first write.
    // The repeated conversion is deterministic and its cast is already in range.
    for (value, code) in input.iter().zip(out) {
        *code = value.map(|value| (value / scale64).round() as i16);
    }
    Ok(EvQuantization {
        scale,
        max_absolute_error: max_error,
    })
}

fn ev_scale(max_abs: f64) -> Result<f32, QuantizeError> {
    if max_abs == 0.0 {
        return Ok(1.0);
    }
    let mut scale = (max_abs / 32767.0) as f32;
    if scale == 0.0 {
        return Err(QuantizeError::EvScaleUnderflow);
    }
    if !scale.is_finite() {
        return Err(QuantizeError::EvScaleOverflow);
    }
    // The product of f32 scale and this 15-bit integer is exact in f64.
    // Comparing products also catches a quotient rounded down onto an f32 value.
    if f64::from(scale) * 32767.0 < max_abs {
        scale = f32::from_bits(scale.to_bits() + 1);
    }
    if !scale.is_finite() {
        return Err(QuantizeError::EvScaleOverflow);
    }
    Ok(scale)
}

/// One stored reach value and its measured representation error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuantizedReach {
    /// The finite f32 value in [0, 1].
    pub value: f32,
    /// Absolute difference from the original f64 value, not a solver error bound.
    pub absolute_error: f64,
}

/// Convert reach to f32, refusing invalid values and positive-to-zero underflow.
/// A zero reach is a present numerical value, distinct from a missing EV.
pub fn quantize_reach(input: f64) -> Result<QuantizedReach, QuantizeError> {
    if !input.is_finite() || !(0.0..=1.0).contains(&input) {
        return Err(QuantizeError::ReachValue);
    }
    let value = input as f32;
    if input > 0.0 && value == 0.0 {
        return Err(QuantizeError::ReachUnderflow);
    }
    Ok(QuantizedReach {
        value,
        absolute_error: (f64::from(value) - input).abs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_rows_have_exact_totals_and_stable_remainder_ties() {
        println!("probability scratch object: {PROBABILITY_SCRATCH_BYTES} bytes");
        for (input, expected) in [
            (vec![1.0], vec![65535]),
            (vec![0.5, 0.5], vec![32768, 32767]),
            (vec![0.25; 4], vec![16384, 16384, 16384, 16383]),
            (vec![0.0, 0.5, 0.5], vec![0, 32768, 32767]),
            (vec![0.1, 0.2, 0.7], vec![6554, 13107, 45874]),
            (vec![0.01, 0.09, 0.9], vec![655, 5898, 58982]),
            (vec![1.0 / 255.0; 255], vec![257; 255]),
        ] {
            let mut out = vec![123; input.len()];
            let error = quantize_probabilities(&input, &mut out).unwrap();
            assert_eq!(out, expected);
            assert_eq!(out.iter().map(|v| u32::from(*v)).sum::<u32>(), 65535);
            assert_eq!(error, probability_error(&input, &out));
            assert!(error <= 1.0 / 65535.0);
        }
    }

    fn probability_error(input: &[f64], codes: &[u16]) -> f64 {
        input
            .iter()
            .zip(codes)
            .map(|(p, q)| (p - f64::from(*q) / 65535.0).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn probability_validation_and_summation_allowance_preserve_failed_outputs() {
        for (input, expected) in [
            (vec![], QuantizeError::ProbabilityLength),
            (vec![1.0 / 256.0; 256], QuantizeError::ProbabilityLength),
            (vec![f64::NAN], QuantizeError::ProbabilityValue),
            (vec![f64::INFINITY], QuantizeError::ProbabilityValue),
            (vec![-f64::EPSILON, 1.0], QuantizeError::ProbabilityValue),
            (vec![1.0 + f64::EPSILON], QuantizeError::ProbabilityValue),
            (vec![0.1, 0.1], QuantizeError::ProbabilitySum),
            (vec![0.6, 0.6], QuantizeError::ProbabilitySum),
            (
                vec![1.0 - 9.0 * f64::EPSILON],
                QuantizeError::ProbabilitySum,
            ),
            (
                vec![0.5, 0.5 + 17.0 * f64::EPSILON],
                QuantizeError::ProbabilitySum,
            ),
        ] {
            let mut out = vec![12345; input.len().max(1)];
            let before = out.clone();
            assert_eq!(
                quantize_probabilities(&input, &mut out),
                Err(expected),
                "{input:?}"
            );
            assert_eq!(out, before);
        }
        let mut out = [23456; 3];
        assert_eq!(
            quantize_probabilities(&[0.5; 2], &mut out),
            Err(QuantizeError::OutputLength)
        );
        assert_eq!(out, [23456; 3]);
        for input in [
            vec![1.0 - 8.0 * f64::EPSILON],
            vec![0.5, 0.5 + 16.0 * f64::EPSILON],
        ] {
            let mut out = vec![0; input.len()];
            let measured = quantize_probabilities(&input, &mut out).unwrap();
            assert_eq!(measured, probability_error(&input, &out));
            assert_eq!(out.iter().map(|v| u32::from(*v)).sum::<u32>(), 65535);
        }
    }

    #[test]
    fn deterministic_generated_rows_obey_original_input_error_bounds() {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for count in 1..=255 {
            for _ in 0..4 {
                let mut weights = Vec::with_capacity(count);
                for _ in 0..count {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1);
                    weights.push(((state >> 32) % 1024) as f64);
                }
                weights[0] += 1.0;
                let total: f64 = weights.iter().sum();
                let input: Vec<_> = weights.iter().map(|weight| weight / total).collect();
                let mut first = vec![0; count];
                let mut second = vec![0; count];
                let report = quantize_probabilities(&input, &mut first).unwrap();
                assert_eq!(quantize_probabilities(&input, &mut second).unwrap(), report);
                assert_eq!(first, second);
                assert_eq!(first.iter().map(|v| u32::from(*v)).sum::<u32>(), 65535);
                assert_eq!(report, probability_error(&input, &first));
                assert!(report <= 1.0 / 65535.0);
            }
        }
    }

    #[test]
    fn ev_codes_preserve_missing_and_round_half_steps_away_from_zero() {
        let input = [
            Some(32767.0),
            Some(-32767.0),
            Some(0.5),
            Some(-0.5),
            Some(1.5),
            Some(-1.5),
            None,
            Some(0.0),
            Some(-0.0),
        ];
        let mut out = [None; 9];
        let report = quantize_evs(&input, &mut out).unwrap();
        assert_eq!(
            report,
            EvQuantization {
                scale: 1.0,
                max_absolute_error: 0.5
            }
        );
        assert_eq!(
            out,
            [
                Some(32767),
                Some(-32767),
                Some(1),
                Some(-1),
                Some(2),
                Some(-2),
                None,
                Some(0),
                Some(0)
            ]
        );
        for input in [vec![], vec![None, None], vec![Some(0.0), None, Some(-0.0)]] {
            let mut out = vec![Some(99); input.len()];
            assert_eq!(
                quantize_evs(&input, &mut out).unwrap(),
                EvQuantization {
                    scale: 1.0,
                    max_absolute_error: 0.0
                }
            );
            assert_eq!(
                out,
                input
                    .iter()
                    .map(|entry| entry.map(|_| 0))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn ev_scale_rounds_up_minimally_and_supports_f32_extremes() {
        let max = 32767.0 * (1.0 + f64::EPSILON);
        let mut out = [None; 2];
        let report = quantize_evs(&[Some(max), Some(-max)], &mut out).unwrap();
        assert_eq!(report.scale.to_bits(), 1.0_f32.to_bits() + 1);
        assert!(f64::from(report.scale) * 32767.0 >= max);
        assert!(f64::from(f32::from_bits(report.scale.to_bits() - 1)) * 32767.0 < max);
        // A scale exactly between adjacent f32 values must round upward,
        // regardless of which neighbor nearest-even conversion would select.
        for bits in [
            1,
            2,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0000,
            0x3f80_0001,
            0x7f7f_fffe,
        ] {
            let lower = f32::from_bits(bits);
            let upper = f32::from_bits(bits + 1);
            let required = (f64::from(lower) + f64::from(upper)) * 0.5;
            let max = required * 32767.0;
            let report = quantize_evs(&[Some(max), Some(-max)], &mut out).unwrap();
            assert_eq!(report.scale, upper);
            assert!(f64::from(lower) * 32767.0 < max);
            assert!(f64::from(upper) * 32767.0 >= max);
        }
        for scale in [f32::from_bits(1), f32::MIN_POSITIVE, 1.0, f32::MAX] {
            let max = f64::from(scale) * 32767.0;
            let half = f64::from(scale) * 0.5;
            let input = [Some(max), Some(-max), Some(half), Some(-half)];
            let mut out = [None; 4];
            let report = quantize_evs(&input, &mut out).unwrap();
            assert_eq!(report.scale, scale);
            assert_eq!(report.max_absolute_error, half);
            assert_eq!(out, [Some(32767), Some(-32767), Some(1), Some(-1)]);
        }
    }

    #[test]
    fn ev_failures_preserve_every_output_and_maximum_block_is_supported() {
        let limit = f64::from(f32::MAX) * 32767.0;
        let above_limit = f64::from_bits(limit.to_bits() + 1);
        let half_minimum = f64::from(f32::from_bits(1)) * 0.5 * 32767.0;
        for (input, expected) in [
            (vec![Some(1.0), Some(f64::NAN)], QuantizeError::EvValue),
            (vec![Some(f64::INFINITY)], QuantizeError::EvValue),
            (vec![Some(f64::NEG_INFINITY)], QuantizeError::EvValue),
            (vec![Some(f64::MAX)], QuantizeError::EvScaleOverflow),
            (vec![Some(above_limit)], QuantizeError::EvScaleOverflow),
            (
                vec![Some(f64::from_bits(1))],
                QuantizeError::EvScaleUnderflow,
            ),
            (vec![Some(half_minimum)], QuantizeError::EvScaleUnderflow),
            (vec![None; MAX_EV_ENTRIES + 1], QuantizeError::EvLength),
        ] {
            let mut out = vec![Some(123); input.len()];
            assert_eq!(quantize_evs(&input, &mut out), Err(expected));
            assert!(out.iter().all(|value| *value == Some(123)));
        }
        let mut out = [Some(456); 2];
        assert_eq!(
            quantize_evs(&[None], &mut out),
            Err(QuantizeError::OutputLength)
        );
        assert_eq!(out, [Some(456); 2]);
        let input = vec![Some(0.0); MAX_EV_ENTRIES];
        let mut out = vec![None; MAX_EV_ENTRIES];
        assert_eq!(quantize_evs(&input, &mut out).unwrap().scale, 1.0);
        assert!(out.iter().all(|value| *value == Some(0)));
    }

    #[test]
    fn generated_evs_and_half_step_neighbors_obey_the_strict_stored_scale_bound() {
        for bits in [
            1,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0001,
            0x4020_0013,
            0x7f7f_ffff,
        ] {
            let scale = f32::from_bits(bits);
            let scale64 = f64::from(scale);
            let mut input = vec![Some(scale64 * 32767.0), None];
            for code in [0, 1, 19, 1023, 16383, 32766] {
                let midpoint = (f64::from(code) + 0.5) * scale64;
                for value in [
                    f64::from_bits(midpoint.to_bits() - 1),
                    midpoint,
                    f64::from_bits(midpoint.to_bits() + 1),
                ] {
                    input.push(Some(value));
                    input.push(Some(-value));
                }
            }
            let mut out = vec![None; input.len()];
            let report = quantize_evs(&input, &mut out).unwrap();
            assert_eq!(report.scale, scale);
            let mut maximum = 0.0_f64;
            for (value, code) in input.iter().zip(out) {
                match (value, code) {
                    (Some(value), Some(code)) => {
                        assert!((-32767..=32767).contains(&code));
                        let error = (value - f64::from(code) * scale64).abs();
                        assert!(error <= 0.5 * scale64);
                        maximum = maximum.max(error);
                    }
                    (None, None) => {}
                    _ => panic!("missing EV changed state"),
                }
            }
            assert_eq!(report.max_absolute_error, maximum);
        }
    }

    #[test]
    fn reach_reports_conversion_error_and_refuses_positive_underflow() {
        for input in [0.0, -0.0, f64::from(f32::from_bits(1)), 0.1, 1.0 / 3.0, 1.0] {
            let report = quantize_reach(input).unwrap();
            assert_eq!(report.value.to_bits(), (input as f32).to_bits());
            assert_eq!(
                report.absolute_error,
                (f64::from(report.value) - input).abs()
            );
        }
        for input in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -f64::EPSILON,
            1.0 + f64::EPSILON,
        ] {
            assert_eq!(quantize_reach(input), Err(QuantizeError::ReachValue));
        }
        for input in [f64::from_bits(1), f64::from(f32::from_bits(1)) * 0.5] {
            assert_eq!(quantize_reach(input), Err(QuantizeError::ReachUnderflow));
        }
    }
}
