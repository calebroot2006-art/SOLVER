use std::{fmt, str::FromStr};

use crate::{Card, CardError};

/// An unordered pair of distinct cards, ordered by its stable combo ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Combo {
    id: u16,
    cards: [Card; 2],
}

impl Combo {
    /// Build an unordered combo, rejecting duplicate cards.
    pub const fn new(first: Card, second: Card) -> Result<Self, CardError> {
        if first.id() == second.id() { return Err(CardError::DuplicateCard(first)); }
        let cards = if first.id() < second.id() { [first, second] } else { [second, first] };
        let a = cards[0].id() as u16;
        let b = cards[1].id() as u16;
        Ok(Self { id: b * (b - 1) / 2 + a, cards })
    }

    /// Convert a combo ID, rejecting values outside `0..1326`.
    pub fn from_id(id: u16) -> Result<Self, CardError> {
        if id >= 1326 { return Err(CardError::InvalidComboId(id)); }
        let mut b = 1_u16;
        while b * (b + 1) / 2 <= id { b += 1; }
        let a = id - b * (b - 1) / 2;
        Ok(Self { id, cards: [Card::from_id(a as u8)?, Card::from_id(b as u8)?] })
    }

    /// Return `b * (b - 1) / 2 + a` for sorted card IDs `a < b`.
    #[must_use]
    pub const fn id(self) -> u16 { self.id }

    /// Return both cards in ascending card-ID order.
    #[must_use]
    pub const fn cards(self) -> [Card; 2] { self.cards }

    /// Return the two-bit card mask.
    #[must_use]
    pub const fn mask(self) -> u64 { self.cards[0].mask() | self.cards[1].mask() }

    /// Iterate through all 1326 combos in ascending combo-ID order.
    pub fn all() -> impl ExactSizeIterator<Item = Self> + DoubleEndedIterator {
        (0..1326).map(|id| Self::from_id(id).expect("the combo ID iterator stays below 1326"))
    }

    /// Return a cell in the descending-rank 13-by-13 range grid.
    ///
    /// Pairs occupy the diagonal; suited combos are above it and offsuit combos
    /// are below it. Row and column zero represent ace.
    #[must_use]
    pub const fn grid_cell(self) -> (usize, usize) {
        let low = 12 - self.cards[0].rank().index() as usize;
        let high = 12 - self.cards[1].rank().index() as usize;
        if self.cards[0].suit().index() == self.cards[1].suit().index() {
            (high, low)
        } else { (low, high) }
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.cards[1], self.cards[0])
    }
}

impl FromStr for Combo {
    type Err = CardError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 4 || !text.is_ascii() {
            return Err(CardError::InvalidComboText(text.to_owned()));
        }
        let first = text[..2].parse::<Card>()
            .map_err(|_| CardError::InvalidComboText(text.to_owned()))?;
        let second = text[2..].parse::<Card>()
            .map_err(|_| CardError::InvalidComboText(text.to_owned()))?;
        Self::new(first, second)
    }
}
