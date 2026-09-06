//! Checked standard-deck cards, unordered two-card combinations, and weighted ranges.
//!
//! Card IDs use rank-major order with clubs, diamonds, hearts, then spades.
//! Combo IDs use the triangular mapping documented by [`Combo::id`]. Neither
//! encoding claims compatibility with an external solver's array ordering.

mod card;
mod combo;
mod error;
mod eval;
mod range;

pub use card::{Card, CardSet, Rank, Suit};
pub use combo::Combo;
pub use error::{CardError, RangeError};
pub use eval::{HandCategory, HandValue, RiverEvaluator, evaluate_five, evaluate_holdem, evaluate_seven};
pub use range::{MAX_RANGE_BYTES, MAX_RANGE_TOKENS, Range, combos_for_cell};
