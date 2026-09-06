use super::*;
use std::sync::OnceLock;
use std::time::Instant;

mod oracle;

fn hand<const N: usize>(text: &str) -> [Card; N] {
    assert_eq!(text.len(), 2 * N);
    std::array::from_fn(|i| text[2 * i..2 * i + 2].parse().unwrap())
}

fn correspondence() -> &'static [u32] {
    static SCORES: OnceLock<Vec<u32>> = OnceLock::new();
    SCORES.get_or_init(|| {
        let started = Instant::now();
        let deck: [Card; 52] = std::array::from_fn(|id| Card::from_id(id as u8).unwrap());
        let mut scores = vec![0_u32; 10 << 12];
        let mut counts = [0_u64; 9];
        for a in 0..48 {
            for b in a + 1..49 {
                for c in b + 1..50 {
                    for d in c + 1..51 {
                        for e in d + 1..52 {
                            let cards = [deck[a], deck[b], deck[c], deck[d], deck[e]];
                            let expected = oracle::five(cards);
                            let evaluated = evaluate_five(cards).unwrap();
                            let category = (expected >> 20) as usize;
                            assert_eq!(evaluated.category() as usize, category, "{cards:?}");
                            counts[category] += 1;
                            let slot = &mut scores[usize::from(evaluated.0.to_raw())];
                            if *slot == 0 { *slot = expected; }
                            else { assert_eq!(*slot, expected, "incorrect tie: {cards:?}"); }
                        }
                    }
                }
            }
        }
        assert_eq!(counts, [1_302_540, 1_098_240, 123_552, 54_912, 10_200, 5_108, 3_744, 624, 40]);
        let mut previous = 0;
        let mut distinct = 0;
        for &score in &scores {
            if score != 0 {
                assert!(score > previous, "backend classes are split or misordered");
                previous = score;
                distinct += 1;
            }
        }
        assert_eq!(distinct, 7_462);
        eprintln!("five_card_exhaustive hands=2598960 classes={distinct} elapsed_seconds={:.3}", started.elapsed().as_secs_f64());
        scores
    })
}

struct Samples(u64);

impl Samples {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn hand(&mut self) -> [Card; 7] {
        let mut mask = 0_u64;
        std::array::from_fn(|_| loop {
            let value = self.next();
            // Reject the short tail before modular reduction, then duplicates.
            if value < 52_u64.wrapping_neg() % 52 { continue; }
            let card = Card::from_id((value % 52) as u8).unwrap();
            if mask & card.mask() == 0 {
                mask |= card.mask();
                break card;
            }
        })
    }
}

#[test]
fn all_five_card_hands_match_independent_categories_ties_and_ordering() {
    correspondence();
}

#[test]
fn seven_cards_match_ten_million_independent_subset_oracles() {
    let scores = correspondence();
    let started = Instant::now();
    let mut samples = Samples(0x6173_7472_612d_7632);
    for sample in 1..=10_000_000 {
        let cards = samples.hand();
        let expected = oracle::seven(cards);
        let actual = evaluate_seven(cards).unwrap();
        assert_eq!(scores[usize::from(actual.0.to_raw())], expected, "sample={sample}, cards={cards:?}");
        if sample == 100_000 || sample % 1_000_000 == 0 {
            eprintln!("seven_card_oracle samples={sample} elapsed_seconds={:.3}", started.elapsed().as_secs_f64());
        }
    }
}

#[test]
fn special_hands_and_full_flush_kickers_are_ordered() {
    let cases = [
        ("As2d3h4c5sKdQh", HandCategory::Straight),
        ("AsAhAdKsKhKd2c", HandCategory::FullHouse),
        ("AsAhKsKhQsQh2c", HandCategory::TwoPair),
        ("AsAhAdAcKsQhJd", HandCategory::FourOfAKind),
        ("AsKsQsJsTs9s2d", HandCategory::StraightFlush),
    ];
    for (text, category) in cases {
        let cards = hand::<7>(text);
        let actual = evaluate_seven(cards).unwrap();
        assert_eq!(actual.category(), category);
        assert_eq!(correspondence()[usize::from(actual.0.to_raw())], oracle::seven(cards));
    }
    assert!(evaluate_seven(hand("AsKs9s7s3s2d4h")).unwrap() > evaluate_seven(hand("AsQs9s7s3s2d4h")).unwrap());
    assert!(evaluate_seven(hand("AsAhAdKsKh2d4h")).unwrap() > evaluate_seven(hand("AsKs9s7s3s2d4h")).unwrap());
    assert!(evaluate_seven(hand("As2d3h4c5sKdQh")).unwrap() < evaluate_seven(hand("2d3h4c5s6dKdQh")).unwrap());
}

#[test]
fn card_and_suit_permutations_preserve_strength() {
    let mut samples = Samples(0x736f_6c76_6572_7632);
    for _ in 0..2_000 {
        let cards = samples.hand();
        let expected = evaluate_seven(cards).unwrap();
        let mut reversed = cards;
        reversed.reverse();
        assert_eq!(evaluate_seven(reversed).unwrap(), expected);
        for a in 0..4 {
            for b in 0..4 {
                if a == b { continue; }
                for c in 0..4 {
                    if a == c || b == c { continue; }
                    let d = 6 - a - b - c;
                    let permutation = [a, b, c, d];
                    let changed = cards.map(|card| Card::new(card.rank(), Suit::from_index(permutation[usize::from(card.suit().index())]).unwrap()));
                    assert_eq!(evaluate_seven(changed).unwrap(), expected);
                }
            }
        }
    }
}

#[test]
fn cached_board_matches_direct_evaluation_and_rejects_overlap() {
    for text in ["AcKd7s4h2c", "AsJs8s4s2h", "AcAdAh7s2c", "As2d3h4c5s", "KhKd7s7c2d", "AsKsQsJsTs"] {
        let board = hand::<5>(text);
        let evaluator = RiverEvaluator::new(board).unwrap();
        let dead = CardSet::new(&board).unwrap();
        for combo in Combo::all().filter(|combo| combo.mask() & dead.bits() == 0) {
            let [a, b] = combo.cards();
            assert_eq!(evaluator.evaluate(combo).unwrap(), evaluate_seven([board[0], board[1], board[2], board[3], board[4], a, b]).unwrap());
        }
    }
    let board = hand::<5>("AcKd7s4h2c");
    let evaluator = RiverEvaluator::new(board).unwrap();
    let mask = CardSet::new(&board).unwrap();
    for combo in Combo::all() {
        if combo.mask() & mask.bits() != 0 {
            assert!(evaluator.evaluate(combo).is_err());
            assert!(evaluate_holdem(board, combo).is_err());
        } else {
            let [a, b] = combo.cards();
            assert_eq!(evaluator.evaluate(combo).unwrap(), evaluate_seven([board[0], board[1], board[2], board[3], board[4], a, b]).unwrap());
        }
    }
    assert!(evaluate_five(hand("AcAc7s4h2c")).is_err());
    assert!(evaluate_seven(hand("AcKd7s4h2cAcQs")).is_err());
    assert!(RiverEvaluator::new(hand("AcAc7s4h2c")).is_err());
    let royal = RiverEvaluator::new(hand("AsKsQsJsTs")).unwrap();
    assert_eq!(royal.evaluate("2c3d".parse().unwrap()).unwrap(), royal.evaluate("AhAd".parse().unwrap()).unwrap());
}

#[test]
fn report_checked_evaluation_workload_timings() {
    use std::hint::black_box;
    let mut samples = Samples(0x6265_6e63_686d_6172);
    let inputs: Vec<_> = (0..100_000).map(|_| samples.hand()).collect();
    let mut durations = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        for cards in &inputs { black_box(evaluate_seven(black_box(*cards)).unwrap()); }
        durations.push(start.elapsed().as_secs_f64());
    }
    durations.sort_by(f64::total_cmp);
    let board = hand::<5>("AcKd7s4h2c");
    let dead = CardSet::new(&board).unwrap();
    let combos: Vec<_> = Combo::all().filter(|combo| combo.mask() & dead.bits() == 0).collect();
    let setup = Instant::now();
    let evaluator = black_box(RiverEvaluator::new(black_box(board)).unwrap());
    let setup_seconds = setup.elapsed().as_secs_f64();
    let start = Instant::now();
    for _ in 0..100 {
        for combo in &combos { black_box(evaluator.evaluate(black_box(*combo)).unwrap()); }
    }
    let cached_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    for _ in 0..100 {
        for combo in &combos { black_box(evaluate_holdem(black_box(board), black_box(*combo)).unwrap()); }
    }
    eprintln!("evaluation_timing os={} arch={} profile=test_opt2 checked_seven_hands=100000 median_seconds={:.6} board_setup_seconds={setup_seconds:.9} cached_hands={} cached_seconds={cached_seconds:.6} rebuilt_board_seconds={:.6} backend_static_bytes=312320 river_evaluator_bytes={}", std::env::consts::OS, std::env::consts::ARCH, durations[2], 100 * combos.len(), start.elapsed().as_secs_f64(), std::mem::size_of::<RiverEvaluator>());
}
