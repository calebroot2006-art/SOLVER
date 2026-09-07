//! Street-aware postflop games: river equivalence, the all-in runout against a
//! brute-force enumeration, the chance contract, memory accounting, and a small
//! turn solve that reaches a measured target.

use cards::{Card, Combo, Range, evaluate_seven};
use postflop::{
    NodeId, Precision, RiverGame, RiverSolver, SolveConfig, SolveError, StopReason, Variant,
    streets::{PostflopGame, PostflopOptions, PostflopSolver, PostflopStrategy},
    terminal::{OutcomeUtilities, ShowdownScratch, ShowdownTable},
};
use tree::{
    Action, BetSizeOptions, PostflopNodeKind, PostflopTree, PostflopTreeConfig, RiverTree,
    RiverTreeConfig, Street,
};

const LIMIT: usize = 4 * 1024 * 1024 * 1024;

fn cards(text: &str) -> Vec<Card> {
    text.split_ascii_whitespace()
        .map(|card| card.parse().unwrap())
        .collect()
}

fn menus(bets: &str, raises: &str) -> [BetSizeOptions; 2] {
    let sizes = BetSizeOptions::try_from((bets, raises)).unwrap();
    [sizes.clone(), sizes]
}

fn options(memory_limit_bytes: usize) -> PostflopOptions {
    PostflopOptions {
        memory_limit_bytes,
        precision: Precision::F64,
        threads: 1,
    }
}

/// The three phase 3 river fixtures: only the effective stack differs.
fn river_fixture(effective_stack: u64) -> RiverTreeConfig {
    RiverTreeConfig {
        starting_pot: 10,
        effective_stack,
        min_bet: 1,
        sizes: menus("50%", "100%"),
        max_raises: 32,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 1_000_000,
    }
}

/// The same settings as a river-start postflop tree. The flop and turn menus
/// are absurd on purpose: a river-start tree must never read them.
fn as_postflop(river: &RiverTreeConfig) -> PostflopTreeConfig {
    PostflopTreeConfig {
        starting_pot: river.starting_pot,
        effective_stack: river.effective_stack,
        min_bet: river.min_bet,
        start_street: Street::River,
        sizes: [
            menus("1c,a", "1c,a"),
            menus("999%", "9x"),
            river.sizes.clone(),
        ],
        max_raises: river.max_raises,
        add_all_in_threshold: river.add_all_in_threshold,
        force_all_in_threshold: river.force_all_in_threshold,
        max_nodes: river.max_nodes,
    }
}

/// A turn tree whose only wager is the jam, so every called line runs the board
/// out with no further decision.
fn all_in_turn(effective_stack: u64) -> PostflopTree {
    PostflopTree::new(PostflopTreeConfig {
        starting_pot: 10,
        effective_stack,
        min_bet: 1,
        start_street: Street::Turn,
        sizes: [menus("a", ""), menus("a", ""), menus("a", "")],
        max_raises: 0,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })
    .unwrap()
}

fn child(game: &PostflopGame, node: NodeId, action: Action) -> NodeId {
    let view = game.node(node).unwrap();
    let index = view
        .actions()
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap_or_else(|| panic!("node {node} has no {action}"));
    view.children()[index]
}

fn ranges(oop: &str, ip: &str) -> [Range; 2] {
    [Range::parse(oop).unwrap(), Range::parse(ip).unwrap()]
}

#[test]
fn a_river_start_game_reproduces_the_river_solver_bit_for_bit() {
    let board = cards("Ah Kd 7c 2s 9h");
    let fixed: [Card; 5] = board.as_slice().try_into().unwrap();
    let text = (
        "22+, A2s-AKs, KTs-KQs, QJs, AJo-AKo, KQo",
        "33+, A5s-AKs, K9s-KQs, QTs-QJs, JTs, ATo-AKo, KJo-KQo",
    );

    for stack in [20_u64, 100, 200] {
        let config = river_fixture(stack);
        let river_tree = RiverTree::new(config.clone()).unwrap();
        let postflop_tree = PostflopTree::new(as_postflop(&config)).unwrap();
        assert_eq!(postflop_tree.nodes().len(), river_tree.nodes().len());

        let river_game = RiverGame::new(fixed, ranges(text.0, text.1), river_tree, LIMIT).unwrap();
        let postflop_game = PostflopGame::new(
            &board,
            ranges(text.0, text.1),
            postflop_tree,
            options(LIMIT),
        )
        .unwrap();
        assert_eq!(postflop_game.num_nodes(), river_game.tree().nodes().len());
        assert!(postflop_game.runout_ranges().is_empty());
        assert_eq!(
            postflop_game.compatible_weight(),
            river_game.compatible_weight()
        );
        assert_eq!(
            postflop_game.initial_weights(0),
            river_game.initial_weights(0)
        );

        let variant = Variant::Discounted {
            alpha: 1.5,
            beta: 0.0,
            gamma: 2.0,
        };
        let mut river_solver = RiverSolver::new(river_game, variant).unwrap();
        let mut postflop_solver = PostflopSolver::new(postflop_game.clone(), variant).unwrap();
        for _ in 0..5 {
            river_solver.run_iteration().unwrap();
            postflop_solver.run_iteration().unwrap();
        }
        for id in 0..postflop_game.num_nodes() as NodeId {
            assert_eq!(
                postflop_solver.regrets(id).unwrap(),
                river_solver.regrets(id).unwrap(),
                "regrets differ at node {id} on a {stack}-chip stack"
            );
            assert_eq!(
                postflop_solver.strategy_sum(id).unwrap(),
                river_solver.strategy_sum(id).unwrap(),
                "strategy sums differ at node {id} on a {stack}-chip stack"
            );
            assert_eq!(
                postflop_solver.current_row(id).unwrap(),
                river_solver.current_row(id).unwrap(),
                "current policy differs at node {id} on a {stack}-chip stack"
            );
        }
        let mine = postflop_solver.average_strategy().unwrap();
        let theirs = river_solver.average_strategy().unwrap();
        assert_eq!(mine.rows(), theirs.rows());
        assert_eq!(
            mine.exploitability().unwrap(),
            theirs.exploitability().unwrap()
        );
    }
}

#[test]
fn a_called_turn_all_in_matches_the_river_sweep_and_a_brute_force_enumeration() {
    let board = cards("9c 5d 2h Ks");
    let [oop, ip] = ranges("AA, QQ, JTs", "KK, 99, 76s");
    let game = PostflopGame::new(&board, [oop, ip], all_in_turn(20), options(LIMIT)).unwrap();

    let jam = child(&game, game.root(), Action::AllIn(20));
    let called = child(&game, jam, Action::Call);
    let chance = game.node(called).unwrap();
    assert!(matches!(
        chance.kind(),
        PostflopNodeKind::Chance {
            next: Street::River
        }
    ));
    assert_eq!(chance.possible_cards().len(), 48);
    assert_eq!(chance.children().len(), 48);
    for runout in chance.children() {
        let terminal = game.node(*runout).unwrap();
        assert_eq!(terminal.board().len(), 5);
        assert!(matches!(
            terminal.kind(),
            PostflopNodeKind::Terminal(tree::Terminal::Showdown)
        ));
    }

    // The in-position player faces the jam. Calling leads straight to the deal,
    // so the conditional value of the call is the all-in value of the hand.
    let strategy = PostflopStrategy::uniform(&game).unwrap();
    let report = strategy.decision_values(jam).unwrap();
    assert_eq!(report.player(), 1);
    assert_eq!(report.street(), Street::Turn);
    assert_eq!(report.board(), board.as_slice());
    assert!(report.runout().is_empty());
    let call = game
        .node(jam)
        .unwrap()
        .actions()
        .iter()
        .position(|action| *action == Action::Call)
        .unwrap();
    let actions = report.action_count();

    // The opponent reach the query used: their range times the uniform jam.
    let opponent: Vec<f64> = game
        .initial_weights(0)
        .unwrap()
        .iter()
        .map(|weight| weight * 0.5)
        .collect();
    let amount = 10.0 / 2.0 + 20.0;
    let utilities = OutcomeUtilities::new(amount, 0.0, -amount).unwrap();

    // The phase 3 river sweep, once per runout, combined by the chance rule.
    let mut swept = vec![0.0_f64; 1326];
    let mut scratch = ShowdownScratch::default();
    let probability = 1.0 / 44.0;
    for card in chance.possible_cards() {
        let mut five = board.clone();
        five.push(*card);
        let table = ShowdownTable::new(five.as_slice().try_into().unwrap()).unwrap();
        let mut masked = [0.0_f64; 1326];
        for combo in Combo::all() {
            let id = usize::from(combo.id());
            if combo.mask() & card.mask() == 0 {
                masked[id] = opponent[id] * probability;
            }
        }
        let mut out = [0.0_f64; 1326];
        table
            .evaluate(&masked, utilities, &mut out, &mut scratch)
            .unwrap();
        for combo in Combo::all() {
            let id = usize::from(combo.id());
            if combo.mask() & card.mask() == 0 {
                swept[id] += out[id];
            }
        }
    }

    // The same quantity from scratch, comparing seven cards at a time.
    let live: Vec<Combo> = Combo::all()
        .filter(|combo| board.iter().all(|card| combo.mask() & card.mask() == 0))
        .collect();
    let mut brute = vec![0.0_f64; 1326];
    let mut mass = vec![0.0_f64; 1326];
    for hero in &live {
        let hero_id = usize::from(hero.id());
        for villain in &live {
            let villain_id = usize::from(villain.id());
            if opponent[villain_id] == 0.0 || hero.mask() & villain.mask() != 0 {
                continue;
            }
            mass[hero_id] += opponent[villain_id];
            for card in chance.possible_cards() {
                if (hero.mask() | villain.mask()) & card.mask() != 0 {
                    continue;
                }
                let mut five = board.clone();
                five.push(*card);
                let seven = |combo: &Combo| {
                    let [a, b] = combo.cards();
                    evaluate_seven([five[0], five[1], five[2], five[3], five[4], a, b]).unwrap()
                };
                let value = match seven(hero).cmp(&seven(villain)) {
                    std::cmp::Ordering::Greater => amount,
                    std::cmp::Ordering::Equal => 0.0,
                    std::cmp::Ordering::Less => -amount,
                };
                brute[hero_id] += probability * opponent[villain_id] * value;
            }
        }
    }

    let mut compared = 0;
    for hero in &live {
        let id = usize::from(hero.id());
        assert!(
            (swept[id] - brute[id]).abs() < 1e-9,
            "runout sweep and brute force differ at combo {id}: {} vs {}",
            swept[id],
            brute[id]
        );
        assert!(
            (report.opponent_mass()[id] - mass[id]).abs() < 1e-12,
            "opponent mass differs at combo {id}"
        );
        if mass[id] > 0.0 && report.own_reach()[id] > 0.0 {
            let reported = report.values()[id * actions + call].unwrap();
            assert!(
                (reported - brute[id] / mass[id]).abs() < 1e-9,
                "call value differs at combo {id}: {reported} vs {}",
                brute[id] / mass[id]
            );
            compared += 1;
        }
    }
    assert!(compared >= 8, "only {compared} combos were compared");
}

#[test]
fn the_deal_gives_every_compatible_pair_exactly_one_unit_of_chance_mass() {
    let board = cards("9c 5d 2h Ks");
    let game = PostflopGame::new(
        &board,
        ranges("AA, QQ, JTs", "KK, 99, 76s"),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap();

    let mut chance_nodes = 0;
    for id in 0..game.num_nodes() as NodeId {
        let view = game.node(id).unwrap();
        if !matches!(view.kind(), PostflopNodeKind::Chance { .. }) {
            assert!(view.possible_cards().is_empty());
            assert!(view.chance_probability().is_none());
            continue;
        }
        chance_nodes += 1;
        let probability = view.chance_probability().unwrap();
        assert!((probability - 1.0 / 44.0).abs() < 1e-15);
        let cards = view.possible_cards();
        assert_eq!(cards.len(), 48);
        for hero in Combo::all().take(200) {
            for villain in Combo::all().skip(700).take(50) {
                let used = hero.mask() | villain.mask();
                if hero.mask() & villain.mask() != 0
                    || board.iter().any(|card| used & card.mask() != 0)
                {
                    continue;
                }
                let dealable = cards.iter().filter(|card| used & card.mask() == 0).count();
                let mass = dealable as f64 * probability;
                assert!(
                    (mass - 1.0).abs() < 1e-12,
                    "chance mass at node {id} is {mass}"
                );
            }
        }
    }
    assert!(chance_nodes > 0);

    // The masks themselves: on a river history the compatible opponent mass
    // must exclude every combo holding the card that was dealt.
    let checked = child(&game, game.root(), Action::Check);
    let deal = child(&game, checked, Action::Check);
    let deal_view = game.node(deal).unwrap();
    let river = deal_view.children()[7];
    let dealt = deal_view.possible_cards()[7];
    let strategy = PostflopStrategy::uniform(&game).unwrap();
    let report = strategy.decision_values(river).unwrap();
    assert_eq!(report.street(), Street::River);
    assert_eq!(report.runout(), &[dealt]);
    let opponent = game.initial_weights(1).unwrap();
    // Both players checked, so the in-position range still carries one uniform
    // check, and the deal contributes one forty-fourth.
    let reach = 0.5 / 44.0;
    let mut checked_combos = 0;
    for hero in Combo::all() {
        let id = usize::from(hero.id());
        if hero.mask() & dealt.mask() != 0
            || board.iter().any(|card| hero.mask() & card.mask() != 0)
        {
            continue;
        }
        let expected: f64 = Combo::all()
            .filter(|villain| {
                villain.mask() & hero.mask() == 0
                    && villain.mask() & dealt.mask() == 0
                    && board.iter().all(|card| villain.mask() & card.mask() == 0)
            })
            .map(|villain| opponent[usize::from(villain.id())] * reach)
            .sum();
        assert!(
            (report.opponent_mass()[id] - expected).abs() < 1e-12,
            "river mass at combo {id}: {} vs {expected}",
            report.opponent_mass()[id]
        );
        checked_combos += 1;
    }
    assert!(
        checked_combos > 1000,
        "only {checked_combos} combos checked"
    );
}

#[test]
fn the_estimate_bounds_every_reservation_and_refuses_a_game_it_cannot_hold() {
    let board = cards("9c 5d 2h Ks");
    let text = ("AA, QQ, JTs", "KK, 99, 76s");
    let game = PostflopGame::new(
        &board,
        ranges(text.0, text.1),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap();
    let memory = game.memory_usage();
    println!(
        "turn estimate: {} boards, {} tables, {} nodes, shared {} B, solver {} B,          snapshot {} B, traversal {} B, scratch {} B, decision {} B, bound {} B",
        memory.board_states,
        memory.showdown_tables,
        memory.expanded_nodes,
        memory.shared_bytes,
        memory.solver_bytes,
        memory.snapshot_bytes,
        memory.traversal_bytes,
        memory.scratch_bytes,
        memory.decision_bytes,
        memory.working_set_bound_bytes
    );
    assert_eq!(memory.board_states, 49);
    assert_eq!(memory.showdown_tables, 48);
    assert_eq!(memory.expanded_nodes, game.num_nodes());
    assert_eq!(game.reserved_bytes(), memory.shared_bytes);

    let solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
    assert_eq!(
        game.reserved_bytes(),
        memory.shared_bytes + memory.solver_bytes + memory.scratch_bytes
    );
    {
        let snapshot = solver.average_strategy().unwrap();
        assert_eq!(
            game.reserved_bytes(),
            memory.shared_bytes
                + memory.solver_bytes
                + memory.scratch_bytes
                + memory.snapshot_bytes
        );
        drop(snapshot);
    }
    assert_eq!(
        game.reserved_bytes(),
        memory.shared_bytes + memory.solver_bytes + memory.scratch_bytes
    );
    drop(solver);
    assert_eq!(game.reserved_bytes(), memory.shared_bytes);

    // One byte under the estimate refuses before a single row is allocated.
    let error = PostflopGame::new(
        &board,
        ranges(text.0, text.1),
        all_in_turn(20),
        options(memory.working_set_bound_bytes - 1),
    )
    .unwrap_err();
    match error {
        SolveError::MemoryLimit { required, limit } => {
            assert_eq!(required, memory.working_set_bound_bytes);
            assert_eq!(limit, memory.working_set_bound_bytes - 1);
        }
        other => panic!("expected a memory limit, got {other}"),
    }

    // The phase 4 gate flop tree does not fit at f64 without compaction, and
    // says so instead of trying.
    let flop_tree = PostflopTree::new(PostflopTreeConfig {
        starting_pot: 55,
        effective_stack: 975,
        min_bet: 10,
        start_street: Street::Flop,
        sizes: [
            menus("33%,a", "100%,a"),
            menus("33%,a", "100%,a"),
            menus("33%,75%", "100%,a"),
        ],
        max_raises: 1,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })
    .unwrap();
    let flop_board = cards("9c 5d 2h");
    let error = PostflopGame::new(
        &flop_board,
        ranges(text.0, text.1),
        flop_tree,
        options(12 * 1024 * 1024 * 1024),
    )
    .unwrap_err();
    match error {
        SolveError::MemoryLimit { required, limit } => {
            println!("flop gate estimate: {required} B needed against a {limit} B limit");
            assert!(required > limit, "{required} should exceed {limit}");
            assert!(required > 50_000_000_000, "{required} is implausibly small");
        }
        other => panic!("expected a memory limit, got {other}"),
    }
}

#[test]
fn malformed_boards_and_unimplemented_precisions_are_rejected_by_name() {
    let text = ("AA, QQ, JTs", "KK, 99, 76s");
    let short = PostflopGame::new(
        &cards("9c 5d 2h"),
        ranges(text.0, text.1),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap_err()
    .to_string();
    assert!(short.contains("4-card board"), "{short}");

    let mut precision = options(LIMIT);
    precision.precision = Precision::F32;
    let error = PostflopGame::new(
        &cards("9c 5d 2h Ks"),
        ranges(text.0, text.1),
        all_in_turn(20),
        precision,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("f32"), "{error}");

    let error = PostflopGame::new(
        &cards("9c 5d 2h Ks"),
        ranges(text.0, text.1),
        all_in_turn(20),
        options(0),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("memory limit"), "{error}");

    let error = PostflopGame::new(
        &cards("9c 9c 2h Ks"),
        ranges(text.0, text.1),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("invalid game"), "{error}");
}

#[test]
fn a_small_turn_solve_reaches_a_measured_target_rather_than_the_cap() {
    let board = cards("9c 5d 2h Ks");
    let game = PostflopGame::new(
        &board,
        ranges(
            "22+, A2s-AKs, KTs-KQs, AJo-AKo",
            "33+, A5s-AKs, K9s-KQs, JTs, ATo-AKo",
        ),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap();
    let config = SolveConfig {
        target_pct_of_pot: 0.5,
        max_iterations: 400,
        check_every: 25,
        log_every_secs: 30,
        threads: 1,
    };
    let mut solver = PostflopSolver::new(
        game,
        Variant::Discounted {
            alpha: 1.5,
            beta: 0.0,
            gamma: 2.0,
        },
    )
    .unwrap();
    let mut measurements = 0;
    let report = solver
        .solve(&config, |progress| {
            assert!(progress.exploitability.pct_of_pot.is_finite());
            measurements += 1;
        })
        .unwrap();
    println!(
        "turn solve: iterations {}, stop {:?}, exploitability {:.6}% of pot,          nash_conv {:.6} chips",
        report.iterations,
        report.stop_reason,
        report.exploitability.pct_of_pot,
        report.exploitability.nash_conv
    );
    assert_eq!(report.stop_reason, StopReason::TargetReached);
    assert!(report.iterations <= config.max_iterations);
    assert!(report.exploitability.pct_of_pot <= config.target_pct_of_pot);
    assert!(report.exploitability.nash_conv >= 0.0);
    assert!(measurements > 0);

    // The measurement is a real best-response walk over every runout, not a
    // number carried over from the last check.
    let average = solver.average_strategy().unwrap();
    let measured = average.exploitability().unwrap();
    assert!((measured.pct_of_pot - report.exploitability.pct_of_pot).abs() < 1e-12);
    assert!((average.expected_value(0).unwrap() + average.expected_value(1).unwrap()).abs() < 1e-9);
}
