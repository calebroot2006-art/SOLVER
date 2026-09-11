//! Independent river accounting, small-state parity, and resource/failure checks.
use cards::{Card, Combo, Range, evaluate_holdem};
use postflop::{
    Cfr, Game, NodeId, NodeKind, Real, RiverGame, RiverSolver, RiverStrategy, SolveConfig,
    SolveError, StopReason, Variant, expected_value, exploitability,
};
use tree::{Action, BetSizeOptions, RiverNodeKind, RiverTree, RiverTreeConfig, Terminal};

fn board(text: &str) -> [Card; 5] {
    text.split_ascii_whitespace()
        .map(|card| card.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn config(bets: &str, raises: &str) -> RiverTreeConfig {
    let sizes = BetSizeOptions::try_from((bets, raises)).unwrap();
    RiverTreeConfig {
        starting_pot: 10,
        effective_stack: 20,
        min_bet: 1,
        sizes: [sizes.clone(), sizes],
        max_raises: 0,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 1000,
    }
}

fn small() -> RiverGame {
    RiverGame::new(
        board("2c 3d 7h 9s Tc"),
        [
            Range::parse("AsAh,KsKh:0.5,QsQh:0.25").unwrap(),
            Range::parse("AdAc,AsKs:0.5,JhJd:0.25").unwrap(),
        ],
        RiverTree::new(config("50%", "")).unwrap(),
        64 * 1024 * 1024,
    )
    .unwrap()
}

fn close(a: f64, b: f64) {
    assert!(a.is_finite() && b.is_finite());
    assert!(
        (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())),
        "{a:e} != {b:e}"
    );
}

fn terminal_payoff(
    game: &RiverGame,
    node: NodeId,
    player: usize,
    hero: Combo,
    villain: Combo,
) -> f64 {
    let state = game.tree().node(node).unwrap();
    let RiverNodeKind::Terminal(terminal) = state.kind() else {
        panic!("not terminal");
    };
    let share = match terminal {
        Terminal::Fold { winner } => {
            if winner as usize == player {
                1.0
            } else {
                0.0
            }
        }
        Terminal::Showdown => match evaluate_holdem(game.board(), hero)
            .unwrap()
            .cmp(&evaluate_holdem(game.board(), villain).unwrap())
        {
            std::cmp::Ordering::Less => 0.0,
            std::cmp::Ordering::Equal => 0.5,
            std::cmp::Ordering::Greater => 1.0,
        },
    };
    let chips = state.contributions();
    let pot = game.tree().config().starting_pot as f64;
    share * (pot + chips.iter().sum::<u64>() as f64) - (pot / 2.0 + chips[player] as f64)
}

struct Sparse {
    game: RiverGame,
    hands: [Vec<Combo>; 2],
    weights: [Vec<f64>; 2],
}

impl Sparse {
    fn new(game: &RiverGame) -> Self {
        let hands: [Vec<Combo>; 2] = std::array::from_fn(|p| {
            Combo::all()
                .filter(|h| game.initial_weights(p).unwrap()[usize::from(h.id())] > 0.0)
                .collect()
        });
        let weights = std::array::from_fn(|p| {
            hands[p]
                .iter()
                .map(|h| game.initial_weights(p).unwrap()[usize::from(h.id())])
                .collect()
        });
        Self {
            game: game.clone(),
            hands,
            weights,
        }
    }
}

impl Game for Sparse {
    fn num_nodes(&self) -> usize {
        self.game.tree().nodes().len()
    }
    fn root(&self) -> NodeId {
        self.game.tree().root()
    }
    fn kind(&self, id: NodeId) -> NodeKind {
        let node = &self.game.tree().nodes()[id as usize];
        match node.kind() {
            RiverNodeKind::Decision { player } => NodeKind::Player {
                player,
                num_actions: node.actions().len() as u8,
            },
            RiverNodeKind::Terminal(_) => NodeKind::Terminal,
        }
    }
    fn child(&self, id: NodeId, action: usize) -> NodeId {
        self.game.tree().nodes()[id as usize].children()[action]
    }
    fn num_private_states(&self, p: usize) -> usize {
        self.hands[p].len()
    }
    fn initial_weights(&self, p: usize) -> &[Real] {
        &self.weights[p]
    }
    fn compatible(&self, a: usize, b: usize) -> bool {
        self.hands[0][a].mask() & self.hands[1][b].mask() == 0
    }
    fn chance_prob(&self, _: NodeId, _: usize) -> Real {
        0.0
    }
    fn chance_mask(&self, _: NodeId, _: usize, _: usize) -> &[Real] {
        &[]
    }
    fn starting_pot(&self) -> Real {
        self.game.tree().config().starting_pot as f64
    }
    fn info_label(&self, id: NodeId, p: usize, hand: usize) -> String {
        format!("{id}:{p}:{}", self.hands[p][hand])
    }
    fn terminal_values(&self, id: NodeId, p: usize, opponent: &[Real], output: &mut [Real]) {
        for (index, hero) in self.hands[p].iter().enumerate() {
            output[index] = self.hands[1 - p]
                .iter()
                .zip(opponent)
                .filter(|(villain, _)| hero.mask() & villain.mask() == 0)
                .map(|(villain, reach)| reach * terminal_payoff(&self.game, id, p, *hero, *villain))
                .sum();
        }
    }
}

// Enumerate all pure responses for one own hand. A choice is fixed per public
// node before any opposing hand is considered, preventing a perfect-information BR.
fn oracle_value(strategy: &RiverStrategy, player: usize, maximize: bool) -> f64 {
    let game = strategy.game();
    let hands = Sparse::new(game);
    let own_nodes: Vec<_> = game
        .tree()
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            (node.kind()
                == (RiverNodeKind::Decision {
                    player: player as u8,
                }))
            .then_some(id)
        })
        .collect();
    assert!(own_nodes.len() < 8);
    let mut total = 0.0;
    let mut normalizer = 0.0;
    for (h, &hero) in hands.hands[player].iter().enumerate() {
        let mut best = f64::NEG_INFINITY;
        for pattern in 0..if maximize { 1 << own_nodes.len() } else { 1 } {
            let mut choices = vec![None; game.tree().nodes().len()];
            if maximize {
                for (bit, &id) in own_nodes.iter().enumerate() {
                    choices[id] = Some((pattern >> bit) & 1);
                }
            }
            let mut own_value = 0.0;
            for (v, &villain) in hands.hands[1 - player].iter().enumerate() {
                if hero.mask() & villain.mask() != 0 {
                    continue;
                }
                own_value += hands.weights[1 - player][v]
                    * pair_value(strategy, 0, player, hero, villain, &choices);
                if pattern == 0 {
                    normalizer += hands.weights[player][h] * hands.weights[1 - player][v];
                }
            }
            best = best.max(own_value);
        }
        total += hands.weights[player][h] * best;
    }
    total / normalizer
}

fn pair_value(
    strategy: &RiverStrategy,
    id: NodeId,
    player: usize,
    hero: Combo,
    villain: Combo,
    choices: &[Option<usize>],
) -> f64 {
    let game = strategy.game();
    let node = game.tree().node(id).unwrap();
    let RiverNodeKind::Decision { player: actor } = node.kind() else {
        return terminal_payoff(game, id, player, hero, villain);
    };
    if let Some(action) = choices[id as usize] {
        return pair_value(
            strategy,
            node.children()[action],
            player,
            hero,
            villain,
            choices,
        );
    }
    let hand = if actor as usize == player {
        hero
    } else {
        villain
    };
    let probabilities = strategy.row(id, hand).unwrap();
    node.children()
        .iter()
        .zip(probabilities)
        .map(|(child, probability)| {
            probability * pair_value(strategy, *child, player, hero, villain, choices)
        })
        .sum()
}

#[test]
fn owned_river_matches_sparse_legacy_updates_and_independent_pure_responses() {
    for variant in [
        Variant::Vanilla,
        Variant::Plus,
        Variant::Discounted {
            alpha: 1.5,
            beta: 0.0,
            gamma: 2.0,
        },
    ] {
        let game = small();
        let sparse = Sparse::new(&game);
        let mut legacy = Cfr::new(&sparse, variant).unwrap();
        let mut river = RiverSolver::new(game.clone(), variant).unwrap();
        for _ in 0..32 {
            legacy.run_iteration(&sparse).unwrap();
            river.run_iteration().unwrap();
            let average = river.average_strategy().unwrap();
            let old_average = legacy.average_strategy(&sparse).unwrap();
            for (id, node) in game.tree().nodes().iter().enumerate() {
                if let RiverNodeKind::Decision { player } = node.kind() {
                    let n = node.actions().len();
                    for (old, hand) in sparse.hands[player as usize].iter().enumerate() {
                        for action in 0..n {
                            let a = usize::from(hand.id()) * n + action;
                            let b = old * n + action;
                            close(
                                river.current_row(id as NodeId).unwrap().unwrap()[a],
                                legacy
                                    .current_strategy()
                                    .unwrap()
                                    .row(id as NodeId)
                                    .unwrap()[b],
                            );
                            close(
                                river.regrets(id as NodeId).unwrap().unwrap()[a],
                                legacy.regrets(id as NodeId).unwrap()[b],
                            );
                            close(
                                river.strategy_sum(id as NodeId).unwrap().unwrap()[a],
                                legacy.strategy_sum(id as NodeId).unwrap()[b],
                            );
                            close(
                                average.node_row(id as NodeId).unwrap()[a],
                                old_average.row(id as NodeId).unwrap()[b],
                            );
                        }
                    }
                }
            }
        }
        let average = river.average_strategy().unwrap();
        let old_average = legacy.average_strategy(&sparse).unwrap();
        for player in 0..2 {
            close(
                average.expected_value(player).unwrap(),
                expected_value(&sparse, &old_average, player).unwrap(),
            );
            close(
                average.expected_value(player).unwrap(),
                oracle_value(&average, player, false),
            );
            close(
                average.best_response(player).unwrap(),
                oracle_value(&average, player, true),
            );
        }
        close(
            average.exploitability().unwrap().pct_of_pot,
            exploitability(&sparse, &old_average).unwrap().pct_of_pot,
        );
    }
}

#[test]
fn root_mass_blockers_scaling_and_forced_showdowns_have_known_values() {
    let cfg = config("", "");
    let full = RiverGame::new(
        board("2c 3d 7h 9s Tc"),
        [
            Range::from_weights([1.0; 1326]).unwrap(),
            Range::from_weights([1.0; 1326]).unwrap(),
        ],
        RiverTree::new(cfg.clone()).unwrap(),
        8 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(full.compatible_weight(), 1_070_190.0);
    close(
        RiverStrategy::uniform(&full)
            .unwrap()
            .expected_value(0)
            .unwrap(),
        0.0,
    );
    let ranges = [
        Range::parse("AsAh:1e-200").unwrap(),
        Range::parse("KsKh:1e-250").unwrap(),
    ];
    let game = RiverGame::new(
        full.board(),
        ranges,
        RiverTree::new(cfg).unwrap(),
        8 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(game.compatible_weight(), 1.0);
    let strategy = RiverStrategy::uniform(&game).unwrap();
    assert_eq!(strategy.expected_value(0).unwrap(), 5.0);
    assert_eq!(strategy.expected_value(1).unwrap(), -5.0);
    assert_eq!(strategy.exploitability().unwrap().pct_of_pot, 0.0);
    assert_eq!(strategy.row(0, "2c2d".parse().unwrap()), None);
}

#[test]
fn conditional_action_values_use_history_opponent_mass_and_root_payoff_origin() {
    let game = small();
    let strategy = RiverStrategy::uniform(&game).unwrap();
    let root = game.tree().node(0).unwrap();
    let response = root.children()[root
        .actions()
        .iter()
        .position(|a| *a == Action::Bet(5))
        .unwrap()];
    let report = strategy.decision_values(response).unwrap();
    assert_eq!(report.player(), 1);
    assert_eq!(report.action_count(), 2);
    let opponent_weights = game.initial_weights(0).unwrap();
    for hero in Combo::all() {
        let id = usize::from(hero.id());
        if report.own_reach()[id] == 0.0 {
            continue;
        }
        let mut mass = 0.0;
        let mut call = 0.0;
        let child = game.tree().node(response).unwrap().children()[1];
        for villain in Combo::all() {
            if hero.mask() & villain.mask() != 0 {
                continue;
            }
            let reach = opponent_weights[usize::from(villain.id())] * 0.5;
            if reach == 0.0 {
                continue;
            }
            mass += reach;
            call += reach * terminal_payoff(&game, child, 1, hero, villain);
        }
        close(report.opponent_mass()[id], mass);
        if mass > 0.0 {
            close(report.values()[id * 2].unwrap(), -5.0);
            close(report.values()[id * 2 + 1].unwrap(), call / mass);
        }
    }
    assert!(strategy.decision_values(u32::MAX).is_err());
    assert!(strategy.expected_value(2).is_err());
}

#[test]
fn memory_reservations_bound_retained_snapshots_and_release_on_drop_or_failure() {
    let initial = small();
    let bound = initial.memory_usage().working_set_bound_bytes;
    let game = RiverGame::new(
        initial.board(),
        initial.ranges().clone(),
        initial.tree().clone(),
        bound,
    )
    .unwrap();
    let baseline = game.reserved_bytes();
    let mut held = Vec::new();
    loop {
        match RiverStrategy::uniform(&game) {
            Ok(strategy) => held.push(strategy),
            Err(SolveError::MemoryLimit { .. }) => break,
            Err(error) => panic!("unexpected {error}"),
        }
        assert!(held.len() < 1000);
    }
    assert!(!held.is_empty());
    assert!(game.reserved_bytes() <= bound);
    drop(held);
    assert_eq!(game.reserved_bytes(), baseline);
    assert!(
        RiverSolver::new(
            game.clone(),
            Variant::Discounted {
                alpha: f64::NAN,
                beta: 0.0,
                gamma: 2.0
            }
        )
        .is_err()
    );
    assert_eq!(game.reserved_bytes(), baseline);
    assert!(RiverStrategy::from_rows(&game, vec![]).is_err());
    assert_eq!(game.reserved_bytes(), baseline);
    let strategy = RiverStrategy::uniform(&game).unwrap();
    let before = game.reserved_bytes();
    let report = strategy.decision_values(0).unwrap();
    assert!(game.reserved_bytes() > before);
    drop(report);
    assert_eq!(game.reserved_bytes(), before);
    // An import whose buffer is far larger than the game's own rows is charged
    // what it holds, and the budget refuses it.
    let mut rows: Vec<Vec<f64>> = (0..game.tree().nodes().len())
        .map(|node| strategy.node_row(node as NodeId).unwrap().to_vec())
        .collect();
    rows[0].reserve_exact(bound / 8 + 1);
    assert_eq!(rows[0].len(), strategy.node_row(0).unwrap().len());
    assert!(matches!(
        RiverStrategy::from_rows(&game, rows),
        Err(SolveError::MemoryLimit { .. })
    ));
    assert_eq!(game.reserved_bytes(), before);
}

#[test]
fn cancellation_resumes_complete_iterations_and_snapshots_retain_identity() {
    let game = small();
    let other = small();
    let mut solver = RiverSolver::new(game.clone(), Variant::Vanilla).unwrap();
    let cfg = SolveConfig {
        target_pct_of_pot: 0.0,
        max_iterations: 100,
        check_every: 100,
        log_every_secs: 1000,
        threads: 1,
    };
    let report = solver.solve_with_cancel(&cfg, |_| {}, || true).unwrap();
    assert_eq!(report.stop_reason, StopReason::Cancelled);
    assert_eq!(report.iterations, 0);
    let mut calls = 0;
    let report = solver
        .solve_with_cancel(
            &cfg,
            |_| {},
            || {
                calls += 1;
                calls == 3
            },
        )
        .unwrap();
    assert_eq!(report.stop_reason, StopReason::Cancelled);
    assert_eq!(report.iterations, 2);
    let report = solver
        .solve(
            &SolveConfig {
                max_iterations: 3,
                ..cfg
            },
            |_| {},
        )
        .unwrap();
    assert_eq!(report.iterations, 3);
    let strategy = solver.average_strategy().unwrap();
    drop(solver);
    assert!(strategy.is_bound_to(&game));
    assert!(!strategy.is_bound_to(&other));
    assert!(strategy.exploitability().is_ok());
}

#[test]
fn concurrent_snapshots_share_one_budget_and_release_every_reservation() {
    let initial = small();
    let bound = initial.memory_usage().working_set_bound_bytes;
    let game = RiverGame::new(
        initial.board(),
        initial.ranges().clone(),
        initial.tree().clone(),
        bound,
    )
    .unwrap();
    let baseline = game.reserved_bytes();
    let barrier = std::sync::Barrier::new(17);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    let result = RiverStrategy::uniform(&game);
                    barrier.wait();
                    result
                })
            })
            .collect();
        barrier.wait();
        assert!(game.reserved_bytes() <= bound);
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert!(results.iter().any(Result::is_ok));
        for result in &results {
            assert!(matches!(
                result,
                Ok(_) | Err(SolveError::MemoryLimit { .. })
            ));
        }
    });
    assert_eq!(game.reserved_bytes(), baseline);
}

#[test]
fn invalid_deals_memory_and_positive_subnormal_arithmetic_are_rejected() {
    let cfg = config("50%", "");
    let cards = board("2c 3d 7h 9s Tc");
    let overlapping = [Range::parse("AsAh").unwrap(), Range::parse("AsKs").unwrap()];
    assert!(matches!(
        RiverGame::new(
            cards,
            overlapping.clone(),
            RiverTree::new(cfg.clone()).unwrap(),
            8 * 1024 * 1024
        ),
        Err(SolveError::EmptyGame)
    ));
    assert!(
        RiverGame::new(
            [cards[0]; 5],
            overlapping.clone(),
            RiverTree::new(cfg.clone()).unwrap(),
            8 * 1024 * 1024
        )
        .is_err()
    );
    assert!(matches!(
        RiverGame::new(cards, overlapping, RiverTree::new(cfg.clone()).unwrap(), 1),
        Err(SolveError::MemoryLimit { .. })
    ));
    let mut low = Range::parse("KsKh").unwrap();
    low.set_weight("AsAh".parse().unwrap(), f64::from_bits(1))
        .unwrap();
    let game = RiverGame::new(
        cards,
        [low, Range::parse("JdJh").unwrap()],
        RiverTree::new(cfg).unwrap(),
        8 * 1024 * 1024,
    )
    .unwrap();
    let strategy = RiverStrategy::uniform(&game).unwrap();
    assert!(matches!(
        strategy.decision_values(1),
        Err(SolveError::Arithmetic { .. })
    ));
    assert!(matches!(
        strategy.expected_value(1),
        Err(SolveError::Arithmetic { .. })
    ));
    let reversed = RiverGame::new(
        game.board(),
        [game.ranges()[1].clone(), game.ranges()[0].clone()],
        game.tree().clone(),
        8 * 1024 * 1024,
    )
    .unwrap();
    let mut solver = RiverSolver::new(reversed, Variant::Vanilla).unwrap();
    let error = solver.run_iteration().unwrap_err();
    assert!(matches!(error, SolveError::Arithmetic { iteration: 1, .. }));
    assert_eq!(solver.iteration(), 0);
    assert_eq!(solver.run_iteration().unwrap_err(), error);
    assert_eq!(solver.average_strategy().unwrap_err(), error);
    assert_eq!(solver.current_row(0).unwrap_err(), error);
    assert_eq!(solver.regrets(0).unwrap_err(), error);
    assert_eq!(solver.strategy_sum(0).unwrap_err(), error);
    let mut cfg = config("", "");
    cfg.starting_pot = 1;
    cfg.effective_stack = 0;
    let mut rare = Range::parse("KdQd").unwrap();
    rare.set_weight("AsAd".parse().unwrap(), f64::from_bits(1))
        .unwrap();
    let game = RiverGame::new(
        board("2h 3h 4s 5s 9c"),
        [rare, Range::parse("KsKd").unwrap()],
        RiverTree::new(cfg).unwrap(),
        8 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(game.compatible_weight(), f64::from_bits(1));
    assert!(matches!(
        RiverStrategy::uniform(&game).unwrap().expected_value(0),
        Err(SolveError::Arithmetic { .. })
    ));
}
