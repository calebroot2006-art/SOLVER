use std::{error::Error, fmt};

use crate::{Card, Combo};

/// Invalid card identity, card text, or repeated-card input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CardError {
    /// A card ID outside `0..52`.
    InvalidCardId(u8),
    /// A combo ID outside `0..1326`.
    InvalidComboId(u16),
    /// Text that is not exactly an uppercase rank and lowercase suit.
    InvalidCardText(String),
    /// Text that is not exactly two valid cards.
    InvalidComboText(String),
    /// A card appears more than once in an input that requires uniqueness.
    DuplicateCard(Card),
}

impl fmt::Display for CardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCardId(id) => write!(f, "card ID {id} is outside 0..52"),
            Self::InvalidComboId(id) => write!(f, "combo ID {id} is outside 0..1326"),
            Self::InvalidCardText(text) => write!(f, "invalid card text {text:?}"),
            Self::InvalidComboText(text) => write!(f, "invalid combo text {text:?}"),
            Self::DuplicateCard(card) => write!(f, "duplicate card {card}"),
        }
    }
}

impl Error for CardError {}

/// A range fails its input, weight, or grid contract.
#[derive(Clone, Debug, PartialEq)]
pub enum RangeError {
    /// Text exceeds the byte limit, checked before tokenization.
    InputTooLong {
        /// Actual number of bytes.
        bytes: usize,
        /// Maximum accepted number of bytes.
        limit: usize,
    },
    /// Text exceeds the token limit, checked before expanding any expression.
    TooManyTokens {
        /// Number of tokens observed when rejecting the input.
        count: usize,
        /// Maximum accepted number of tokens.
        limit: usize,
    },
    /// A leading, trailing, or adjacent comma leaves an empty item.
    EmptyCommaItem {
        /// Zero-based comma-separated item index.
        item: usize,
    },
    /// A token does not belong to the supported syntax.
    InvalidToken {
        /// The complete token, including any weight suffix.
        token: String,
        /// Why this token was rejected.
        reason: String,
    },
    /// An array or editor weight is nonfinite or outside `[0,1]`.
    InvalidWeight {
        /// Combo whose weight was rejected.
        combo: Combo,
        /// Rejected inclusion weight.
        weight: f64,
    },
    /// Two expressions assign unequal weights to the same physical combo.
    ConflictingAssignment {
        /// Token that introduced the conflict.
        token: String,
        /// Physical combo assigned twice.
        combo: Combo,
        /// Weight from the earlier token.
        previous: f64,
        /// Weight requested by this token.
        incoming: f64,
    },
    /// A grid coordinate is outside the 13-by-13 grid.
    InvalidCell {
        /// Rejected row index.
        row: usize,
        /// Rejected column index.
        col: usize,
    },
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLong { bytes, limit } => {
                write!(f, "range has {bytes} bytes; limit is {limit}")
            }
            Self::TooManyTokens { count, limit } => {
                write!(f, "range has at least {count} tokens; limit is {limit}")
            }
            Self::EmptyCommaItem { item } => write!(f, "range comma item {item} is empty"),
            Self::InvalidToken { token, reason } => {
                write!(f, "invalid range token {token:?}: {reason}")
            }
            Self::InvalidWeight { combo, weight } => {
                write!(f, "weight {weight} for {combo} must be finite and in [0,1]")
            }
            Self::ConflictingAssignment {
                token,
                combo,
                previous,
                incoming,
            } => write!(
                f,
                "token {token:?} assigns {incoming} to {combo}, already assigned {previous}"
            ),
            Self::InvalidCell { row, col } => {
                write!(f, "range cell ({row},{col}) is outside the 13-by-13 grid")
            }
        }
    }
}

impl Error for RangeError {}
