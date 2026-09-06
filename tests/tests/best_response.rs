//! CFR-independent utility, best-response and card-removal checks.

use postflop::{Game, NodeKind, Strategy, best_response, expected_value, exploitability};
use toygames::{history_oracle::HistoryOracle, kuhn, leduc};

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-12, "{actual:.17} != {expected:.17}");
}

#[test]
fn uniform_profiles_match_independent_published_constants() {
    for (game, nash, infosets) in [(kuhn::game(), 11.0 / 12.0, 12), (leduc::game(), 4.747222222222222, 936)] {
        let uniform = Strategy::uniform(&game).unwrap();
        let oracle = HistoryOracle::new(&game).unwrap();
        let metrics = exploitability(&game, &uniform).unwrap();
        close(metrics.nash_conv, nash);
        close(metrics.average, nash / 2.0);
        close(metrics.pct_of_pot, 100.0 * metrics.average / game.starting_pot());
        close(metrics.nash_conv, metrics.pct_of_pot * game.starting_pot() / 50.0);
        assert_eq!(game.legal_information_sets(), infosets);
        for player in 0..2 {
            close(metrics.br_value[player], oracle.best_response(&uniform, player));
            close(expected_value(&game, &uniform, player).unwrap(), oracle.expected_value(&uniform, player));
        }
    }
    let game = kuhn::game();
    let uniform = Strategy::uniform(&game).unwrap();
    close(best_response(&game, &uniform, 0).unwrap(), 0.5);
    close(best_response(&game, &uniform, 1).unwrap(), 5.0 / 12.0);
    close(expected_value(&game, &uniform, 0).unwrap(), 0.125);
}

#[test]
fn known_kuhn_equilibrium_has_zero_deviation_gain() {
    let game = kuhn::game();
    let mut rows = Strategy::uniform(&game).unwrap().rows().to_vec();
    for (history, second_action) in [
        ("", [1.0 / 3.0, 0.0, 1.0]),
        ("c", [1.0 / 3.0, 0.0, 1.0]),
        ("r", [0.0, 1.0 / 3.0, 1.0]),
        ("cr", [0.0, 2.0 / 3.0, 1.0]),
    ] {
        let node = game.node_for_history(history).unwrap() as usize;
        for (hand, probability) in second_action.iter().enumerate() {
            rows[node][hand * 2] = 1.0 - probability;
            rows[node][hand * 2 + 1] = *probability;
        }
    }
    let strategy = Strategy::from_rows(&game, rows).unwrap();
    close(exploitability(&game, &strategy).unwrap().nash_conv, 0.0);
    close(expected_value(&game, &strategy, 0).unwrap(), -1.0 / 18.0);
    let oracle = HistoryOracle::new(&game).unwrap();
    close(oracle.best_response(&strategy, 0), -1.0 / 18.0);
    close(oracle.best_response(&strategy, 1), 1.0 / 18.0);
}

#[test]
fn each_compatible_pair_has_exactly_one_unit_of_board_chance() {
    let game = leduc::game();
    for id in 0..game.num_nodes() as u32 {
        if let NodeKind::Chance { num_outcomes } = game.kind(id) {
            for first in 0..6 {
                for second in 0..6 {
                    if first == second { continue; }
                    let mass: f64 = (0..usize::from(num_outcomes)).map(|board| {
                        game.chance_prob(id, board) * game.chance_mask(id, board, 0)[first] * game.chance_mask(id, board, 1)[second]
                    }).sum();
                    close(mass, 1.0);
                }
            }
        }
    }
}

#[test]
fn fold_payouts_ties_and_physical_blockers() {
    let kuhn = kuhn::game();
    let fold = kuhn.node_for_history("rf").unwrap();
    let mut out = [0.0; 3];
    kuhn.terminal_values(fold, 0, &[1.0, 2.0, 3.0], &mut out);
    assert_eq!(out, [5.0, 4.0, 3.0]);
    kuhn.terminal_values(fold, 1, &[1.0, 2.0, 3.0], &mut out);
    assert_eq!(out, [-5.0, -4.0, -3.0]);
    let leduc = leduc::game();
    let showdown = leduc.node_for_history("cc/4/cc").unwrap();
    let mut out = [0.0; 6];
    leduc.terminal_values(showdown, 0, &[0.0, 1.0, 0.0, 0.0, 0.0, 0.0], &mut out);
    close(out[0], 0.0); // Same rank, different physical cards tie.
    close(out[1], 0.0); // The same physical card is an impossible deal.
    close(out[4], 0.0); // A private card cannot also be the board.
    close(out[5], 1.0); // Pair of kings beats the opponent's jack.
}
