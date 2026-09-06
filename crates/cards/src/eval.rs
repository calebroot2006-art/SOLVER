//! Checked standard hold'em evaluation with a private, replaceable backend.

use rs_poker::core::{
    Card as BackendCard, CoreRank, Rank as BackendRank, Rankable, SevenCardAccum,
    Suit as BackendSuit, Value,
};

use crate::{Card, CardError, CardSet, Combo, Rank, Suit};

/// Five-card hand categories, ordered from weakest to strongest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum HandCategory {
    /// No pair, straight, or flush.
    HighCard,
    /// One pair and three kickers.
    OnePair,
    /// Two pairs and one kicker.
    TwoPair,
    /// Three equal ranks and two kickers.
    ThreeOfAKind,
    /// Five consecutive ranks, with the ace also usable below the deuce.
    Straight,
    /// Five cards of the same suit, compared by all five ranks.
    Flush,
    /// Three equal ranks and a pair.
    FullHouse,
    /// Four equal ranks and one kicker.
    FourOfAKind,
    /// Five consecutive ranks of the same suit.
    StraightFlush,
}

/// Opaque strength of the best five-card hand; larger values are stronger.
///
/// Equal values mean a showdown tie. Only checked evaluators construct this type;
/// backend score encodings are neither accepted as inputs nor a storage format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct HandValue(BackendRank);

impl HandValue {
    /// Return the hand category without its tie-breaking detail.
    #[must_use]
    pub fn category(self) -> HandCategory {
        match self.0.category() {
            CoreRank::HighCard => HandCategory::HighCard,
            CoreRank::OnePair => HandCategory::OnePair,
            CoreRank::TwoPair => HandCategory::TwoPair,
            CoreRank::ThreeOfAKind => HandCategory::ThreeOfAKind,
            CoreRank::Straight => HandCategory::Straight,
            CoreRank::Flush => HandCategory::Flush,
            CoreRank::FullHouse => HandCategory::FullHouse,
            CoreRank::FourOfAKind => HandCategory::FourOfAKind,
            CoreRank::StraightFlush => HandCategory::StraightFlush,
        }
    }
}

fn backend_card(card: Card) -> BackendCard {
    // Match names explicitly: dependency suit IDs are not our serialization IDs.
    let rank = match card.rank() {
        Rank::Two => Value::Two,
        Rank::Three => Value::Three,
        Rank::Four => Value::Four,
        Rank::Five => Value::Five,
        Rank::Six => Value::Six,
        Rank::Seven => Value::Seven,
        Rank::Eight => Value::Eight,
        Rank::Nine => Value::Nine,
        Rank::Ten => Value::Ten,
        Rank::Jack => Value::Jack,
        Rank::Queen => Value::Queen,
        Rank::King => Value::King,
        Rank::Ace => Value::Ace,
    };
    let suit = match card.suit() {
        Suit::Clubs => BackendSuit::Club,
        Suit::Diamonds => BackendSuit::Diamond,
        Suit::Hearts => BackendSuit::Heart,
        Suit::Spades => BackendSuit::Spade,
    };
    BackendCard::new(rank, suit)
}

fn checked_rank<const N: usize>(cards: [Card; N]) -> Result<HandValue, CardError> {
    CardSet::new(&cards)?;
    let converted = cards.map(backend_card);
    Ok(HandValue(converted.as_slice().rank()))
}

/// Evaluate exactly five distinct standard-deck cards.
pub fn evaluate_five(cards: [Card; 5]) -> Result<HandValue, CardError> {
    checked_rank(cards)
}

/// Evaluate the best five-card hand among exactly seven distinct cards.
pub fn evaluate_seven(cards: [Card; 7]) -> Result<HandValue, CardError> {
    checked_rank(cards)
}

/// Evaluate a checked river board and two hole cards, rejecting any overlap.
pub fn evaluate_holdem(board: [Card; 5], hole: Combo) -> Result<HandValue, CardError> {
    RiverEvaluator::new(board)?.evaluate(hole)
}

/// Reusable immutable board prefix for repeated checked river evaluation.
#[derive(Clone, Copy)]
pub struct RiverEvaluator {
    board: CardSet,
    accumulator: SevenCardAccum,
}

impl RiverEvaluator {
    /// Validate five distinct board cards and prepare their evaluation prefix.
    pub fn new(board: [Card; 5]) -> Result<Self, CardError> {
        let mask = CardSet::new(&board)?;
        let mut accumulator = SevenCardAccum::new();
        for card in board {
            accumulator.add(backend_card(card));
        }
        Ok(Self { board: mask, accumulator })
    }

    /// Evaluate one hole combo without allowing a card already on the board.
    pub fn evaluate(&self, hole: Combo) -> Result<HandValue, CardError> {
        let mut accumulator = self.accumulator;
        for card in hole.cards() {
            if self.board.contains(card) {
                return Err(CardError::DuplicateCard(card));
            }
            accumulator.add(backend_card(card));
        }
        Ok(HandValue(accumulator.rank()))
    }
}

#[cfg(test)]
mod tests;
