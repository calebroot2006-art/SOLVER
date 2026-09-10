//! Street-aware postflop games: river equivalence, the all-in runout against a
//! brute-force enumeration on the turn and on the flop, the chance contract, the
//! disjoint runout ranges a parallel walk needs, memory accounting, and a small
//! turn solve that reaches a measured target.

use cards::{Card, Combo, Range, evaluate_seven};
use postflop::{
    NodeId, Precision, RiverGame, RiverSolver, SolveConfig, SolveError, SolveReport, StopReason,
    Strategy, Variant,
    streets::{
        MemoryOverlap, MemoryReservation, PostflopGame, PostflopMemory, PostflopOptions,
        PostflopSolver, PostflopStrategy, StoragePlan, VERIFICATION_ALIASES, rows,
    },
    terminal::{OutcomeUtilities, ShowdownScratch, ShowdownTable},
};
use std::time::{Duration, Instant};
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

/// One bet menu per player, with no raises for either.
fn per_player(oop: &str, ip: &str) -> [BetSizeOptions; 2] {
    [
        BetSizeOptions::try_from((oop, "")).unwrap(),
        BetSizeOptions::try_from((ip, "")).unwrap(),
    ]
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

/// A flop tree whose only wager is the out-of-position flop jam, with no menu on
/// any later street. It stays small while still holding two chance levels: a
/// called flop all-in runs the turn and the river out with no decision between,
/// and checking through reaches the river one dealt card at a time.
fn all_in_flop(effective_stack: u64) -> PostflopTree {
    PostflopTree::new(PostflopTreeConfig {
        starting_pot: 10,
        effective_stack,
        min_bet: 1,
        start_street: Street::Flop,
        sizes: [per_player("a", ""), menus("", ""), menus("", "")],
        max_raises: 0,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })
    .unwrap()
}

/// Traversal workers a game resolves `threads: 0` to, read the same way the
/// crate reads it.
fn cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

/// Private states carrying weight after board removal, per player.
fn live_states(game: &PostflopGame) -> [usize; 2] {
    std::array::from_fn(|player| {
        game.initial_weights(player)
            .unwrap()
            .iter()
            .filter(|weight| **weight > 0.0)
            .count()
    })
}

/// Expanded terminals, counted from the public API.
fn terminals(game: &PostflopGame) -> usize {
    (0..game.num_nodes() as NodeId)
        .filter(|id| {
            matches!(
                game.node(*id).unwrap().kind(),
                PostflopNodeKind::Terminal(_)
            )
        })
        .count()
}

/// The disjoint-runout contract: one chance node's outcome ranges are
/// non-empty, in outcome order, and they partition that node's own subtree less
/// its root. Contiguity plus order is what makes them pairwise disjoint, which
/// is what a parallel walk splits its accumulators along.
fn assert_outcome_ranges(game: &PostflopGame, chance: NodeId, expected: usize) {
    let view = game.node(chance).unwrap();
    assert!(matches!(view.kind(), PostflopNodeKind::Chance { .. }));
    assert_eq!(view.children().len(), expected);
    let whole = game.subtree(chance).unwrap();
    assert_eq!(whole.start, chance);
    let mut previous = whole.start + 1;
    let mut covered = 0_usize;
    for (outcome, dealt) in view.children().iter().enumerate() {
        let range = game.outcome_range(chance, outcome).unwrap();
        assert!(range.start < range.end, "outcome {outcome} owns no nodes");
        assert_eq!(
            range.start, previous,
            "outcome {outcome} of node {chance} is not contiguous with the last"
        );
        assert!(
            range.end <= whole.end,
            "outcome {outcome} leaves the subtree"
        );
        assert_eq!(range.start, *dealt);
        previous = range.end;
        covered += (range.end - range.start) as usize;
    }
    assert_eq!(previous, whole.end);
    assert_eq!(covered + 1, (whole.end - whole.start) as usize);
    assert!(game.outcome_range(chance, expected).is_none());
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
        // A river-start tree deals nothing, so the whole tree is one subtree and
        // no node has an outcome range.
        assert_eq!(
            postflop_game.subtree(postflop_game.root()),
            Some(0..postflop_game.num_nodes() as NodeId)
        );
        for id in 0..postflop_game.num_nodes() as NodeId {
            let view = postflop_game.node(id).unwrap();
            assert!(!matches!(view.kind(), PostflopNodeKind::Chance { .. }));
            assert!(postflop_game.outcome_range(id, 0).is_none());
        }
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

    // Construction walked the whole tree: every node reachable once, one unit of
    // chance mass per live pair at all three deals, and every terminal checked
    // pairwise for zero sum.
    let validation = game.validation();
    assert_eq!(validation.nodes, game.num_nodes());
    // AA, QQ and JTs leave 6 + 6 + 4 = 16 live combos. KK and 99 leave three
    // each, because the board holds the king of spades and the nine of clubs,
    // and 76s leaves four: 10. Every one of the 16 * 10 = 160 pairs is walked.
    assert_eq!(live_states(&game), [16, 10]);
    assert_eq!(validation.pairs, 160);
    // Three river deals in the compact turn tree, after check-check, after a
    // called check-jam and after a called open jam, on one turn board.
    assert_eq!(validation.chance_nodes, 3);
    // Terminals: two folds, 48 showdowns under each of the two called jams, and
    // the five the river subtree holds on each of the 48 boards checking
    // through reaches. 2 + 2 * 48 + 48 * 5 = 338.
    assert_eq!(validation.terminals, terminals(&game));
    assert_eq!(validation.terminals, 338);
    assert_eq!(validation.zero_sum_terminals, 338);

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
fn a_called_flop_all_in_matches_a_brute_force_enumeration_over_both_deals() {
    let board = cards("9c 5d 2h");
    let stack = 20_u64;
    // Six combos each: small enough that the construction-time path validation
    // checks every pair at every deal and every terminal, and that the brute
    // force below enumerates all 49 x 48 ordered runouts per pair.
    let text = ("AA", "KK");
    let game = PostflopGame::new(
        &board,
        ranges(text.0, text.1),
        all_in_flop(stack),
        options(LIMIT),
    )
    .unwrap();
    let memory = game.memory_usage();
    println!(
        "flop-start estimate: {} boards, {} tables, {} nodes, construction {} B, bound {} B",
        memory.board_states,
        memory.showdown_tables,
        memory.expanded_nodes,
        memory.construction_bytes,
        memory.working_set_bound_bytes
    );
    assert_eq!(memory.board_states, 1 + 49 + 49 * 48);
    assert_eq!(memory.showdown_tables, 49 * 48);

    let live = live_states(&game);
    assert_eq!(live, [6, 6]);
    let validation = game.validation();
    assert_eq!(validation.nodes, game.num_nodes());
    assert_eq!(validation.pairs, 36);
    // The compact tree deals twice on the flop, once after check-check and once
    // after the called jam, and twice on the turn under those same two lines.
    // Each flop deal stays one node and each turn deal becomes one per dealt
    // turn card: 2 + 2 * 49 = 100.
    assert_eq!(validation.chance_nodes, 100, "{validation:?}");
    // Terminals: the one fold, one showdown per ordered runout under the called
    // jam, and one more per ordered runout after checking through.
    // 1 + 49 * 48 + 49 * 48 = 4705.
    assert_eq!(validation.terminals, terminals(&game));
    assert_eq!(validation.terminals, 1 + 2 * 49 * 48);
    assert_eq!(validation.zero_sum_terminals, 1 + 2 * 49 * 48);

    // The called flop all-in: two chance levels with no decision between them.
    let jam = child(&game, game.root(), Action::AllIn(stack));
    let called = child(&game, jam, Action::Call);
    let turn_deal = game.node(called).unwrap();
    assert!(matches!(
        turn_deal.kind(),
        PostflopNodeKind::Chance { next: Street::Turn }
    ));
    assert_eq!(turn_deal.street(), Street::Flop);
    assert_eq!(turn_deal.possible_cards().len(), 49);
    assert!((turn_deal.chance_probability().unwrap() - 1.0 / 45.0).abs() < 1e-15);
    assert_outcome_ranges(&game, called, 49);

    let mut showdowns = 0;
    for (index, river_deal) in turn_deal.children().iter().enumerate() {
        let view = game.node(*river_deal).unwrap();
        assert_eq!(view.street(), Street::Turn);
        assert_eq!(view.runout().len(), 1);
        assert_eq!(view.possible_cards().len(), 48);
        assert!((view.chance_probability().unwrap() - 1.0 / 44.0).abs() < 1e-15);
        // Nested deals keep the contract: each river subtree is disjoint inside
        // its own turn card's range, which is disjoint inside the flop deal's.
        assert_outcome_ranges(&game, *river_deal, 48);
        assert_eq!(
            game.subtree(*river_deal).unwrap(),
            game.outcome_range(called, index).unwrap()
        );
        for terminal in view.children() {
            let leaf = game.node(*terminal).unwrap();
            assert_eq!(leaf.board().len(), 5);
            assert_eq!(leaf.street(), Street::River);
            assert!(matches!(
                leaf.kind(),
                PostflopNodeKind::Terminal(tree::Terminal::Showdown)
            ));
            showdowns += 1;
        }
    }
    assert_eq!(showdowns, 49 * 48);

    // It solves: iterations over two chance levels, a measured exploitability,
    // and expected values that still sum to zero.
    let mut solver = PostflopSolver::new(
        game.clone(),
        Variant::Discounted {
            alpha: 1.5,
            beta: 0.0,
            gamma: 2.0,
        },
    )
    .unwrap();
    for _ in 0..2 {
        solver.run_iteration().unwrap();
    }
    let average = solver.average_strategy().unwrap();
    let measured = average.exploitability().unwrap();
    println!(
        "flop-start solve: 2 iterations, exploitability {:.6}% of pot, nash_conv {:.6} chips",
        measured.pct_of_pot, measured.nash_conv
    );
    assert!(measured.pct_of_pot.is_finite() && measured.pct_of_pot >= 0.0);
    assert!((average.expected_value(0).unwrap() + average.expected_value(1).unwrap()).abs() < 1e-9);
    drop(average);
    drop(solver);

    // The value of calling the jam, against a seven-card enumeration of every
    // ordered turn and river the pair leaves live.
    let strategy = PostflopStrategy::uniform(&game).unwrap();
    let report = strategy.decision_values(jam).unwrap();
    assert_eq!(report.player(), 1);
    assert_eq!(report.street(), Street::Flop);
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

    let opponent: Vec<f64> = game
        .initial_weights(0)
        .unwrap()
        .iter()
        .map(|weight| weight * 0.5)
        .collect();
    let amount = 10.0 / 2.0 + stack as f64;
    let deck: Vec<Card> = Card::all()
        .filter(|card| board.iter().all(|dealt| card.mask() & dealt.mask() == 0))
        .collect();
    assert_eq!(deck.len(), 49);
    let probability = (1.0 / 45.0) * (1.0 / 44.0);

    let candidates: Vec<Combo> = Combo::all()
        .filter(|combo| board.iter().all(|card| combo.mask() & card.mask() == 0))
        .collect();
    let mut brute = vec![0.0_f64; 1326];
    let mut mass = vec![0.0_f64; 1326];
    // The compatible opponent mass is cheap for every combo the board leaves.
    for hero in &candidates {
        let hero_id = usize::from(hero.id());
        for villain in &candidates {
            let villain_id = usize::from(villain.id());
            if opponent[villain_id] == 0.0 || hero.mask() & villain.mask() != 0 {
                continue;
            }
            mass[hero_id] += opponent[villain_id];
        }
    }
    // The runout enumeration is quadratic in the deck, so it runs only for the
    // hands the caller can actually hold: the in-position range facing the jam.
    let holdings = game.initial_weights(1).unwrap();
    let mut enumerated = 0_usize;
    for hero in candidates
        .iter()
        .filter(|combo| holdings[usize::from(combo.id())] > 0.0)
    {
        let hero_id = usize::from(hero.id());
        for villain in &candidates {
            let villain_id = usize::from(villain.id());
            if opponent[villain_id] == 0.0 || hero.mask() & villain.mask() != 0 {
                continue;
            }
            let held = hero.mask() | villain.mask();
            for turn in &deck {
                if held & turn.mask() != 0 {
                    continue;
                }
                for river in &deck {
                    if held & river.mask() != 0 || river.id() == turn.id() {
                        continue;
                    }
                    let five = [board[0], board[1], board[2], *turn, *river];
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
                    enumerated += 1;
                }
            }
        }
    }
    // Six holdings, six opposing hands, and both orders of every runout the
    // four cards leave: 36 * 45 * 44.
    assert_eq!(enumerated, 36 * 45 * 44);

    let mut compared = 0;
    for hero in &candidates {
        let id = usize::from(hero.id());
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
    assert_eq!(compared, 6, "only {compared} combos were compared");
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
    // The turn deal's forty-eight runouts own disjoint contiguous ranges, and a
    // decision node has none.
    assert_outcome_ranges(&game, deal, 48);
    assert!(game.outcome_range(checked, 0).is_none());
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

/// Both players hold every combo the board leaves, which puts the tree far
/// above the pair budget. The two quadratic checks are the only ones that stop:
/// the structural walk still covers every node, and the report says so.
#[test]
fn a_tree_above_the_pair_budget_still_gets_the_whole_structural_walk() {
    let board = cards("9c 5d 2h Ks");
    let full = || Range::from_weights([1.0; 1326]).unwrap();
    let game =
        PostflopGame::new(&board, [full(), full()], all_in_turn(20), options(LIMIT)).unwrap();

    // The four board cards leave 48, so each player holds C(48,2) = 1128 combos
    // and the walk would face 1128 * 1128 = 1,272,384 pairs, well above the
    // 512 * 512 budget the crate allows a construction-time pair check.
    let live = live_states(&game);
    assert_eq!(live, [1128, 1128]);
    assert!(live[0] * live[1] > 512 * 512, "{live:?}");

    // The linear half ran, over exactly the tree the smaller fixture above
    // reports: the same 3 chance nodes and 338 terminals, since only the ranges
    // differ. Reachability, child counts, probabilities and mask shapes were
    // all checked on every one of those nodes.
    let validation = game.validation();
    assert_eq!(validation.nodes, game.num_nodes());
    assert_eq!(validation.chance_nodes, 3);
    assert_eq!(validation.terminals, terminals(&game));
    assert_eq!(validation.terminals, 338);

    // The quadratic half did not run, and reports nothing rather than a pair
    // count it never walked.
    assert_eq!(validation.pairs, 0);
    assert_eq!(validation.zero_sum_terminals, 0);
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
        "turn estimate: {} boards, {} tables, {} nodes, shared {} B, solver {} B,          snapshot {} B, traversal {} B, scratch {} B, decision {} B,          node {} B, construction {} B, bound {} B",
        memory.board_states,
        memory.showdown_tables,
        memory.expanded_nodes,
        memory.shared_bytes,
        memory.solver_bytes,
        memory.snapshot_bytes,
        memory.traversal_bytes,
        memory.scratch_bytes,
        memory.decision_bytes,
        memory.node_bytes,
        memory.construction_bytes,
        memory.working_set_bound_bytes
    );
    assert_eq!(memory.board_states, 49);
    assert_eq!(memory.showdown_tables, 48);
    assert_eq!(memory.expanded_nodes, game.num_nodes());
    assert_eq!(game.reserved_bytes(), memory.shared_bytes);

    // The bound is exactly its components: one solver, two retained averages,
    // one traversal buffer set and one scratch per worker for an iteration, one
    // more of each for a strategy query that overlaps it, one decision report,
    // one node report, and the transients construction itself held.
    assert_eq!(
        memory.working_set_bound_bytes,
        memory.shared_bytes
            + memory.solver_bytes
            + 2 * memory.snapshot_bytes
            + 2 * memory.scratch_bytes
            + 2 * memory.traversal_bytes
            + memory.decision_bytes
            + memory.node_bytes
            + memory.construction_bytes
    );

    let mut solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
    // One iteration so the retained average below is a real policy, and so the
    // traversal reservation it takes has been made and released.
    solver.run_iteration().unwrap();
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
        // Two retained averages, one report of each kind, and the workspaces an
        // iteration and this query hold at the same time, all inside the bound.
        let second = solver.average_strategy().unwrap();
        let values = snapshot.decision_values(game.root()).unwrap();
        assert_eq!(values.action_count(), 2);
        let nodes = snapshot.node_values(game.root()).unwrap();
        let held = game.reserved_bytes();
        assert_eq!(
            held,
            memory.shared_bytes
                + memory.solver_bytes
                + memory.scratch_bytes
                + 2 * memory.snapshot_bytes
                + memory.decision_bytes
                + memory.node_bytes
        );
        assert_eq!(
            held + memory.traversal_bytes * 2 + memory.scratch_bytes + memory.construction_bytes,
            memory.working_set_bound_bytes
        );
        drop((snapshot, second, values, nodes));
    }
    assert_eq!(
        game.reserved_bytes(),
        memory.shared_bytes + memory.solver_bytes + memory.scratch_bytes
    );
    drop(solver);
    assert_eq!(game.reserved_bytes(), memory.shared_bytes);

    // Zero threads is resolved through the platform's core count in one place,
    // so the solver allocates exactly the scratches the estimate charged for.
    let mut per_core = options(LIMIT);
    per_core.threads = 0;
    let wide =
        PostflopGame::new(&board, ranges(text.0, text.1), all_in_turn(20), per_core).unwrap();
    let charged = wide.memory_usage();
    let workers = cores();
    assert_eq!(
        charged.working_set_bound_bytes,
        charged.shared_bytes
            + charged.solver_bytes
            + 2 * charged.snapshot_bytes
            + (workers + 1) * charged.scratch_bytes
            + (workers + 1) * charged.traversal_bytes
            + charged.decision_bytes
            + charged.node_bytes
            + charged.construction_bytes
    );
    let per_core_solver = PostflopSolver::new(wide.clone(), Variant::Plus).unwrap();
    assert_eq!(
        wide.reserved_bytes(),
        charged.shared_bytes + charged.solver_bytes + workers * charged.scratch_bytes
    );
    drop(per_core_solver);

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
    // Decision 3's turn target, not the roadmap's looser flop gate: this fixture
    // is small enough to reach 0.25% of pot inside the cap.
    let config = SolveConfig {
        target_pct_of_pot: 0.25,
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
            assert!(
                progress
                    .exploitability
                    .is_some_and(|m| m.pct_of_pot.is_finite())
            );
            measurements += 1;
        })
        .unwrap();
    println!(
        "turn solve: iterations {}, stop {:?}, exploitability {:.6}% of pot,          nash_conv {:.6} chips",
        report.iterations,
        report.stop_reason,
        report.measured().unwrap().pct_of_pot,
        report.measured().unwrap().nash_conv
    );
    assert_eq!(report.stop_reason, StopReason::TargetReached);
    assert!(report.iterations <= config.max_iterations);
    assert!(report.measured().unwrap().pct_of_pot <= config.target_pct_of_pot);
    assert!(report.measured().unwrap().nash_conv >= 0.0);
    assert!(measurements > 0);

    // The measurement is a real best-response walk over every runout, not a
    // number carried over from the last check.
    let average = solver.average_strategy().unwrap();
    let measured = average.exploitability().unwrap();
    assert!((measured.pct_of_pot - report.measured().unwrap().pct_of_pot).abs() < 1e-12);
    assert!((average.expected_value(0).unwrap() + average.expected_value(1).unwrap()).abs() < 1e-9);
}

/// Whether some chance node has another chance node inside its own subtree,
/// which is what makes the accumulator split nest rather than partition once.
fn nests_a_chance_node(game: &PostflopGame) -> bool {
    (0..game.num_nodes() as NodeId)
        .filter(|id| {
            matches!(
                game.node(*id).unwrap().kind(),
                PostflopNodeKind::Chance { .. }
            )
        })
        .any(|chance| {
            let range = game.subtree(chance).unwrap();
            (range.start + 1..range.end).any(|inside| {
                matches!(
                    game.node(inside).unwrap().kind(),
                    PostflopNodeKind::Chance { .. }
                )
            })
        })
}

/// Every strategy row's exact bits, folded to one value. Equal hashes over two
/// solves mean equal policies to the last bit, which is what "identical for any
/// thread count" has to mean for a solver whose sums are not associative.
fn policy_hash(strategy: &PostflopStrategy) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for row in strategy.rows() {
        for value in row {
            for byte in value.to_bits().to_be_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    hash
}

/// The same solve at a chosen worker count, with its wall time.
fn solve_turn_fixture(threads: usize, iterations: u64) -> (SolveReport, u64, usize, Duration) {
    let board = cards("9c 5d 2h Ks");
    let mut chosen = options(LIMIT);
    chosen.threads = threads;
    let game = PostflopGame::new(
        &board,
        ranges(
            "22+, A2s-AKs, KTs-KQs, AJo-AKo",
            "33+, A5s-AKs, K9s-KQs, JTs, ATo-AKo",
        ),
        all_in_turn(20),
        chosen,
    )
    .unwrap();
    let config = SolveConfig {
        target_pct_of_pot: 0.25,
        max_iterations: iterations,
        check_every: 25,
        log_every_secs: 30,
        threads,
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
    let workers = solver.workers();
    let started = Instant::now();
    let report = solver.solve(&config, |_| {}).unwrap();
    let elapsed = started.elapsed();
    let hash = policy_hash(&solver.average_strategy().unwrap());
    (report, hash, workers, elapsed)
}

#[test]
fn the_turn_fixture_solves_to_the_same_bits_on_one_two_and_four_workers() {
    let (serial, expected, workers, serial_time) = solve_turn_fixture(1, 400);
    assert_eq!(workers, 1, "one thread must build no pool at all");
    assert_eq!(serial.stop_reason, StopReason::TargetReached);
    let mut four_time = serial_time;
    for threads in [2, 4] {
        // Oversubscribing a two-CPU runner is deliberate: determinism is a
        // property of the reduction order, not of the number of cores.
        let (report, hash, pool, elapsed) = solve_turn_fixture(threads, 400);
        assert_eq!(pool, threads, "the pool must hold the resolved workers");
        assert_eq!(hash, expected, "{threads} workers changed the policy bits");
        assert_eq!(report.iterations, serial.iterations);
        assert_eq!(report.stop_reason, serial.stop_reason);
        assert_eq!(
            report.measured().unwrap().nash_conv.to_bits(),
            serial.measured().unwrap().nash_conv.to_bits(),
            "{threads} workers changed the measured NashConv"
        );
        assert_eq!(
            report.measured().unwrap().pct_of_pot.to_bits(),
            serial.measured().unwrap().pct_of_pot.to_bits()
        );
        if threads == 4 {
            four_time = elapsed;
        }
    }
    // Informational, not a gate: a CI runner has two CPUs and shares them.
    println!(
        "turn fixture {} iterations: 1 worker {:.2}s, 4 workers {:.2}s, speedup {:.2}x",
        serial.iterations,
        serial_time.as_secs_f64(),
        four_time.as_secs_f64(),
        serial_time.as_secs_f64() / four_time.as_secs_f64()
    );
}

#[test]
fn a_flop_start_solve_nests_the_split_without_changing_a_bit() {
    let board = cards("9c 5d 2h");
    let text = ("AA, QQ, JTs", "KK, 99, 76s");
    let mut hashes = Vec::new();
    for threads in [1, 4] {
        let mut chosen = options(LIMIT);
        chosen.threads = threads;
        let game =
            PostflopGame::new(&board, ranges(text.0, text.1), all_in_flop(20), chosen).unwrap();
        // The turn deal splits, and inside each turn runout the river deal
        // splits again over that runout's own range, so this fixture is the one
        // that exercises a nested split rather than a single flat one.
        assert!(
            nests_a_chance_node(&game),
            "a flop-start tree must deal inside a deal"
        );
        let mut solver = PostflopSolver::new(game, Variant::Plus).unwrap();
        assert_eq!(solver.workers(), threads);
        for _ in 0..2 {
            solver.run_iteration().unwrap();
        }
        let average = solver.average_strategy().unwrap();
        hashes.push((policy_hash(&average), average.exploitability().unwrap()));
    }
    assert_eq!(hashes[0].0, hashes[1].0, "four workers changed the policy");
    assert_eq!(
        hashes[0].1.nash_conv.to_bits(),
        hashes[1].1.nash_conv.to_bits(),
        "four workers changed the measured NashConv"
    );
    println!(
        "flop-start solve: policy hash {:#018x}, exploitability {:.6}% of pot",
        hashes[0].0, hashes[0].1.pct_of_pot
    );
}

#[test]
fn zero_threads_gives_the_pool_the_worker_count_the_estimate_charged() {
    let board = cards("9c 5d 2h Ks");
    let text = ("AA, QQ, JTs", "KK, 99, 76s");
    let mut per_core = options(LIMIT);
    per_core.threads = 0;
    let game =
        PostflopGame::new(&board, ranges(text.0, text.1), all_in_turn(20), per_core).unwrap();
    let charged = game.memory_usage();
    let workers = cores();
    // One `resolve_workers` answer: the pool the solver starts and the worker
    // term the estimate charged cannot drift apart.
    assert_eq!(
        charged.working_set_bound_bytes,
        charged.shared_bytes
            + charged.solver_bytes
            + 2 * charged.snapshot_bytes
            + (workers + 1) * charged.scratch_bytes
            + (workers + 1) * charged.traversal_bytes
            + charged.decision_bytes
            + charged.node_bytes
            + charged.construction_bytes
    );
    let solver = PostflopSolver::new(game, Variant::Plus).unwrap();
    assert_eq!(solver.workers(), workers);
    assert!(format!("{solver:?}").contains(&format!("workers: {workers}")));
}

/// Step 5c: the row breakdown restates the estimate one buffer at a time, and
/// the two fixture sums the plan records are reproduced from it.
#[test]
fn the_memory_rows_sum_to_the_estimate_on_both_fixtures() {
    let turn = PostflopGame::new(
        &cards("9c 5d 2h Ks"),
        ranges("AA, QQ, JTs", "KK, 99, 76s"),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap();
    let flop = PostflopGame::new(
        &cards("9c 5d 2h"),
        ranges("AA", "KK"),
        all_in_flop(20),
        options(LIMIT),
    )
    .unwrap();

    // The sums recorded in docs/phase-4/PLAN.md for these two fixtures, at one
    // worker. They are pinned here so a change to any charged term is a test
    // failure rather than a number that quietly moves in a table.
    assert_eq!(turn.memory_usage().working_set_bound_bytes, 31_876_137);
    assert_eq!(turn.memory_usage().construction_bytes, 4_205_121);
    assert_eq!(flop.memory_usage().working_set_bound_bytes, 422_791_850);

    for game in [&turn, &flop] {
        let memory = game.memory_usage();
        let table = memory.rows().unwrap();
        let counted: usize = table
            .iter()
            .filter(|row| row.overlap.is_counted())
            .map(|row| row.bytes)
            .sum();
        assert_eq!(counted, memory.working_set_bound_bytes);
        assert_eq!(
            memory.bound_under(&StoragePlan::today()).unwrap(),
            memory.working_set_bound_bytes
        );

        // One row per name, so a reservation that names a row reaches exactly
        // one of them.
        let mut names: Vec<&str> = table.iter().map(|row| row.name).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "a row name appears twice");

        // The one uncounted row is the verification walk, and it is the
        // snapshot and the iteration workspaces seen again, not new bytes.
        let uncounted: Vec<_> = table
            .iter()
            .filter(|row| !row.overlap.is_counted())
            .collect();
        assert_eq!(uncounted.len(), 1);
        assert_eq!(uncounted[0].name, rows::VERIFICATION);
        assert_eq!(
            uncounted[0].overlap,
            MemoryOverlap::Aliases(VERIFICATION_ALIASES)
        );
        // The rows it borrows, named by the same constants the table prints.
        assert_eq!(
            VERIFICATION_ALIASES,
            [
                rows::SNAPSHOTS,
                rows::TRAVERSAL,
                rows::SCRATCH,
                rows::QUERY_WORKSPACE
            ]
        );
        assert_eq!(
            uncounted[0].bytes,
            memory.snapshot_bytes
                + memory.workers * (memory.traversal_bytes + memory.scratch_bytes)
        );

        // The entry counts each row reports are the bytes it charges: one
        // stored array is its entries at the plan's width plus one Vec header
        // per node, and a snapshot adds its own header per retained copy.
        let headers = memory.expanded_nodes * std::mem::size_of::<Vec<f64>>();
        for row in &table {
            if row.arrays == 0 {
                assert_eq!(
                    row.entries, 0,
                    "{} reports entries without an array",
                    row.name
                );
                continue;
            }
            let overhead = if row.name == rows::SNAPSHOTS {
                headers + std::mem::size_of::<Strategy>() + 256
            } else {
                headers
            };
            assert_eq!(
                row.bytes,
                row.entries * 8 + row.arrays * overhead,
                "{} does not charge its entries",
                row.name
            );
        }

        // f32 and i16 are the same rows with narrower entries, plus, for i16,
        // one f32 scale per decision node per stored array: three solver arrays
        // and two snapshots today.
        let entries = memory.entries_under(&StoragePlan::today()).unwrap();
        for (precision, width) in [(Precision::F32, 4_usize), (Precision::I16, 2)] {
            let narrow = memory
                .bound_under(&StoragePlan::today().at(precision))
                .unwrap();
            let scales = if precision == Precision::I16 {
                4 * memory.expanded_decision_nodes * 5
            } else {
                0
            };
            assert_eq!(
                narrow,
                memory.working_set_bound_bytes - 5 * entries * (8 - width) + scales,
                "{precision:?} rows do not narrow by the entry width alone"
            );
        }

        // Step 6's target layout: the current policy derived rather than
        // stored, no snapshot retained during the solve, and the private states
        // compacted to the live combos of this game.
        let live = live_states(game);
        let target = StoragePlan {
            precision: Precision::F64,
            states: live,
            snapshots: 0,
            store_current_policy: false,
        };
        let compacted = memory.entries_under(&target).unwrap();
        assert_eq!(
            compacted,
            memory.action_slots[0] * live[0] + memory.action_slots[1] * live[1]
        );
        assert!(memory.bound_under(&target).unwrap() < memory.working_set_bound_bytes);
    }
}

/// Step 5c: every `Budget` reservation this crate makes is a named set of rows,
/// and the reservations plus the construction transients are the whole bound.
#[test]
fn every_budget_reservation_names_the_rows_it_draws_from() {
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
    let table = memory.rows().unwrap();
    let bytes_of = |name: &str| {
        table
            .iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("no row named {name}"))
            .bytes
    };

    // Each reservation is exactly the rows it names.
    let mut claimed: Vec<&str> = Vec::new();
    for reservation in MemoryReservation::ALL {
        let named: usize = reservation.row_names().iter().copied().map(bytes_of).sum();
        assert_eq!(
            reservation.bytes(&memory) * reservation.charged(),
            named,
            "{reservation:?} does not equal the rows it names"
        );
        claimed.extend(reservation.row_names());
    }

    // Every row is claimed by one reservation, except the two that no lease
    // covers: the construction transients are freed before a solver exists, and
    // the verification walk borrows rows another reservation already holds.
    for row in &table {
        let count = claimed.iter().filter(|name| **name == row.name).count();
        let expected =
            usize::from(row.name != rows::CONSTRUCTION && row.name != rows::VERIFICATION);
        assert_eq!(count, expected, "{} is claimed {count} times", row.name);
    }

    // The reservations, at the multiplicity the bound charges, are the bound.
    let charged: usize = MemoryReservation::ALL
        .iter()
        .map(|reservation| reservation.bytes(&memory) * reservation.charged())
        .sum();
    assert_eq!(
        charged + memory.construction_bytes,
        memory.working_set_bound_bytes
    );
    assert_eq!(MemoryReservation::Snapshot.charged(), 2);

    // And the reservations the budget actually takes are those numbers.
    assert_eq!(
        game.reserved_bytes(),
        MemoryReservation::Shared.bytes(&memory)
    );
    let mut solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
    solver.run_iteration().unwrap();
    assert_eq!(
        game.reserved_bytes(),
        MemoryReservation::Shared.bytes(&memory) + MemoryReservation::Solver.bytes(&memory)
    );
    let first = solver.average_strategy().unwrap();
    let second = solver.average_strategy().unwrap();
    let values = first.decision_values(game.root()).unwrap();
    let nodes = first.node_values(game.root()).unwrap();
    assert_eq!(
        game.reserved_bytes(),
        MemoryReservation::Shared.bytes(&memory)
            + MemoryReservation::Solver.bytes(&memory)
            + 2 * MemoryReservation::Snapshot.bytes(&memory)
            + MemoryReservation::DecisionReport.bytes(&memory)
            + MemoryReservation::NodeReport.bytes(&memory)
    );
    // The two leases nothing outside a walk can observe, plus what is held
    // above and the transients construction freed, are the whole bound.
    assert_eq!(
        game.reserved_bytes()
            + MemoryReservation::Iteration.bytes(&memory)
            + MemoryReservation::Query.bytes(&memory)
            + memory.construction_bytes,
        memory.working_set_bound_bytes
    );
    drop((first, second, values, nodes));

    // An imported average is a retained average: whatever capacity its rows
    // arrive with, it draws at least the snapshot row the table charges, so
    // MemoryReservation::Snapshot covers this site too.
    let uniform = PostflopStrategy::uniform(&game).unwrap();
    let before = game.reserved_bytes();
    let imported = PostflopStrategy::from_rows(&game, uniform.rows().to_vec()).unwrap();
    let charged = game.reserved_bytes() - before;
    assert!(
        charged >= MemoryReservation::Snapshot.bytes(&memory),
        "an import charged {charged}, under one snapshot"
    );
    drop((uniform, imported));
    assert_eq!(
        game.reserved_bytes(),
        MemoryReservation::Shared.bytes(&memory) + MemoryReservation::Solver.bytes(&memory)
    );
}

/// Step 5c: the table's entry point prices a tree no game can be built from,
/// which is the case the flop gate is in, and refuses a mismatched board.
#[test]
fn a_tree_too_large_to_build_can_still_be_priced() {
    let board = cards("9c 5d 2h Ks");
    let text = ("AA, QQ, JTs", "KK, 99, 76s");
    let game = PostflopGame::new(
        &board,
        ranges(text.0, text.1),
        all_in_turn(20),
        options(LIMIT),
    )
    .unwrap();
    assert_eq!(
        PostflopMemory::for_tree(&all_in_turn(20), 4, 1).unwrap(),
        game.memory_usage()
    );

    let error = PostflopMemory::for_tree(&all_in_turn(20), 3, 1)
        .unwrap_err()
        .to_string();
    assert!(error.contains("4-card board"), "{error}");
    let error = PostflopMemory::for_tree(&all_in_turn(20), 4, 0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("traversal worker"), "{error}");

    // The phase 4 gate flop tree, which PostflopGame::new refuses under the
    // 12 GiB default, is priced from the same arithmetic.
    let gate = PostflopTree::new(PostflopTreeConfig {
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
    let memory = PostflopMemory::for_tree(&gate, 3, 1).unwrap();
    assert_eq!(memory.expanded_nodes, 1_792_006);
    assert_eq!(memory.working_set_bound_bytes, 89_793_166_538);
    assert_eq!(
        memory.bound_under(&StoragePlan::today()).unwrap(),
        89_793_166_538
    );

    // The same tree on the turn is the turn gate: 9,003 expanded nodes and 11.4
    // million entries per stored array.
    let mut turn_config = gate.config().clone();
    turn_config.start_street = Street::Turn;
    let turn = PostflopMemory::for_tree(&PostflopTree::new(turn_config).unwrap(), 4, 1).unwrap();
    assert_eq!(turn.expanded_nodes, 9_003);
    assert_eq!(turn.working_set_bound_bytes, 471_031_163);
    assert_eq!(
        turn.entries_under(&StoragePlan::today()).unwrap(),
        11_363_820
    );
}
