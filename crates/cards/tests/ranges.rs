//! Syntax, blocker, failure, and exact-weight roundtrip checks for weighted ranges.

use cards::{Card, CardSet, Combo, MAX_RANGE_BYTES, MAX_RANGE_TOKENS, Range, RangeError};

fn nonzero(range: &Range) -> usize {
    range
        .weights()
        .iter()
        .filter(|&&weight| weight > 0.0)
        .count()
}

fn same_bits(first: &Range, second: &Range) {
    for combo in Combo::all() {
        assert_eq!(
            first.weight(combo).to_bits(),
            second.weight(combo).to_bits(),
            "{combo}"
        );
    }
}

#[test]
fn shorthand_classes_select_physical_combos_without_normalizing_weights() {
    for (text, count) in [("AA", 6), ("AKs", 4), ("AKo", 12), ("AK", 16), ("AsKh", 1)] {
        let range = Range::parse(&format!("{text}:0.25")).unwrap();
        assert_eq!(nonzero(&range), count);
        assert_eq!(range.weights().iter().sum::<f64>(), count as f64 * 0.25);
        for combo in Combo::all() {
            let [a, b] = combo.cards();
            let expected = match text {
                "AA" => a.rank().index() == 12 && b.rank().index() == 12,
                "AKs" => a.rank().index() == 11 && b.rank().index() == 12 && a.suit() == b.suit(),
                "AKo" => a.rank().index() == 11 && b.rank().index() == 12 && a.suit() != b.suit(),
                "AK" => a.rank().index() == 11 && b.rank().index() == 12,
                _ => combo == "AsKh".parse::<Combo>().unwrap(),
            };
            assert_eq!(range.weight(combo), if expected { 0.25 } else { 0.0 });
        }
    }
}

#[test]
fn plus_and_interval_expansions_cover_exactly_the_documented_classes() {
    for (expression, expanded) in [
        ("99+", "99 TT JJ QQ KK AA"),
        ("ATs+", "ATs AJs AQs AKs"),
        ("KTo+", "KTo KJo KQo"),
        ("AT+", "AT AJ AQ AK"),
        ("AA+", "AA"),
        ("AKs+", "AKs"),
        ("99-JJ", "99 TT JJ"),
        ("JJ-99", "99 TT JJ"),
        ("A2s-A5s", "A2s A3s A4s A5s"),
        ("A5s-A2s", "A2s A3s A4s A5s"),
        ("65s-T9s", "65s 76s 87s 98s T9s"),
        ("T9s-65s", "65s 76s 87s 98s T9s"),
        ("64o-T8o", "64o 75o 86o 97o T8o"),
        ("64-T8", "64 75 86 97 T8"),
        ("AKo-AKo", "AKo"),
    ] {
        assert_eq!(
            Range::parse(expression).unwrap(),
            Range::parse(expanded).unwrap(),
            "{expression}"
        );
    }
    let weighted = Range::parse("99+:0.5, ATs+:0.25, 65s-T9s:0.125").unwrap();
    assert_eq!(nonzero(&weighted), 72);
    assert_eq!(weighted.weight("AsAh".parse().unwrap()), 0.5);
    assert_eq!(weighted.weight("AsTs".parse().unwrap()), 0.25);
    assert_eq!(weighted.weight("Ts9s".parse().unwrap()), 0.125);
}

#[test]
fn whitespace_empty_ranges_and_equal_overlaps_are_supported() {
    assert_eq!(Range::parse("AA\u{b}KK").unwrap(), Range::parse("AA KK").unwrap());
    assert_eq!(Range::parse("").unwrap(), Range::empty());
    assert_eq!(Range::parse(" \t\r\n\x0b\x0c").unwrap(), Range::empty());
    assert_eq!(Range::empty().to_string(), "");
    assert_eq!(
        Range::parse(" AA\tKK,\n QQ JJ \r\n").unwrap(),
        Range::parse("AA,KK,QQ,JJ").unwrap()
    );
    let range = Range::parse("AK:0.5 AKs:5e-1 AsKh:0.50 KhAs:0.5").unwrap();
    assert_eq!(range, Range::parse("AK:0.5").unwrap());
    assert_eq!(Range::parse("AA:0,AA:-0").unwrap(), Range::empty());
}

#[test]
fn conflicting_overlaps_report_the_token_and_physical_combo() {
    match Range::parse("AK:0.5 AsKh:0.25").unwrap_err() {
        RangeError::ConflictingAssignment {
            token,
            combo,
            previous,
            incoming,
        } => {
            assert_eq!(token, "AsKh:0.25");
            assert_eq!(combo, "AsKh".parse().unwrap());
            assert_eq!(previous, 0.5);
            assert_eq!(incoming, 0.25);
        }
        other => panic!("unexpected error: {other}"),
    }
    for text in ["AA:0 AA", "AA AA:0", "AsKh:1 KhAs:0.5", "AKs AK:0.5"] {
        assert!(
            matches!(
                Range::parse(text),
                Err(RangeError::ConflictingAssignment { .. })
            ),
            "{text}"
        );
    }
}

#[test]
fn malformed_syntax_and_invalid_weights_fail() {
    for text in [
        ",",
        ",AA",
        "AA,",
        "AA,,KK",
        "AA, \t,KK",
        "AA:50%",
        "AA:",
        "AA::0.5",
        ":0.5",
        "AA:NaN",
        "AA:inf",
        "AA:-inf",
        "AA:1.0001",
        "AA:-0.01",
        "AA:1e9999",
        "AA:1/2",
        "AA: 0.5",
        "AAs",
        "AAo",
        "AAs+",
        "AAs-KKs",
        "AKs-AQo",
        "AK-AQs",
        "AA-AKs",
        "ATs-K8s",
        "AsKh-KsQh",
        "AsAs",
        "AsKh+",
        "AK++",
        "AKs-",
        "-AKs",
        "AA-KK-QQ",
        "KA",
        "2A",
        "ak",
        "10Ts",
        "AKS",
        "A♠K♠",
        "AA\u{a0}KK",
        "AA\0",
        "AA;KK",
    ] {
        assert!(Range::parse(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn input_limits_are_enforced_before_expansion() {
    assert!(Range::parse(&" ".repeat(MAX_RANGE_BYTES)).is_ok());
    assert!(matches!(
        Range::parse(&" ".repeat(MAX_RANGE_BYTES + 1)),
        Err(RangeError::InputTooLong { .. })
    ));
    assert!(Range::parse(&"AA ".repeat(MAX_RANGE_TOKENS)).is_ok());
    assert!(matches!(
        Range::parse(&"AA ".repeat(MAX_RANGE_TOKENS + 1)),
        Err(RangeError::TooManyTokens { .. })
    ));
    let invalid_then_too_many = format!("invalid {}", "AA ".repeat(MAX_RANGE_TOKENS));
    assert!(matches!(
        Range::parse(&invalid_then_too_many),
        Err(RangeError::TooManyTokens { .. })
    ));
}

#[test]
fn checked_weights_fail_atomically_and_editor_overrides_are_explicit() {
    let combo = "AsKh".parse::<Combo>().unwrap();
    let mut range = Range::parse("AsKh:0.25").unwrap();
    for weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 1.0001] {
        assert!(range.set_weight(combo, weight).is_err());
        assert_eq!(range.weight(combo), 0.25);
        let mut weights = [0.0; 1326];
        weights[combo.id() as usize] = weight;
        assert!(Range::from_weights(weights).is_err());
    }
    range.set_weight(combo, 0.75).unwrap();
    assert_eq!(range.weight(combo), 0.75);
    range.set_weight(combo, -0.0).unwrap();
    assert_eq!(range.weight(combo).to_bits(), 0.0_f64.to_bits());
    assert!(
        Range::from_weights([-0.0; 1326])
            .unwrap()
            .weights()
            .iter()
            .all(|weight| weight.to_bits() == 0)
    );
}

#[test]
fn dead_cards_zero_only_overlapping_combos_and_keep_fractional_weights() {
    let range = Range::from_weights([0.125; 1326]).unwrap();
    for card in Card::all() {
        let dead = CardSet::new(&[card]).unwrap();
        let filtered = range.without_cards(dead);
        assert_eq!(nonzero(&filtered), 1275);
        for combo in Combo::all() {
            assert_eq!(
                filtered.weight(combo),
                if combo.cards().contains(&card) {
                    0.0
                } else {
                    0.125
                }
            );
        }
    }
    let board: Vec<Card> = ["As", "Kh", "Qd", "Jc", "Ts"]
        .into_iter()
        .map(|s| s.parse().unwrap())
        .collect();
    assert_eq!(
        nonzero(&range.without_cards(CardSet::new(&board).unwrap())),
        1081
    );
    assert_eq!(range.without_cards(CardSet::default()), range);
    assert_eq!(
        nonzero(&range.without_cards(CardSet::new(&Card::all().collect::<Vec<_>>()).unwrap())),
        0
    );
    assert_eq!(nonzero(&range), 1326);
}

#[test]
fn canonical_text_preserves_arbitrary_weights_and_tiny_values_within_limits() {
    for weight in [f64::from_bits(1), f64::MIN_POSITIVE, 1e-200, 0.1, 1.0] {
        let range = Range::from_weights([weight; 1326]).unwrap();
        let text = range.to_canonical_string();
        assert!(text.len() <= MAX_RANGE_BYTES);
        assert!(text.split(',').count() <= MAX_RANGE_TOKENS);
        same_bits(&range, &Range::parse(&text).unwrap());
    }
    let mut random = 0xc728_945d_21ef_a360_u64;
    for _ in 0..100 {
        let mut weights = [0.0; 1326];
        for weight in &mut weights {
            random = random.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut bits = random;
            bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            bits ^= bits >> 31;
            *weight = match bits % 5 {
                0 => 0.0,
                1 => 1.0,
                2 => (bits % 1_000_000) as f64 / 1_000_000.0,
                _ => f64::from_bits(bits % (1.0_f64.to_bits() + 1)),
            };
        }
        let range = Range::from_weights(weights).unwrap();
        let text = range.to_string();
        assert!(text.len() <= MAX_RANGE_BYTES);
        same_bits(&range, &text.parse::<Range>().unwrap());
        assert_eq!(Range::parse(&text).unwrap().to_string(), text);
    }
}

#[test]
fn canonical_order_and_distinct_weights_within_a_cell_are_preserved() {
    let mut range = Range::empty();
    let members = Range::combos_for_cell(0, 1).unwrap();
    for (index, combo) in members.iter().enumerate() {
        range.set_weight(*combo, (index + 1) as f64 / 4.0).unwrap();
    }
    let text = range.to_string();
    let printed: Vec<Combo> = text
        .split(',')
        .map(|token| token.split(':').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(printed, members);
    same_bits(&range, &Range::parse(&text).unwrap());
    assert!(Range::combos_for_cell(13, 0).is_err());
}
