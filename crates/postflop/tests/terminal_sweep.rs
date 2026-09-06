use cards::{Card, CardSet, Combo, HandValue, evaluate_seven};
use postflop::terminal::{
    OutcomeUtilities, ShowdownScratch, ShowdownTable, TerminalError, evaluate_fold,
};
use std::cmp::Ordering;

fn board(text: [&str; 5]) -> [Card; 5] {
    text.map(|card| card.parse().unwrap())
}

fn combo(text: &str) -> Combo {
    text.parse().unwrap()
}

// Independent matchup enumeration: no rank groups, card-mass sums, or sweep
// helpers. Each legal pair is classified and weighted directly. Card evaluation
// itself has the separate exhaustive-five-card/ten-million-seven-card gate.
fn pairwise_showdown(board: [Card; 5], reach: &[f64; 1326], utilities: [f64; 3]) -> [f64; 1326] {
    let dead = CardSet::new(&board).unwrap();
    let combos: Vec<_> = Combo::all().collect();
    let values: Vec<Option<HandValue>> = combos
        .iter()
        .map(|hand| {
            if hand.mask() & dead.bits() != 0 {
                None
            } else {
                let hole = hand.cards();
                Some(evaluate_seven([board[0], board[1], board[2], board[3], board[4], hole[0], hole[1]]).unwrap())
            }
        })
        .collect();
    let mut result = [0.0; 1326];
    for (hero_id, hero) in combos.iter().enumerate() {
        let Some(hero_value) = values[hero_id] else {
            continue;
        };
        for (villain_id, villain) in combos.iter().enumerate() {
            let Some(villain_value) = values[villain_id] else {
                continue;
            };
            if hero.mask() & villain.mask() != 0 {
                continue;
            }
            let utility = match hero_value.cmp(&villain_value) {
                Ordering::Greater => utilities[0],
                Ordering::Equal => utilities[1],
                Ordering::Less => utilities[2],
            };
            result[hero_id] += reach[villain_id] * utility;
        }
    }
    result
}

fn pairwise_fold(dead: CardSet, reach: &[f64; 1326], utility: f64) -> [f64; 1326] {
    let mut result = [0.0; 1326];
    for hero in Combo::all() {
        if hero.mask() & dead.bits() != 0 {
            continue;
        }
        for villain in Combo::all() {
            if villain.mask() & dead.bits() == 0 && hero.mask() & villain.mask() == 0 {
                result[usize::from(hero.id())] += reach[usize::from(villain.id())] * utility;
            }
        }
    }
    result
}

fn assert_close(actual: &[f64; 1326], expected: &[f64; 1326], scale: f64) {
    // The independent direct summation has up to 1081 rounded additions, unlike
    // exact sweep bucket sums. Bound its rounding with total absolute payoffs,
    // rather than an output-relative tolerance near zero.
    let allowance = 2048.0 * f64::EPSILON * scale.max(1.0);
    for (id, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(actual.is_finite());
        assert!(
            (actual - expected).abs() <= allowance,
            "combo {id}: actual {actual:e}, expected {expected:e}, allowance {allowance:e}"
        );
    }
}

fn check_showdown(board: [Card; 5], reach: &[f64; 1326], utilities: [f64; 3]) {
    let table = ShowdownTable::new(board).unwrap();
    let mut scratch = ShowdownScratch::default();
    let mut actual = [123.0; 1326];
    let start = std::time::Instant::now();
    table.evaluate(reach, OutcomeUtilities::new(utilities[0], utilities[1], utilities[2]).unwrap(), &mut actual, &mut scratch).unwrap();
    let elapsed = start.elapsed();
    let expected = pairwise_showdown(board, reach, utilities);
    let scale = reach.iter().sum::<f64>() * utilities.iter().map(|value| value.abs()).fold(0.0, f64::max);
    assert_close(&actual, &expected, scale);
    assert!(table.storage_bytes() + scratch.storage_bytes() < 1_048_576);
    eprintln!("showdown traversal {elapsed:?}; table {} bytes; scratch {} bytes", table.storage_bytes(), scratch.storage_bytes());
}

// SplitMix64 supplies reproducible test data only. Rejection sampling keeps each
// distinct board draw uniform without introducing a dependency or runtime RNG.
struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, length: u64) -> usize {
        let threshold = length.wrapping_neg() % length;
        loop {
            let value = self.next();
            if value >= threshold {
                return (value % length) as usize;
            }
        }
    }

    fn board(&mut self) -> [Card; 5] {
        let mut deck: Vec<_> = Card::all().collect();
        std::array::from_fn(|_| {
            let index = self.index(deck.len() as u64);
            deck.swap_remove(index)
        })
    }

    fn reach(&mut self, sparse: bool) -> [f64; 1326] {
        std::array::from_fn(|_| {
            if sparse && self.index(8) != 0 {
                0.0
            } else {
                // Exactly represented dyadic weights isolate sweep correctness
                // from irrelevant random-number decimal conversion.
                self.index(1025) as f64 / 1024.0
            }
        })
    }
}

#[test]
fn full_and_sparse_ranges_match_pairwise_on_varied_boards() {
    let boards = [
        board(["As", "Ks", "Qs", "Js", "Ts"]),
        board(["Ac", "Ad", "Ah", "As", "2c"]),
        board(["2c", "3d", "4h", "5s", "9c"]),
        board(["Kh", "Kd", "7s", "7c", "2d"]),
        board(["Ah", "Jh", "9h", "4h", "2c"]),
    ];
    let mut generator = Generator(0x313ad9ed2a1ce603);
    for board in boards {
        check_showdown(board, &[1.0; 1326], [11.0, 2.0, -7.0]);
        check_showdown(board, &generator.reach(true), [3.25, -0.5, -4.25]);
    }
    for _ in 0..8 {
        let board = generator.board();
        check_showdown(board, &generator.reach(false), [1.0, 0.0, -1.0]);
        check_showdown(board, &generator.reach(true), [7.0, 1.5, -4.0]);
    }
}

#[test]
fn all_ties_exact_overlap_and_tiny_live_mass_survive_blocker_subtraction() {
    let board = board(["As", "Ks", "Qs", "Js", "Ts"]);
    let table = ShowdownTable::new(board).unwrap();
    let mut scratch = ShowdownScratch::default();
    let hero = combo("2c2d");
    let mut reach = [0.0; 1326];
    reach[usize::from(hero.id())] = 0.75;
    let mut output = [17.0; 1326];
    table.evaluate(&reach, OutcomeUtilities::new(17.0, 1.0, -11.0).unwrap(), &mut output, &mut scratch).unwrap();
    assert_eq!(output[usize::from(hero.id())], 0.0);
    assert_eq!(output[usize::from(combo("3c4c").id())], 0.75);

    reach[usize::from(hero.id())] = 1e300;
    reach[usize::from(combo("2c3c").id())] = 1e200;
    reach[usize::from(combo("2d4c").id())] = 1e100;
    reach[usize::from(combo("3d4d").id())] = f64::from_bits(1);
    table.evaluate(&reach, OutcomeUtilities::new(0.0, 1.0, 0.0).unwrap(), &mut output, &mut scratch).unwrap();
    assert_eq!(output[usize::from(hero.id())].to_bits(), 1);
    let mut fold = [17.0; 1326];
    evaluate_fold(CardSet::new(&board).unwrap(), &reach, 1.0, &mut fold).unwrap();
    assert_eq!(fold[usize::from(hero.id())].to_bits(), 1);
    assert_eq!(output, fold);
}

#[test]
fn fold_compatibility_matches_direct_enumeration_for_any_dead_set() {
    let mut generator = Generator(0xf19a567ed940c231);
    for count in [0, 3, 4, 5, 51, 52] {
        let cards: Vec<_> = Card::all().take(count).collect();
        let dead = CardSet::new(&cards).unwrap();
        let reach = generator.reach(false);
        let mut output = [17.0; 1326];
        evaluate_fold(dead, &reach, -2.5, &mut output).unwrap();
        let expected = pairwise_fold(dead, &reach, -2.5);
        assert_close(&output, &expected, 2.5 * reach.iter().sum::<f64>());
    }
}

#[test]
fn linearity_weighted_zero_sum_and_reused_scratch() {
    let board = board(["Ac", "Jd", "8h", "5s", "2c"]);
    let table = ShowdownTable::new(board).unwrap();
    let mut scratch = ShowdownScratch::default();
    let mut generator = Generator(0x5f0dedaccfc6a83e);
    let first = generator.reach(false);
    let second = generator.reach(true);
    let combined = std::array::from_fn(|id| 2.0 * first[id] + 0.5 * second[id]);
    let mut first_values = [0.0; 1326];
    let mut second_values = [0.0; 1326];
    let mut combined_values = [0.0; 1326];
    // Contributions 7 and 11: pot 18, hero utilities 11/2/-7; the
    // opponent's utilities for their own win/tie/loss are 7/-2/-11.
    let own = OutcomeUtilities::new(11.0, 2.0, -7.0).unwrap();
    table.evaluate(&first, own, &mut first_values, &mut scratch).unwrap();
    table.evaluate(&second, own, &mut second_values, &mut scratch).unwrap();
    table.evaluate(&combined, own, &mut combined_values, &mut scratch).unwrap();
    let linear = std::array::from_fn(|id| 2.0 * first_values[id] + 0.5 * second_values[id]);
    assert_close(&combined_values, &linear, 11.0 * combined.iter().sum::<f64>());
    let mut opposing_values = [0.0; 1326];
    table.evaluate(&first, OutcomeUtilities::new(7.0, -2.0, -11.0).unwrap(), &mut opposing_values, &mut scratch).unwrap();
    let ev_first: f64 = first.iter().zip(second_values).map(|(reach, value)| reach * value).sum();
    let ev_second: f64 = second.iter().zip(opposing_values).map(|(reach, value)| reach * value).sum();
    let scale = 11.0 * first.iter().sum::<f64>() * second.iter().sum::<f64>();
    assert!((ev_first + ev_second).abs() <= 4096.0 * f64::EPSILON * scale);
    table.evaluate(&[0.0; 1326], own, &mut combined_values, &mut scratch).unwrap();
    assert_eq!(combined_values, [0.0; 1326]);
}

#[test]
fn checked_failures_preserve_outputs_and_scratch_can_be_reused() {
    let board = board(["As", "Ks", "Qs", "Js", "Ts"]);
    let table = ShowdownTable::new(board).unwrap();
    let dead = CardSet::new(&board).unwrap();
    let mut scratch = ShowdownScratch::default();
    let utilities = OutcomeUtilities::new(1.0, 1.0, -1.0).unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.25] {
        let mut reach = [0.0; 1326];
        // Invalid reach is rejected even for a board-blocked combo.
        reach[usize::from(combo("AsKs").id())] = invalid;
        let mut output = [17.0; 1326];
        assert!(matches!(table.evaluate(&reach, utilities, &mut output, &mut scratch), Err(TerminalError::InvalidReach(_))));
        assert_eq!(output, [17.0; 1326]);
        assert!(evaluate_fold(dead, &reach, 1.0, &mut output).is_err());
        assert_eq!(output, [17.0; 1326]);
        assert!(OutcomeUtilities::new(invalid, 0.0, 0.0).is_err() || invalid.is_finite());
    }
    for reach in [[f64::MAX; 1326], [f64::from_bits(1); 1326]] {
        let tiny = reach[0] == f64::from_bits(1);
        let utility = if tiny { f64::from_bits(1) } else { 1.0 };
        let mut output = [17.0; 1326];
        assert!(table.evaluate(&reach, OutcomeUtilities::new(0.0, utility, 0.0).unwrap(), &mut output, &mut scratch).is_err());
        assert_eq!(output, [17.0; 1326]);
        assert!(evaluate_fold(dead, &reach, utility, &mut output).is_err());
        assert_eq!(output, [17.0; 1326]);
    }
    let mut output = [17.0; 1326];
    assert!(evaluate_fold(dead, &[0.0; 1326], f64::NAN, &mut output).is_err());
    assert_eq!(output, [17.0; 1326]);
    table.evaluate(&[-0.0; 1326], utilities, &mut output, &mut scratch).unwrap();
    assert_eq!(output, [0.0; 1326]);
    assert!(ShowdownTable::new([board[0]; 5]).is_err());
}
