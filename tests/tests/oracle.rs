//! Cross-check the vector solver against independent scalar poker histories.

use postflop::{Cfr, Game, NodeKind, Strategy, Variant, best_response, expected_value};
use toygames::{ToyGame, history_oracle::HistoryOracle, kuhn, leduc};

fn biased_strategy(game: &ToyGame) -> Strategy {
    let mut rows = Strategy::uniform(game).unwrap().rows().to_vec();
    for (id, row) in rows.iter_mut().enumerate() {
        if let NodeKind::Player { num_actions, .. } = game.kind(id as u32) {
            for (hand, probabilities) in row.chunks_mut(usize::from(num_actions)).enumerate() {
                let mut sum = 0.0;
                for (action, probability) in probabilities.iter_mut().enumerate() {
                    *probability = ((id * 7 + hand * 11 + action * 13) % 19 + 1) as f64;
                    sum += *probability;
                }
                for probability in probabilities {
                    *probability /= sum;
                }
            }
        }
    }
    Strategy::from_rows(game, rows).unwrap()
}

#[test]
fn explicit_histories_match_values_and_legal_best_responses_on_weighted_ranges() {
    let cases = [
        kuhn::game(),
        kuhn::game().with_weights([vec![0.2, 3.0, 0.8], vec![2.0, 0.4, 1.7]]),
        kuhn::game().with_weights([vec![0.0, 1.0, 0.0], vec![0.2, 0.0, 0.7]]),
        leduc::game(),
        leduc::game().with_weights([
            vec![0.0, 0.2, 2.0, 1.0, 0.0, 0.7],
            vec![0.4, 1.7, 0.0, 0.5, 3.0, 0.0],
        ]),
    ];
    for game in cases {
        let oracle = HistoryOracle::new(&game).unwrap();
        let policy = biased_strategy(&game);
        for player in 0..2 {
            let ev = expected_value(&game, &policy, player).unwrap();
            let br = best_response(&game, &policy, player).unwrap();
            assert!((ev - oracle.expected_value(&policy, player)).abs() < 1e-12);
            assert!((br - oracle.best_response(&policy, player)).abs() < 1e-12);
            assert!(br + 1e-12 >= ev);
        }
        let scaled = game.clone().with_weights([
            game.initial_weights(0).iter().map(|w| w * 7.0).collect(),
            game.initial_weights(1).iter().map(|w| w * 0.125).collect(),
        ]);
        let scaled_policy = Strategy::from_rows(&scaled, policy.rows().to_vec()).unwrap();
        for player in 0..2 {
            assert!(
                (expected_value(&game, &policy, player).unwrap()
                    - expected_value(&scaled, &scaled_policy, player).unwrap())
                .abs()
                    < 1e-12
            );
            assert!(
                (best_response(&game, &policy, player).unwrap()
                    - best_response(&scaled, &scaled_policy, player).unwrap())
                .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn scalar_cfr_matches_vector_iteration_and_averaging_on_sparse_blocked_deals() {
    for game in [
        kuhn::game(),
        kuhn::game().with_weights([vec![0.2, 3.0, 0.8], vec![2.0, 0.4, 1.7]]),
        kuhn::game().with_weights([vec![0.0, 1.0, 0.0], vec![0.2, 0.0, 0.7]]),
        leduc::game().with_weights([
            vec![0.0, 0.2, 2.0, 1.0, 0.0, 0.7],
            vec![0.4, 1.7, 0.0, 0.5, 3.0, 0.0],
        ]),
    ] {
        let oracle = HistoryOracle::new(&game).unwrap();
        for variant in [
            Variant::Vanilla,
            Variant::Plus,
            Variant::Discounted {
                alpha: 1.5,
                beta: 0.0,
                gamma: 2.0,
            },
        ] {
            let mut solver = Cfr::new(&game, variant).unwrap();
            for iterations in 1..=3 {
                solver.run_iteration(&game).unwrap();
                let (current, average) = oracle.cfr(&game, variant, iterations).unwrap();
                for (expected, actual) in [
                    (current, solver.current_strategy().unwrap().clone()),
                    (average, solver.average_strategy(&game).unwrap()),
                ] {
                    for (id, row) in expected.rows().iter().enumerate() {
                        let NodeKind::Player {
                            player,
                            num_actions,
                        } = game.kind(id as u32)
                        else {
                            continue;
                        };
                        for (entry, want) in row.iter().enumerate() {
                            let hand = entry / usize::from(num_actions);
                            if game.initial_weights(usize::from(player))[hand] == 0.0
                                || game.board(id as u32) == Some(hand)
                            {
                                continue;
                            }
                            let got = actual.row(id as u32).unwrap()[entry];
                            assert!(
                                (got - want).abs() < 1e-12,
                                "{variant:?} iteration={iterations} {} entry={entry}: {got} != {want}",
                                game.info_label(id as u32, usize::from(player), hand)
                            );
                        }
                    }
                }
            }
        }
    }
}
