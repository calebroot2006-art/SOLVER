use std::{fmt, str::FromStr};

use crate::{CardSet, Combo, RangeError, Rank};

/// Maximum UTF-8 byte length accepted by [`Range::parse`].
pub const MAX_RANGE_BYTES: usize = 131_072;
/// Maximum token count accepted before range expressions are expanded.
pub const MAX_RANGE_TOKENS: usize = 4096;

/// Finite inclusion weights for the 1326 physical two-card combos.
///
/// Weights lie in `[0,1]` and are not normalized. Both signs of zero are stored
/// as positive zero. An empty range is valid data; a solve builder must check
/// whether its two ranges contain compatible positive-weight combos.
#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    weights: [f64; 1326],
}

impl Range {
    /// Construct a range with every inclusion weight zero.
    #[must_use]
    pub const fn empty() -> Self { Self { weights: [0.0; 1326] } }

    /// Validate a complete array indexed by [`Combo::id`], without normalization.
    pub fn from_weights(mut weights: [f64; 1326]) -> Result<Self, RangeError> {
        for combo in Combo::all() {
            let weight = &mut weights[combo.id() as usize];
            validate_weight(combo, *weight)?;
            if *weight == 0.0 { *weight = 0.0; }
        }
        Ok(Self { weights })
    }

    /// Borrow the 1326 inclusion weights in ascending combo-ID order.
    #[must_use]
    pub const fn weights(&self) -> &[f64; 1326] { &self.weights }

    /// Read the inclusion weight of a checked physical combo.
    #[must_use]
    pub fn weight(&self, combo: Combo) -> f64 { self.weights[combo.id() as usize] }

    /// Set an editor weight, explicitly overriding any previous value.
    ///
    /// Invalid weights return an error and leave the range unchanged.
    pub fn set_weight(&mut self, combo: Combo, weight: f64) -> Result<(), RangeError> {
        validate_weight(combo, weight)?;
        self.weights[combo.id() as usize] = if weight == 0.0 { 0.0 } else { weight };
        Ok(())
    }

    /// Copy this range and zero every combo overlapping a dead card.
    #[must_use]
    pub fn without_cards(&self, dead: CardSet) -> Self {
        let mut result = self.clone();
        for combo in Combo::all() {
            if combo.mask() & dead.bits() != 0 { result.weights[combo.id() as usize] = 0.0; }
        }
        result
    }

    /// Parse the documented Pio-style subset into physical inclusion weights.
    ///
    /// Tokens are separated by commas or ASCII whitespace. Equal overlapping
    /// assignments are idempotent; unequal assignments are errors, including
    /// conflicts with an explicitly assigned zero. Empty text is an empty range.
    pub fn parse(text: &str) -> Result<Self, RangeError> {
        if text.len() > MAX_RANGE_BYTES {
            return Err(RangeError::InputTooLong { bytes: text.len(), limit: MAX_RANGE_BYTES });
        }
        if text.bytes().all(|byte| byte.is_ascii_whitespace()) { return Ok(Self::empty()); }
        let mut tokens = Vec::new();
        for (item, part) in text.split(',').enumerate() {
            if part.bytes().all(|byte| byte.is_ascii_whitespace()) {
                return Err(RangeError::EmptyCommaItem { item });
            }
            for token in part.split_ascii_whitespace() {
                tokens.push(token);
                if tokens.len() > MAX_RANGE_TOKENS {
                    return Err(RangeError::TooManyTokens { count: tokens.len(), limit: MAX_RANGE_TOKENS });
                }
            }
        }
        let mut result = Self::empty();
        let mut assigned = [false; 1326];
        for token in tokens {
            let (expression, weight) = if let Some((expression, suffix)) = token.split_once(':') {
                let weight = suffix.parse::<f64>().map_err(|_| invalid(token, "weight must be a number in [0,1]"))?;
                if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
                    return Err(invalid(token, "weight must be finite and in [0,1]"));
                }
                (expression, weight)
            } else { (token, 1.0) };
            let combos = expand(expression).map_err(|reason| invalid(token, reason))?;
            for combo in combos {
                let index = combo.id() as usize;
                if assigned[index] && result.weights[index] != weight {
                    return Err(RangeError::ConflictingAssignment {
                        token: token.to_owned(), combo, previous: result.weights[index], incoming: weight,
                    });
                }
                assigned[index] = true;
                result.weights[index] = if weight == 0.0 { 0.0 } else { weight };
            }
        }
        Ok(result)
    }

    /// Return nonzero explicit combos in combo-ID order with roundtrippable weights.
    ///
    /// Scientific notation keeps even the smallest positive weights inside the
    /// parser's byte limit. Unit weights omit their suffix; an empty range prints
    /// an empty string. The output preserves weights, not the input shorthand.
    #[must_use]
    pub fn to_canonical_string(&self) -> String { self.to_string() }

    /// Return the physical combos in a checked 13-by-13 grid cell.
    pub fn combos_for_cell(row: usize, col: usize) -> Result<Vec<Combo>, RangeError> {
        combos_for_cell(row, col)
    }
}

impl Default for Range {
    fn default() -> Self { Self::empty() }
}

impl FromStr for Range {
    type Err = RangeError;
    fn from_str(text: &str) -> Result<Self, Self::Err> { Self::parse(text) }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for combo in Combo::all() {
            let weight = self.weight(combo);
            if weight == 0.0 { continue; }
            if !first { f.write_str(",")?; }
            first = false;
            write!(f, "{combo}")?;
            if weight != 1.0 {
                let decimal = weight.to_string();
                let scientific = format!("{weight:e}");
                write!(f, ":{}", if scientific.len() < decimal.len() { scientific } else { decimal })?;
            }
        }
        Ok(())
    }
}

/// Return all combos in a descending-rank grid cell, in combo-ID order.
///
/// Row/column zero denote ace. Diagonal cells contain six pair combos; cells
/// above the diagonal contain four suited combos; cells below contain twelve
/// offsuit combos. Coordinates outside `0..13` are rejected.
pub fn combos_for_cell(row: usize, col: usize) -> Result<Vec<Combo>, RangeError> {
    if row >= 13 || col >= 13 { return Err(RangeError::InvalidCell { row, col }); }
    Ok(Combo::all().filter(|combo| combo.grid_cell() == (row, col)).collect())
}

fn validate_weight(combo: Combo, weight: f64) -> Result<(), RangeError> {
    if weight.is_finite() && (0.0..=1.0).contains(&weight) { Ok(()) }
    else { Err(RangeError::InvalidWeight { combo, weight }) }
}

fn invalid(token: &str, reason: &str) -> RangeError {
    RangeError::InvalidToken { token: token.to_owned(), reason: reason.to_owned() }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind { Both, Suited, Offsuit }

#[derive(Clone, Copy)]
struct Class { high: u8, low: u8, kind: Kind }

impl Class {
    fn parse(text: &str) -> Result<Self, &'static str> {
        let bytes = text.as_bytes();
        if !(2..=3).contains(&bytes.len()) { return Err("expected a two-rank class with optional s or o suffix"); }
        let high = Rank::from_symbol(bytes[0]).ok_or("invalid high rank")?.index();
        let low = Rank::from_symbol(bytes[1]).ok_or("invalid low rank")?.index();
        if high < low { return Err("class ranks must be in descending order"); }
        let kind = match bytes.get(2) {
            None => Kind::Both,
            Some(b's') => Kind::Suited,
            Some(b'o') => Kind::Offsuit,
            _ => return Err("class suffix must be s or o"),
        };
        if high == low && kind != Kind::Both { return Err("pairs cannot have a suitedness suffix"); }
        Ok(Self { high, low, kind })
    }

    fn matches(self, combo: Combo) -> bool {
        let [a, b] = combo.cards();
        let suited = a.suit() == b.suit();
        a.rank().index() == self.low && b.rank().index() == self.high
            && match self.kind { Kind::Both => true, Kind::Suited => suited, Kind::Offsuit => !suited }
    }
}

fn expand(expression: &str) -> Result<Vec<Combo>, &'static str> {
    let classes = if let Some((start, end)) = expression.split_once('-') {
        let start = Class::parse(start)?;
        let end = Class::parse(end)?;
        if start.kind != end.kind { return Err("interval suffixes must match"); }
        let start_pair = start.high == start.low;
        let end_pair = end.high == end.low;
        if start_pair != end_pair { return Err("interval cannot mix a pair and a nonpair"); }
        if start_pair {
            (start.high.min(end.high)..=start.high.max(end.high))
                .map(|rank| Class { high: rank, low: rank, kind: Kind::Both }).collect::<Vec<_>>()
        } else if start.high == end.high {
            (start.low.min(end.low)..=start.low.max(end.low))
                .map(|low| Class { low, ..start }).collect()
        } else if start.high - start.low == end.high - end.low {
            let gap = start.high - start.low;
            (start.low.min(end.low)..=start.low.max(end.low))
                .map(|low| Class { high: low + gap, low, kind: start.kind }).collect()
        } else { return Err("interval must keep its high rank or its rank gap constant"); }
    } else if let Some(base) = expression.strip_suffix('+') {
        let base = Class::parse(base)?;
        if base.high == base.low {
            (base.high..=12).map(|rank| Class { high: rank, low: rank, kind: Kind::Both }).collect()
        } else {
            (base.low..base.high).map(|low| Class { low, ..base }).collect()
        }
    } else if expression.len() == 4 {
        return expression.parse::<Combo>().map(|combo| vec![combo]).map_err(|_| "expected two distinct cards");
    } else { vec![Class::parse(expression)?] };
    Ok(Combo::all().filter(|&combo| classes.iter().any(|class| class.matches(combo))).collect())
}
