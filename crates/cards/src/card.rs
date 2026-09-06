use std::{fmt, str::FromStr};

use crate::CardError;

/// A standard playing-card rank, ordered from deuce to ace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Rank {
    /// Deuce, written `2`.
    Two,
    /// Three, written `3`.
    Three,
    /// Four, written `4`.
    Four,
    /// Five, written `5`.
    Five,
    /// Six, written `6`.
    Six,
    /// Seven, written `7`.
    Seven,
    /// Eight, written `8`.
    Eight,
    /// Nine, written `9`.
    Nine,
    /// Ten, written `T`.
    Ten,
    /// Jack, written `J`.
    Jack,
    /// Queen, written `Q`.
    Queen,
    /// King, written `K`.
    King,
    /// Ace, written `A`.
    Ace,
}

impl Rank {
    const ALL: [Self; 13] = [
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];

    /// Return the zero-based rank index, with deuce zero and ace twelve.
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// Convert a zero-based rank index, rejecting values above twelve.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        if index < 13 {
            Some(Self::ALL[index as usize])
        } else {
            None
        }
    }

    /// Iterate through all ranks from deuce to ace.
    pub fn all() -> impl ExactSizeIterator<Item = Self> + DoubleEndedIterator {
        Self::ALL.into_iter()
    }

    pub(crate) fn from_symbol(symbol: u8) -> Option<Self> {
        b"23456789TJQKA"
            .iter()
            .position(|&item| item == symbol)
            .map(|index| Self::ALL[index])
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", b"23456789TJQKA"[*self as usize] as char)
    }
}

/// A suit in the order used by card IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Suit {
    /// Clubs, written `c`.
    Clubs,
    /// Diamonds, written `d`.
    Diamonds,
    /// Hearts, written `h`.
    Hearts,
    /// Spades, written `s`.
    Spades,
}

impl Suit {
    const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];

    /// Return the zero-based suit index in clubs, diamonds, hearts, spades order.
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// Convert a zero-based suit index, rejecting values above three.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        if index < 4 {
            Some(Self::ALL[index as usize])
        } else {
            None
        }
    }

    /// Iterate through clubs, diamonds, hearts, then spades.
    pub fn all() -> impl ExactSizeIterator<Item = Self> + DoubleEndedIterator {
        Self::ALL.into_iter()
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", b"cdhs"[*self as usize] as char)
    }
}

/// A checked card in a standard 52-card deck.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Card(u8);

impl Card {
    /// Construct a card from its rank and suit.
    #[must_use]
    pub const fn new(rank: Rank, suit: Suit) -> Self {
        Self(4 * rank.index() + suit.index())
    }

    /// Convert a rank-major ID, rejecting values outside `0..52`.
    pub const fn from_id(id: u8) -> Result<Self, CardError> {
        if id < 52 {
            Ok(Self(id))
        } else {
            Err(CardError::InvalidCardId(id))
        }
    }

    /// Return `4 * rank.index() + suit.index()`.
    #[must_use]
    pub const fn id(self) -> u8 {
        self.0
    }

    /// Return this card's rank.
    #[must_use]
    pub const fn rank(self) -> Rank {
        Rank::ALL[(self.0 / 4) as usize]
    }

    /// Return this card's suit.
    #[must_use]
    pub const fn suit(self) -> Suit {
        Suit::ALL[(self.0 % 4) as usize]
    }

    /// Return the single bit at this card's ID.
    #[must_use]
    pub const fn mask(self) -> u64 {
        1_u64 << self.0
    }

    /// Iterate through all 52 cards in ascending ID order.
    pub fn all() -> impl ExactSizeIterator<Item = Self> + DoubleEndedIterator {
        (0..52).map(Self)
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank(), self.suit())
    }
}

impl FromStr for Card {
    type Err = CardError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let parsed = if let [rank, suit] = text.as_bytes() {
            Rank::from_symbol(*rank).zip(
                b"cdhs"
                    .iter()
                    .position(|candidate| candidate == suit)
                    .map(|index| Suit::ALL[index]),
            )
        } else {
            None
        };
        parsed
            .map(|(rank, suit)| Self::new(rank, suit))
            .ok_or_else(|| CardError::InvalidCardText(text.to_owned()))
    }
}

/// A set whose bits correspond only to checked, distinct cards.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CardSet(u64);

impl CardSet {
    /// Build a set, rejecting a repeated card rather than silently removing it.
    pub fn new(cards: &[Card]) -> Result<Self, CardError> {
        let mut bits = 0;
        for &card in cards {
            if bits & card.mask() != 0 {
                return Err(CardError::DuplicateCard(card));
            }
            bits |= card.mask();
        }
        Ok(Self(bits))
    }

    /// Return the set's card-ID bitmask.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Test membership of a checked card.
    #[must_use]
    pub const fn contains(self, card: Card) -> bool {
        self.0 & card.mask() != 0
    }

    /// Return the number of distinct cards.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Return whether the set has no cards.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}
