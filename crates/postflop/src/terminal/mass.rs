//! Exact nonnegative reach sums avoid losing a small compatible hand when large
//! blocked hands are subtracted. Every finite nonnegative f64 is an integer in
//! units of 2^-1074. Its highest possible set bit is 2097; adding at most 1326
//! reaches and one exact-combo add-back needs at most 2109 bits. Our 34 u64 limbs
//! provide 2176 bits. This fixed factor preserves linear sweep complexity.

use super::TerminalError;
use cards::Combo;
use std::cmp::Ordering;

const LIMBS: usize = 34;
const FRACTION: u64 = (1_u64 << 52) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactMass([u64; LIMBS]);

impl Default for ExactMass {
    fn default() -> Self {
        Self([0; LIMBS])
    }
}

impl ExactMass {
    fn add(&mut self, value: f64) -> Result<(), TerminalError> {
        if !value.is_finite() || value < 0.0 {
            return Err(TerminalError::Arithmetic("invalid exact mass input"));
        }
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        let mantissa = (bits & FRACTION) | if exponent == 0 { 0 } else { 1_u64 << 52 };
        let shift = exponent.saturating_sub(1);
        let index = shift / 64;
        let offset = shift % 64;
        let wide = u128::from(mantissa) << offset;
        self.add_word(index, wide as u64)?;
        self.add_word(index + 1, (wide >> 64) as u64)
    }

    fn add_word(&mut self, mut index: usize, mut value: u64) -> Result<(), TerminalError> {
        while value != 0 {
            let slot = self
                .0
                .get_mut(index)
                .ok_or(TerminalError::Arithmetic("exact mass capacity exceeded"))?;
            let (sum, carry) = slot.overflowing_add(value);
            *slot = sum;
            value = u64::from(carry);
            index += 1;
        }
        Ok(())
    }

    fn compare(&self, other: &Self) -> Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }

    fn subtract(&mut self, other: &Self) -> Result<(), TerminalError> {
        if self.compare(other) == Ordering::Less {
            return Err(TerminalError::Arithmetic("negative compatible reach"));
        }
        let mut borrow = false;
        for (slot, rhs) in self.0.iter_mut().zip(other.0) {
            let (difference, first) = slot.overflowing_sub(rhs);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *slot = difference;
            borrow = first || second;
        }
        debug_assert!(!borrow);
        Ok(())
    }

    fn bit(&self, index: usize) -> bool {
        self.0[index / 64] & (1_u64 << (index % 64)) != 0
    }

    fn any_below(&self, limit: usize) -> bool {
        let whole = limit / 64;
        let remainder = limit % 64;
        self.0[..whole].iter().any(|word| *word != 0)
            || (remainder != 0 && self.0[whole] & ((1_u64 << remainder) - 1) != 0)
    }

    /// Rounds the exact integer once, to nearest f64 with ties to even. The
    /// leading 53 bits form the significand; guard/sticky bits decide rounding.
    /// Subnormal values are already exact integer multiples of the unit.
    fn to_f64(self) -> Result<f64, TerminalError> {
        let Some(index) = self.0.iter().rposition(|word| *word != 0) else {
            return Ok(0.0);
        };
        let mut highest = index * 64 + 63 - self.0[index].leading_zeros() as usize;
        if highest < 52 {
            return Ok(f64::from_bits(self.0[0]));
        }
        let shift = highest - 52;
        let word = shift / 64;
        let offset = shift % 64;
        let mut significand = self.0[word] >> offset;
        if offset != 0 && word + 1 < LIMBS {
            significand |= self.0[word + 1] << (64 - offset);
        }
        if shift != 0
            && self.bit(shift - 1)
            && (self.any_below(shift - 1) || significand & 1 != 0)
        {
            significand += 1;
            if significand == 1_u64 << 53 {
                significand >>= 1;
                highest += 1;
            }
        }
        let exponent = highest - 51;
        if exponent >= 0x7ff {
            return Err(TerminalError::Arithmetic("compatible reach or value overflow"));
        }
        Ok(f64::from_bits(
            ((exponent as u64) << 52) | (significand & FRACTION),
        ))
    }
}

pub(super) struct Bucket {
    total: ExactMass,
    cards: [ExactMass; 52],
}

impl Default for Bucket {
    fn default() -> Self {
        Self {
            total: ExactMass::default(),
            cards: [ExactMass::default(); 52],
        }
    }
}

impl Bucket {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn add(&mut self, combo: Combo, reach: f64) -> Result<(), TerminalError> {
        self.total.add(reach)?;
        for card in combo.cards() {
            self.cards[usize::from(card.id())].add(reach)?;
        }
        Ok(())
    }

    pub(super) fn compatible(&self, hero: Combo, same: f64) -> Result<f64, TerminalError> {
        // Add-back first keeps every exact intermediate nonnegative. A combo
        // equal to hero was counted in total once and in the card bins twice.
        let mut result = self.total;
        result.add(same)?;
        for card in hero.cards() {
            result.subtract(&self.cards[usize::from(card.id())])?;
        }
        result.to_f64()
    }
}

pub(super) fn weighted_value(masses: [f64; 3], utilities: [f64; 3]) -> Result<f64, TerminalError> {
    let mut positive = ExactMass::default();
    let mut negative = ExactMass::default();
    for (mass, utility) in masses.into_iter().zip(utilities) {
        let value = mass * utility;
        if !value.is_finite() {
            return Err(TerminalError::Arithmetic("weighted utility overflow"));
        }
        if value == 0.0 && mass != 0.0 && utility != 0.0 {
            return Err(TerminalError::Arithmetic("weighted utility underflow"));
        }
        if value < 0.0 {
            negative.add(-value)?;
        } else {
            positive.add(value)?;
        }
    }
    // Sum the three rounded products exactly before a final rounding. This
    // avoids spurious overflow and preserves a small residual between them.
    match positive.compare(&negative) {
        Ordering::Less => {
            negative.subtract(&positive)?;
            Ok(-negative.to_f64()?)
        }
        _ => {
            positive.subtract(&negative)?;
            positive.to_f64()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(values: &[f64]) -> Result<f64, TerminalError> {
        let mut total = ExactMass::default();
        for value in values {
            total.add(*value)?;
        }
        total.to_f64()
    }

    #[test]
    fn subnormals_and_normal_boundary_are_exact() {
        for bits in [0, 1, 2, (1_u64 << 52) - 1, 1_u64 << 52, (1_u64 << 52) + 1] {
            assert_eq!(sum(&[f64::from_bits(bits)]).unwrap().to_bits(), bits);
        }
        assert_eq!(
            sum(&[f64::from_bits((1_u64 << 52) - 1), f64::from_bits(1)])
                .unwrap()
                .to_bits(),
            1_u64 << 52
        );
    }

    #[test]
    fn every_finite_exponent_preserves_mantissa_edges() {
        for exponent in 0..0x7ff_u64 {
            for fraction in [0, 1, (1_u64 << 51) - 1, 1_u64 << 51, FRACTION] {
                let bits = (exponent << 52) | fraction;
                assert_eq!(sum(&[f64::from_bits(bits)]).unwrap().to_bits(), bits);
            }
        }
    }

    #[test]
    fn halfway_rounding_uses_even_significands_and_sticky_bits() {
        let half = 2.0_f64.powi(-53);
        assert_eq!(sum(&[1.0, half]).unwrap().to_bits(), 1.0_f64.to_bits());
        assert_eq!(
            sum(&[f64::from_bits(1.0_f64.to_bits() + 1), half]).unwrap().to_bits(),
            1.0_f64.to_bits() + 2
        );
        assert_eq!(
            sum(&[1.0, half, f64::from_bits(1)]).unwrap().to_bits(),
            1.0_f64.to_bits() + 1
        );
    }

    #[test]
    fn carries_and_borrows_cross_multiple_limbs() {
        let mut total = ExactMass::default();
        total.0[0] = u64::MAX;
        total.0[1] = u64::MAX;
        total.add(f64::from_bits(1)).unwrap();
        assert_eq!(&total.0[..3], &[0, 0, 1]);
        let mut one = ExactMass::default();
        one.add(f64::from_bits(1)).unwrap();
        total.subtract(&one).unwrap();
        assert_eq!(&total.0[..3], &[u64::MAX, u64::MAX, 0]);
        assert!(one.subtract(&total).is_err());
    }

    #[test]
    fn overflow_boundary_and_cancellation_are_checked_after_subtraction() {
        assert_eq!(sum(&[f64::MAX]).unwrap(), f64::MAX);
        assert_eq!(sum(&[f64::MAX, 2.0_f64.powi(969)]).unwrap(), f64::MAX);
        assert!(sum(&[f64::MAX, 2.0_f64.powi(970)]).is_err());
        let mut total = ExactMass::default();
        let mut blocked = ExactMass::default();
        for _ in 0..1326 {
            total.add(f64::MAX).unwrap();
            blocked.add(f64::MAX).unwrap();
        }
        total.add(f64::from_bits(1)).unwrap();
        total.subtract(&blocked).unwrap();
        assert_eq!(total.to_f64().unwrap().to_bits(), 1);
        assert_eq!(weighted_value([f64::MAX, 1.0, f64::MAX], [1.0, 1.0, -1.0]).unwrap(), 1.0);
        assert!(weighted_value([f64::from_bits(1), 0.0, 0.0], [0.5, 0.0, 0.0]).is_err());
    }

    #[test]
    fn overlap_add_back_and_blockers_precede_any_float_conversion() {
        let hero: Combo = "2c2d".parse().unwrap();
        let mut bucket = Bucket::default();
        bucket.add(hero, f64::MAX).unwrap();
        bucket.add("2c3c".parse().unwrap(), f64::MAX).unwrap();
        bucket.add("2d4c".parse().unwrap(), f64::MAX).unwrap();
        bucket.add("3d4d".parse().unwrap(), f64::from_bits(1)).unwrap();
        assert_eq!(bucket.compatible(hero, f64::MAX).unwrap().to_bits(), 1);
    }

    #[test]
    fn actual_accumulator_matches_independent_fraction_golden_cases() {
        fn accumulate(terms: &str) -> (ExactMass, usize) {
            let mut total = ExactMass::default();
            let mut count = 0;
            for term in terms.split_ascii_whitespace() {
                let (bits, repetitions) = term.split_once('*').unwrap();
                let value = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
                let repetitions: usize = repetitions.parse().unwrap();
                assert!(repetitions > 0);
                count += repetitions;
                assert!(count <= 1327);
                for _ in 0..repetitions {
                    total.add(value).unwrap();
                }
            }
            (total, count)
        }

        // Generated by tests/reference/exact_mass_vectors.py using Python's
        // Fraction arithmetic and conversion, with no copy of our limb logic.
        let fixture = include_str!("exact_mass_vectors.csv");
        let mut lines = fixture.lines();
        assert_eq!(lines.next().unwrap(), "case,addends,subtrahends,expected");
        let mut count = 0;
        for line in lines {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(fields.len(), 4);
            let (mut total, added) = accumulate(fields[1]);
            let (removed, taken) = accumulate(fields[2]);
            assert!(taken <= added);
            total.subtract(&removed).unwrap();
            let actual = total.to_f64();
            if fields[3] == "overflow" {
                assert!(
                    matches!(&actual, Err(TerminalError::Arithmetic("compatible reach or value overflow"))),
                    "golden case {} expected overflow, got {actual:?}",
                    fields[0]
                );
            } else {
                let expected = u64::from_str_radix(fields[3], 16).unwrap();
                assert_eq!(actual.unwrap().to_bits(), expected, "golden case {}", fields[0]);
            }
            count += 1;
        }
        assert_eq!(count, 1000);
    }
}
