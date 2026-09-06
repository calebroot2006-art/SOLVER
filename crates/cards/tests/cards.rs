//! Exhaustive checks for standard-deck identity, unordered combos, and grid membership.

use std::collections::BTreeSet;

use cards::{Card, CardError, CardSet, Combo, Rank, Suit, combos_for_cell};

#[test]
fn every_card_has_the_specified_text_id_mask_and_rank() {
    let mut mask = 0_u64;
    let mut expected_id = 0;
    for (rank_index, rank_symbol) in "23456789TJQKA".chars().enumerate() {
        for (suit_index, suit_symbol) in "cdhs".chars().enumerate() {
            let text = format!("{rank_symbol}{suit_symbol}");
            let card: Card = text.parse().unwrap();
            assert_eq!(card.id(), expected_id);
            assert_eq!(card.rank().index() as usize, rank_index);
            assert_eq!(card.suit().index() as usize, suit_index);
            assert_eq!(Card::new(card.rank(), card.suit()), card);
            assert_eq!(Card::from_id(expected_id).unwrap(), card);
            assert_eq!(card.to_string(), text);
            assert_eq!(card.mask().count_ones(), 1);
            assert_eq!(mask & card.mask(), 0);
            mask |= card.mask();
            expected_id += 1;
        }
    }
    assert_eq!(mask, (1_u64 << 52) - 1);
    assert_eq!(Card::all().len(), 52);
    assert_eq!(Card::all().map(Card::id).collect::<Vec<_>>(), (0..52).collect::<Vec<_>>());
    assert_eq!(Card::all().next_back().unwrap().to_string(), "As");
    assert_eq!(Rank::all().map(Rank::index).collect::<Vec<_>>(), (0..13).collect::<Vec<_>>());
    assert_eq!(Suit::all().map(Suit::index).collect::<Vec<_>>(), (0..4).collect::<Vec<_>>());
    for index in 0..=u8::MAX {
        assert_eq!(Rank::from_index(index).is_some(), index < 13);
        assert_eq!(Suit::from_index(index).is_some(), index < 4);
        assert_eq!(Card::from_id(index).is_ok(), index < 52);
    }
}

#[test]
fn malformed_card_and_combo_text_is_rejected_without_panics() {
    for text in ["", "A", "AS", "as", "10s", "1c", "Ac ", " Ac", "A♠", "Äs", "As\0"] {
        assert!(text.parse::<Card>().is_err(), "{text:?}");
    }
    for text in ["", "As", "AsAs", "AsKh ", "As Kh", "asKh", "ASkh", "A♠Kh", "éé", "AsK\0"] {
        assert!(text.parse::<Combo>().is_err(), "{text:?}");
    }
    for byte in 0..=u8::MAX {
        let text = String::from_utf8_lossy(&[byte, b's']).into_owned();
        if !b"23456789TJQKA".contains(&byte) { assert!(text.parse::<Card>().is_err()); }
    }
}

#[test]
fn every_unordered_pair_has_one_combo_id_and_two_order_independent_inputs() {
    let mut seen = [false; 1326];
    let mut next_id = 0;
    for b in Card::all() {
        assert_eq!(Combo::new(b, b), Err(CardError::DuplicateCard(b)));
        for a in Card::all().take(b.id() as usize) {
            let combo = Combo::new(a, b).unwrap();
            assert_eq!(combo, Combo::new(b, a).unwrap());
            assert_eq!(combo.id(), next_id);
            assert_eq!(combo.cards(), [a, b]);
            assert_eq!(combo.mask(), a.mask() | b.mask());
            assert_eq!(combo.mask().count_ones(), 2);
            assert!(!seen[combo.id() as usize]);
            seen[combo.id() as usize] = true;
            assert_eq!(Combo::from_id(next_id).unwrap(), combo);
            assert_eq!(format!("{a}{b}").parse::<Combo>().unwrap(), combo);
            assert_eq!(format!("{b}{a}").parse::<Combo>().unwrap(), combo);
            assert_eq!(combo.to_string(), format!("{b}{a}"));
            next_id += 1;
        }
    }
    assert_eq!(next_id, 1326);
    assert!(seen.into_iter().all(|entry| entry));
    assert_eq!(Combo::all().len(), 1326);
    assert_eq!(Combo::all().map(Combo::id).collect::<Vec<_>>(), (0..1326).collect::<Vec<_>>());
    assert_eq!(Combo::all().next_back().unwrap().id(), 1325);
    for id in 1326..=u16::MAX { assert_eq!(Combo::from_id(id), Err(CardError::InvalidComboId(id))); }
}

#[test]
fn grid_is_a_partition_with_correct_rank_suit_and_class_counts() {
    let mut seen = BTreeSet::new();
    for row in 0..13 {
        for col in 0..13 {
            let members = combos_for_cell(row, col).unwrap();
            assert_eq!(members.len(), if row == col { 6 } else if row < col { 4 } else { 12 });
            assert!(members.windows(2).all(|pair| pair[0].id() < pair[1].id()));
            for combo in members {
                assert!(seen.insert(combo.id()));
                let [a, b] = combo.cards();
                assert_eq!(a.rank().index() as usize, 12 - row.max(col));
                assert_eq!(b.rank().index() as usize, 12 - row.min(col));
                if row != col { assert_eq!(a.suit() == b.suit(), row < col); }
                assert_eq!(combo.grid_cell(), (row, col));
            }
        }
    }
    assert_eq!(seen.len(), 1326);
    assert_eq!("AsKs".parse::<Combo>().unwrap().grid_cell(), (0, 1));
    assert_eq!("AsKh".parse::<Combo>().unwrap().grid_cell(), (1, 0));
    assert_eq!("2s2h".parse::<Combo>().unwrap().grid_cell(), (12, 12));
    for (row, col) in [(13, 0), (0, 13), (usize::MAX, usize::MAX)] {
        assert!(combos_for_cell(row, col).is_err());
    }
}

#[test]
fn card_sets_reject_duplicates_and_preserve_every_membership() {
    let cards: Vec<_> = Card::all().collect();
    let set = CardSet::new(&cards).unwrap();
    assert_eq!(set.len(), 52);
    assert_eq!(set.bits(), (1_u64 << 52) - 1);
    assert!(cards.iter().all(|&card| set.contains(card)));
    assert!(CardSet::new(&[]).unwrap().is_empty());
    assert_eq!(CardSet::default().bits(), 0);
    for card in Card::all() {
        assert_eq!(CardSet::new(&[card, card]), Err(CardError::DuplicateCard(card)));
        let singleton = CardSet::new(&[card]).unwrap();
        assert_eq!(singleton.len(), 1);
        for other in Card::all() { assert_eq!(singleton.contains(other), other == card); }
    }
}
