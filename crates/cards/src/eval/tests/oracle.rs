//! Independent five-card classifier: counts and lexicographic kicker tuples.

use crate::Card;

fn packed(category: u8, ranks: [u8; 5]) -> u32 {
    ranks.into_iter().fold(u32::from(category), |value, rank| {
        (value << 4) | u32::from(rank)
    })
}

pub(super) fn five(cards: [Card; 5]) -> u32 {
    let mut counts = [0_u8; 13];
    let mut mask = 0_u16;
    let flush = cards.iter().all(|card| card.suit() == cards[0].suit());
    for card in cards {
        let rank = card.rank().index();
        counts[usize::from(rank)] += 1;
        mask |= 1 << rank;
    }
    let mut descending = [0_u8; 5];
    let mut singles = [0_u8; 5];
    let mut pairs = [0_u8; 2];
    let (mut n, mut n_single, mut n_pair) = (0, 0, 0);
    let (mut trip, mut quad) = (None, None);
    for rank in (0_u8..13).rev() {
        let count = counts[usize::from(rank)];
        for _ in 0..count {
            descending[n] = rank;
            n += 1;
        }
        match count {
            1 => { singles[n_single] = rank; n_single += 1; }
            2 => { pairs[n_pair] = rank; n_pair += 1; }
            3 => trip = Some(rank),
            4 => quad = Some(rank),
            _ => {}
        }
    }
    let straight = if mask == ((1 << 12) | 0b1111) {
        Some(3) // Ace, deuce, three, four, five.
    } else if mask.count_ones() == 5 && descending[0] - descending[4] == 4 {
        Some(descending[0])
    } else { None };
    if let Some(high) = straight && flush {
        return packed(8, [high, 0, 0, 0, 0]);
    }
    if let Some(rank) = quad { return packed(7, [rank, singles[0], 0, 0, 0]); }
    if let Some(rank) = trip && n_pair == 1 {
        return packed(6, [rank, pairs[0], 0, 0, 0]);
    }
    if flush { return packed(5, descending); }
    if let Some(high) = straight { return packed(4, [high, 0, 0, 0, 0]); }
    if let Some(rank) = trip { return packed(3, [rank, singles[0], singles[1], 0, 0]); }
    if n_pair == 2 { return packed(2, [pairs[0], pairs[1], singles[0], 0, 0]); }
    if n_pair == 1 { return packed(1, [pairs[0], singles[0], singles[1], singles[2], 0]); }
    packed(0, descending)
}

pub(super) fn seven(cards: [Card; 7]) -> u32 {
    let mut best = 0;
    for a in 0..3 {
        for b in a + 1..4 {
            for c in b + 1..5 {
                for d in c + 1..6 {
                    for e in d + 1..7 {
                        best = best.max(five([cards[a], cards[b], cards[c], cards[d], cards[e]]));
                    }
                }
            }
        }
    }
    best
}
