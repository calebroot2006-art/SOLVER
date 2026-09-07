//! River equivalence, chance-node shape, and independent whole-tree audits of
//! the street-aware postflop tree.

use tree::{
    Action, BetSize, BetSizeOptions, NodeId, PostflopNodeKind, PostflopTree, PostflopTreeConfig,
    RiverNodeKind, RiverTree, RiverTreeConfig, Street, Terminal,
};

const STREETS: [Street; 3] = [Street::Flop, Street::Turn, Street::River];

fn menus(bets: &str, raises: &str) -> [BetSizeOptions; 2] {
    let sizes = BetSizeOptions::try_from((bets, raises)).unwrap();
    [sizes.clone(), sizes]
}

/// One menu on every street, so a river-start tree and a flop-start tree differ
/// only in where they begin.
fn uniform(bets: &str, raises: &str) -> [[BetSizeOptions; 2]; 3] {
    [
        menus(bets, raises),
        menus(bets, raises),
        menus(bets, raises),
    ]
}

fn config(start: Street, bets: &str, raises: &str) -> PostflopTreeConfig {
    PostflopTreeConfig {
        starting_pot: 100,
        effective_stack: 1000,
        min_bet: 10,
        start_street: start,
        sizes: uniform(bets, raises),
        max_raises: 2,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    }
}

/// The three phase 3 reference fixtures, from `tests/reference/river/cases.json`
/// (20bb dry, 100bb paired, 200bb flush). Only the stack differs between them.
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

fn as_postflop(river: &RiverTreeConfig) -> PostflopTreeConfig {
    PostflopTreeConfig {
        starting_pot: river.starting_pot,
        effective_stack: river.effective_stack,
        min_bet: river.min_bet,
        start_street: Street::River,
        // Flop and turn menus are never read from a river-start tree; making
        // them absurd proves it.
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

fn assert_matches_river(river: &RiverTree, postflop: &PostflopTree) {
    assert_eq!(postflop.nodes().len(), river.nodes().len());
    assert_eq!(postflop.max_depth(), river.max_depth());
    for (index, (left, right)) in postflop.nodes().iter().zip(river.nodes()).enumerate() {
        let expected = match right.kind() {
            RiverNodeKind::Decision { player } => PostflopNodeKind::Decision { player },
            RiverNodeKind::Terminal(terminal) => PostflopNodeKind::Terminal(terminal),
        };
        assert_eq!(left.kind(), expected, "node {index}");
        assert_eq!(left.street(), Street::River, "node {index}");
        assert_eq!(left.contributions(), right.contributions(), "node {index}");
        assert_eq!(left.actions(), right.actions(), "node {index}");
        assert_eq!(left.children(), right.children(), "node {index}");
    }
}

#[test]
fn a_river_start_tree_is_the_river_tree_node_for_node() {
    for stack in [20, 100, 200] {
        let river = river_fixture(stack);
        let built = RiverTree::new(river.clone()).unwrap();
        let postflop = PostflopTree::new(as_postflop(&river)).unwrap();
        assert_matches_river(&built, &postflop);
        // The counters describe one river block and nothing earlier.
        assert_eq!(postflop.decision_nodes_per_street()[0..2], [0, 0]);
        assert_eq!(postflop.live_continuations_per_street()[0..2], [0, 0]);
        audit(&postflop);
    }
    // Every shape the river suite exercises, replayed through the new builder.
    for (bets, raises) in [
        ("50%,20c", "50%,2.5x,20c"),
        ("a,50%,50c,20c,e", "2x,100%,a"),
        ("25%,1c,a", "50%,2x,1c,a"),
        ("", "a"),
        ("1%", ""),
    ] {
        for stack in [0, 7, 25, 1000] {
            for (add, force) in [(0.0, 0.0), (0.3, 0.2), (10.0, 0.5)] {
                let mut river = RiverTreeConfig {
                    starting_pot: 100,
                    effective_stack: stack,
                    min_bet: 10,
                    sizes: menus(bets, raises),
                    max_raises: 2,
                    add_all_in_threshold: add,
                    force_all_in_threshold: force,
                    max_nodes: 100_000,
                };
                river.sizes[1] = BetSizeOptions::try_from(("20c", "a")).unwrap();
                let built = RiverTree::new(river.clone()).unwrap();
                let postflop = PostflopTree::new(as_postflop(&river)).unwrap();
                assert_matches_river(&built, &postflop);
            }
        }
    }
}

#[test]
fn every_chance_node_deals_one_child_block_and_no_actions() {
    for start in [Street::Flop, Street::Turn] {
        let tree = PostflopTree::new(config(start, "50%,a", "100%,a")).unwrap();
        let mut chance = 0;
        for node in tree.nodes() {
            let PostflopNodeKind::Chance { next } = node.kind() else {
                continue;
            };
            chance += 1;
            assert!(node.actions().is_empty());
            assert_eq!(node.children().len(), 1);
            assert_eq!(node.street().next(), Some(next));
            assert!(node.street() >= start);
            let child = tree.node(node.children()[0]).unwrap();
            assert_eq!(child.street(), next);
            assert_eq!(child.contributions(), node.contributions());
            // A street begins with player zero facing no wager, unless nobody
            // has chips behind and the board simply runs out.
            match child.kind() {
                PostflopNodeKind::Decision { player } => assert_eq!(player, 0),
                other => assert_eq!(
                    node.contributions()[0],
                    tree.config().effective_stack,
                    "unexpected {other:?} with chips behind"
                ),
            }
        }
        assert!(chance > 0);
    }
}

#[test]
fn a_called_all_in_deals_the_board_out_without_decisions() {
    let tree = PostflopTree::new(config(Street::Flop, "a", "a")).unwrap();
    let stack = tree.config().effective_stack;
    // check, all-in, call: the flop chance node, the turn chance node, and the
    // showdown, with no decision after the call.
    let flop = at(
        &tree,
        &[
            Step::Act(Action::Check),
            Step::Act(Action::AllIn(stack)),
            Step::Act(Action::Call),
        ],
    );
    assert_eq!(
        tree.node(flop).unwrap().kind(),
        PostflopNodeKind::Chance { next: Street::Turn }
    );
    let turn = tree.node(flop).unwrap().children()[0];
    assert_eq!(
        tree.node(turn).unwrap().kind(),
        PostflopNodeKind::Chance {
            next: Street::River
        }
    );
    let river = tree.node(turn).unwrap().children()[0];
    let showdown = tree.node(river).unwrap();
    assert_eq!(
        showdown.kind(),
        PostflopNodeKind::Terminal(Terminal::Showdown)
    );
    assert_eq!(showdown.contributions(), [stack; 2]);
    assert!(showdown.children().is_empty());
    // Only the check-check line on each street is a live continuation; the
    // three all-in calls run the board out instead.
    assert_eq!(tree.live_continuations_per_street(), [1, 1, 1]);
    assert_eq!(tree.decision_nodes_per_street(), [4, 4, 4]);
    assert_eq!(tree.nodes().len(), 33);
    audit(&tree);
}

#[test]
fn a_zero_stack_runs_the_board_out_from_any_street() {
    for (start, nodes) in [(Street::Flop, 3), (Street::Turn, 2), (Street::River, 1)] {
        let mut cfg = config(start, "a", "a");
        cfg.effective_stack = 0;
        let tree = PostflopTree::new(cfg).unwrap();
        assert_eq!(tree.nodes().len(), nodes);
        assert_eq!(tree.max_depth(), nodes - 1);
        assert_eq!(tree.decision_nodes_per_street(), [0; 3]);
        assert_eq!(tree.live_continuations_per_street(), [0; 3]);
        assert_eq!(
            tree.node(tree.root()).unwrap().street(),
            start,
            "the root belongs to the configured street"
        );
        assert_eq!(
            tree.nodes().last().unwrap().kind(),
            PostflopNodeKind::Terminal(Terminal::Showdown)
        );
        audit(&tree);
    }
}

#[test]
fn later_streets_size_from_the_level_carried_into_them() {
    let mut cfg = config(Street::Turn, "50%", "2.5x");
    cfg.starting_pot = 10;
    cfg.effective_stack = 1000;
    cfg.min_bet = 10;
    cfg.max_raises = 1;
    let tree = PostflopTree::new(cfg).unwrap();
    // Turn: pot 10, bet 5 is below the minimum, so it clamps to 10.
    let turn = at(&tree, &[]);
    assert_eq!(
        tree.node(turn).unwrap().actions(),
        &[Action::Check, Action::Bet(10)]
    );
    // Raise-to 2.5x scales the turn wager, not the whole commitment: 25.
    let raise = at(&tree, &[Step::Act(Action::Bet(10))]);
    assert_eq!(
        tree.node(raise).unwrap().actions(),
        &[Action::Fold, Action::Call, Action::Raise(25)]
    );
    // River after bet 10, call: both carried 10 in, so the pot is 30 and a
    // half-pot bet is 15 more, a total contribution of 25.
    let river = at(
        &tree,
        &[
            Step::Act(Action::Bet(10)),
            Step::Act(Action::Call),
            Step::Deal,
        ],
    );
    assert_eq!(tree.node(river).unwrap().street(), Street::River);
    assert_eq!(
        tree.node(river).unwrap().actions(),
        &[Action::Check, Action::Bet(25)]
    );
    // 2.5x that river wager of 15 is 38 after rounding: 10 carried plus 38.
    let reraise = at(
        &tree,
        &[
            Step::Act(Action::Bet(10)),
            Step::Act(Action::Call),
            Step::Deal,
            Step::Act(Action::Bet(25)),
        ],
    );
    assert_eq!(
        tree.node(reraise).unwrap().actions(),
        &[Action::Fold, Action::Call, Action::Raise(48)]
    );
    // The raise cap is per street. One raise exhausts it on the turn, yet the
    // river that follows still offers its own raise.
    let raised = [
        Step::Act(Action::Bet(10)),
        Step::Act(Action::Raise(25)),
        Step::Act(Action::Call),
        Step::Deal,
    ];
    let capped = at(&tree, &raised[..2]);
    assert_eq!(
        tree.node(capped).unwrap().actions(),
        &[Action::Fold, Action::Call],
        "the turn cap is spent"
    );
    let next_street = at(&tree, &raised);
    assert_eq!(
        tree.node(next_street).unwrap().actions(),
        &[Action::Check, Action::Bet(55)]
    );
    let mut bet_line = raised.to_vec();
    bet_line.push(Step::Act(Action::Bet(55)));
    let facing = at(&tree, &bet_line);
    assert_eq!(
        tree.node(facing).unwrap().actions(),
        &[Action::Fold, Action::Call, Action::Raise(100)]
    );
    audit(&tree);
}

#[test]
fn per_street_menus_are_read_from_the_street_being_played() {
    let mut cfg = config(Street::Flop, "50%", "");
    cfg.starting_pot = 100;
    cfg.effective_stack = 10_000;
    cfg.min_bet = 10;
    cfg.sizes[Street::Flop.index()] = menus("10c", "");
    cfg.sizes[Street::Turn.index()] = menus("20c", "");
    cfg.sizes[Street::River.index()] = menus("30c", "");
    let tree = PostflopTree::new(cfg).unwrap();
    let flop = tree.node(at(&tree, &[])).unwrap();
    assert_eq!(flop.actions(), &[Action::Check, Action::Bet(10)]);
    let checked_through = [
        Step::Act(Action::Check),
        Step::Act(Action::Check),
        Step::Deal,
    ];
    let turn = tree.node(at(&tree, &checked_through)).unwrap();
    assert_eq!(turn.actions(), &[Action::Check, Action::Bet(20)]);
    let to_river = [checked_through, checked_through].concat();
    let river = tree.node(at(&tree, &to_river)).unwrap();
    assert_eq!(river.actions(), &[Action::Check, Action::Bet(30)]);
    audit(&tree);
}

// The planning anchors in docs/phase-4/PLAN.md, measured on the built tree,
// under the raise rule of decision 10: one raise per street, sized at 100% of
// pot, and no explicit all-in token in a bet menu. A street with two bet sizes
// and a raise menu of 100% plus all-in has the plan's nine live continuations
// (check/check, four bet/call, four raise/call; the called all-in leaves no
// chips behind). Its fourteen decision nodes are below the plan's approximate
// eighteen: two unopened, four facing a bet, and eight facing a raise.
#[test]
fn the_planning_anchor_menu_has_fourteen_decisions_and_nine_continuations() {
    let mut cfg = config(Street::River, "33%,75%", "100%,a");
    cfg.starting_pot = 10;
    cfg.effective_stack = 100;
    cfg.min_bet = 1;
    cfg.max_raises = 1;
    let river = PostflopTree::new(cfg.clone()).unwrap();
    assert_eq!(river.decision_nodes_per_street(), [0, 0, 14]);
    assert_eq!(river.live_continuations_per_street(), [0, 0, 9]);
    assert_eq!(river.nodes().len(), 39);
    audit(&river);

    // One street above it: thirteen chance nodes, of which nine lead to river
    // blocks and four are called all-ins that run the board out instead.
    cfg.start_street = Street::Turn;
    let turn = PostflopTree::new(cfg.clone()).unwrap();
    assert_eq!(turn.decision_nodes_per_street(), [0, 14, 110]);
    assert_eq!(turn.live_continuations_per_street(), [0, 9, 65]);
    assert_eq!(turn.nodes().len(), 346);
    audit(&turn);

    cfg.start_street = Street::Flop;
    let flop = PostflopTree::new(cfg).unwrap();
    assert_eq!(flop.decision_nodes_per_street(), [14, 110, 598]);
    assert_eq!(flop.live_continuations_per_street(), [9, 65, 305]);
    assert_eq!(flop.nodes().len(), 1985);
    audit(&flop);
}

// The decided gate menu (docs/phase-4/PLAN.md, decisions 1 and 10): 33% pot
// plus all-in on the flop and the turn, two bet sizes and no all-in token on
// the river, one raise per street sized at 100% of pot with an all-in beside
// it. A 100bb single-raised pot at ten chips per big blind: the button opens
// to 25 and the big blind calls, leaving 55 in the pot and 975 behind.
#[test]
fn the_gate_menu_counts_its_nodes_per_street() {
    let mut cfg = config(Street::Flop, "33%,a", "100%,a");
    cfg.starting_pot = 55;
    cfg.effective_stack = 975;
    cfg.min_bet = 10;
    cfg.max_raises = 1;
    cfg.sizes[Street::River.index()] = menus("33%,75%", "100%,a");
    let tree = PostflopTree::new(cfg.clone()).unwrap();
    assert_eq!(tree.decision_nodes_per_street(), [10, 50, 270]);
    assert_eq!(tree.live_continuations_per_street(), [5, 25, 153]);
    assert_eq!(tree.nodes().len(), 925);
    assert_eq!(tree.max_depth(), 13);
    audit(&tree);

    cfg.start_street = Street::Turn;
    let turn = PostflopTree::new(cfg).unwrap();
    assert_eq!(turn.decision_nodes_per_street(), [0, 10, 66]);
    assert_eq!(turn.live_continuations_per_street(), [0, 5, 41]);
    assert_eq!(turn.nodes().len(), 214);
    audit(&turn);
}

#[test]
fn node_and_depth_budgets_are_enforced_without_a_partial_tree() {
    let mut cfg = config(Street::Flop, "50%,a", "100%,a");
    let count = PostflopTree::new(cfg.clone()).unwrap().nodes().len();
    cfg.max_nodes = count;
    assert_eq!(PostflopTree::new(cfg.clone()).unwrap().nodes().len(), count);
    cfg.max_nodes = count - 1;
    assert!(
        PostflopTree::new(cfg)
            .unwrap_err()
            .to_string()
            .contains("max_nodes")
    );

    // Thirty-two minimum raises on each of three streets is the deepest legal
    // history: 107 edges, inside the 128-edge limit.
    let mut cfg = config(Street::Flop, "1c", "1c");
    cfg.starting_pot = 1;
    cfg.min_bet = 1;
    cfg.effective_stack = 200;
    cfg.max_raises = 32;
    cfg.max_nodes = 1_000_000;
    let deep = PostflopTree::new(cfg).unwrap();
    assert_eq!(deep.max_depth(), 107);
    assert_eq!(deep.nodes().len(), 915_957);
    assert_eq!(deep.decision_nodes_per_street(), [68, 4_556, 305_252]);
    assert_eq!(deep.live_continuations_per_street(), [67, 4_489, 300_763]);
}

#[test]
fn invalid_configuration_and_nonfinite_intermediates_fail() {
    let baseline = config(Street::Flop, "a", "a");
    for field in 0..9 {
        let mut cfg = baseline.clone();
        match field {
            0 => cfg.starting_pot = 0,
            1 => cfg.min_bet = 0,
            2 => cfg.effective_stack = 1_000_000_001,
            3 => cfg.starting_pot = 1_000_000_001,
            4 => cfg.min_bet = 1_000_000_001,
            5 => cfg.max_nodes = 0,
            6 => cfg.max_nodes = 1_000_001,
            7 => cfg.max_raises = 33,
            _ => cfg.max_nodes = 1,
        }
        assert!(PostflopTree::new(cfg).is_err(), "accepted field {field}");
    }
    for value in [f64::NAN, f64::INFINITY, -1.0] {
        let mut cfg = baseline.clone();
        cfg.add_all_in_threshold = value;
        assert!(PostflopTree::new(cfg).is_err());
        let mut cfg = baseline.clone();
        cfg.force_all_in_threshold = value;
        assert!(PostflopTree::new(cfg).is_err());
    }
    for source in 0..3 {
        let mut cfg = baseline.clone();
        match source {
            0 => {
                cfg.sizes[Street::Flop.index()][0] =
                    BetSizeOptions::new(vec![BetSize::Pot(f64::MAX)], vec![]).unwrap()
            }
            1 => cfg.force_all_in_threshold = f64::MAX,
            _ => cfg.add_all_in_threshold = f64::MAX,
        }
        assert!(
            PostflopTree::new(cfg)
                .unwrap_err()
                .to_string()
                .contains("overflow")
        );
    }
    // Menu syntax is rejected before a config can hold it.
    for token in ["50", "50 %", "５０%", "0%", "1000000001c", "2e"] {
        assert!(BetSizeOptions::try_from((token, "")).is_err(), "{token}");
    }
    assert!(BetSizeOptions::try_from(("2x", "")).is_err());
    assert!(BetSizeOptions::new(vec![BetSize::AllIn; 65], vec![]).is_err());
    // A huge but finite size still clamps to the stack.
    let mut cfg = baseline;
    cfg.sizes[Street::Flop.index()][0] =
        BetSizeOptions::new(vec![BetSize::Pot(1e100)], vec![]).unwrap();
    let tree = PostflopTree::new(cfg).unwrap();
    assert_eq!(
        tree.node(tree.root()).unwrap().actions(),
        &[Action::Check, Action::AllIn(1000)]
    );
}

#[test]
fn small_trees_pass_independent_history_and_chip_audits() {
    for start in STREETS {
        for pot in [1, 20] {
            for stack in [0, 1, 7, 25] {
                for cap in [0, 1, 2] {
                    for minimum in [1, 5] {
                        for (add, force) in [(0.0, 0.0), (0.3, 0.2)] {
                            let mut cfg = config(start, "25%,1c,a", "50%,2x,1c,a");
                            cfg.starting_pot = pot;
                            cfg.effective_stack = stack;
                            cfg.max_raises = cap;
                            cfg.min_bet = minimum;
                            cfg.add_all_in_threshold = add;
                            cfg.force_all_in_threshold = force;
                            cfg.max_nodes = 1_000_000;
                            audit(&PostflopTree::new(cfg).unwrap());
                        }
                    }
                }
            }
        }
    }
}

/// One edge of a history: a chosen action, or the deal that ends a street.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Act(Action),
    Deal,
}

/// Walks a history from the root, following named actions and single-child deals.
fn at(tree: &PostflopTree, history: &[Step]) -> NodeId {
    let mut id = tree.root();
    for step in history {
        let node = tree.node(id).unwrap();
        id = match *step {
            Step::Deal => {
                assert!(
                    matches!(node.kind(), PostflopNodeKind::Chance { .. }),
                    "deal at {:?}",
                    node.kind()
                );
                node.children()[0]
            }
            Step::Act(action) => {
                let index = node
                    .actions()
                    .iter()
                    .position(|candidate| *candidate == action)
                    .unwrap_or_else(|| panic!("missing {action} in {:?}", node.actions()));
                node.children()[index]
            }
        };
    }
    id
}

/// What a replayed history has reached, derived only from the action labels and
/// the remaining stacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Betting,
    Dealing,
    Ended(Terminal),
}

fn settle(street: Street) -> Phase {
    if street == Street::River {
        Phase::Ended(Terminal::Showdown)
    } else {
        Phase::Dealing
    }
}

/// Replays every history using remaining stacks and action labels, independent
/// of the builder's state transitions, target generation, and private helpers.
fn audit(tree: &PostflopTree) {
    let cfg = tree.config();
    let stack = cfg.effective_stack;
    let mut pending = vec![(tree.root(), Vec::<Step>::new())];
    let mut visited = vec![false; tree.nodes().len()];
    let mut max_depth = 0;
    let mut decisions = [0usize; 3];
    let mut lives = [0usize; 3];
    while let Some((id, history)) = pending.pop() {
        assert!(!visited[id as usize], "shared child or cycle");
        visited[id as usize] = true;
        max_depth = max_depth.max(history.len());
        assert!(history.len() <= 128);

        let mut remaining = [stack; 2];
        let mut street = cfg.start_street;
        let mut base = 0;
        let mut turn = 0usize;
        let mut checked = false;
        let mut raises = 0;
        let mut phase = if stack == 0 {
            settle(street)
        } else {
            Phase::Betting
        };
        for step in &history {
            match *step {
                Step::Deal => {
                    assert_eq!(phase, Phase::Dealing, "deal outside a street transition");
                    assert_eq!(remaining[0], remaining[1], "unequal stacks at a deal");
                    street = street.next().expect("the river deals nothing");
                    base = stack - remaining[0];
                    turn = 0;
                    checked = false;
                    raises = 0;
                    phase = if remaining[0] == 0 {
                        settle(street)
                    } else {
                        Phase::Betting
                    };
                }
                Step::Act(action) => {
                    assert_eq!(phase, Phase::Betting, "action after the street ended");
                    let actor = turn % 2;
                    let facing = remaining[actor] > remaining[1 - actor];
                    match action {
                        Action::Check => {
                            assert!(!facing);
                            if checked {
                                phase = settle(street);
                            }
                            checked = true;
                        }
                        Action::Fold => {
                            assert!(facing);
                            remaining[1 - actor] = remaining[actor];
                            phase = Phase::Ended(Terminal::Fold {
                                winner: (1 - actor) as u8,
                            });
                        }
                        Action::Call => {
                            assert!(facing);
                            remaining[actor] = remaining[1 - actor];
                            phase = settle(street);
                        }
                        Action::Bet(target) | Action::Raise(target) | Action::AllIn(target) => {
                            assert!(!remaining.contains(&0));
                            let highest = stack - remaining[1 - actor];
                            assert!(target > highest && target <= stack);
                            assert_eq!(matches!(action, Action::AllIn(_)), target == stack);
                            let minimum = if facing {
                                raises += 1;
                                assert!(raises <= cfg.max_raises);
                                assert!(!matches!(action, Action::Bet(_)));
                                highest + (remaining[actor] - remaining[1 - actor]).max(cfg.min_bet)
                            } else {
                                assert!(!matches!(action, Action::Raise(_)));
                                base + cfg.min_bet
                            };
                            assert!(target >= minimum || target == stack);
                            remaining[actor] = stack - target;
                            checked = false;
                        }
                    }
                    turn += 1;
                }
            }
        }

        let node = tree.node(id).unwrap();
        let expected = remaining.map(|behind| stack - behind);
        assert_eq!(node.street(), street);
        assert_eq!(node.contributions(), expected);
        assert!(expected.iter().all(|chips| *chips <= stack));
        assert_eq!(
            cfg.starting_pot + remaining.iter().sum::<u64>() + expected.iter().sum::<u64>(),
            cfg.starting_pot + 2 * stack,
            "chips are neither created nor destroyed"
        );
        let behind = remaining[0] > 0 && remaining[1] > 0;
        match phase {
            Phase::Ended(terminal) => {
                assert_eq!(node.kind(), PostflopNodeKind::Terminal(terminal));
                assert!(node.actions().is_empty());
                assert!(node.children().is_empty());
                assert_eq!(expected[0], expected[1], "a terminal splits the wagers");
                match terminal {
                    Terminal::Showdown => {
                        assert_eq!(street, Street::River);
                        if behind {
                            lives[street.index()] += 1;
                        }
                    }
                    // The folder is whoever acted last on this street.
                    Terminal::Fold { winner } => {
                        assert_eq!(winner, 1 - ((turn - 1) % 2) as u8);
                    }
                }
            }
            Phase::Dealing => {
                let next = street.next().unwrap();
                assert_eq!(node.kind(), PostflopNodeKind::Chance { next });
                assert!(node.actions().is_empty());
                assert_eq!(node.children().len(), 1);
                assert_eq!(expected[0], expected[1], "a deal follows equal wagers");
                if behind {
                    lives[street.index()] += 1;
                }
            }
            Phase::Betting => {
                let actor = turn % 2;
                decisions[street.index()] += 1;
                assert_eq!(
                    node.kind(),
                    PostflopNodeKind::Decision {
                        player: actor as u8
                    }
                );
                assert_eq!(node.actions().len(), node.children().len());
                assert!(node.actions().windows(2).all(|pair| pair[0] < pair[1]));
                if remaining[actor] > remaining[1 - actor] {
                    assert!(node.actions().starts_with(&[Action::Fold, Action::Call]));
                    if raises == cfg.max_raises || remaining.contains(&0) {
                        assert_eq!(node.actions(), &[Action::Fold, Action::Call]);
                    }
                } else {
                    assert_eq!(node.actions().first(), Some(&Action::Check));
                }
            }
        }
        for (index, child) in node.children().iter().enumerate() {
            assert!(tree.node(*child).is_some());
            let mut next = history.clone();
            next.push(match node.kind() {
                PostflopNodeKind::Chance { .. } => Step::Deal,
                _ => Step::Act(node.actions()[index]),
            });
            pending.push((*child, next));
        }
    }
    assert!(visited.into_iter().all(|seen| seen), "unreachable node");
    assert_eq!(tree.max_depth(), max_depth);
    assert_eq!(tree.decision_nodes_per_street(), decisions);
    assert_eq!(tree.live_continuations_per_street(), lives);
}
