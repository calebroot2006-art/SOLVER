//! Betting syntax, exact histories, and independent whole-tree legality checks.

use tree::{
    Action, BetSize, BetSizeOptions, RiverNode, RiverNodeKind, RiverTree, RiverTreeConfig,
    Terminal,
};

fn config(bets: &str, raises: &str) -> RiverTreeConfig {
    let sizes = BetSizeOptions::try_from((bets, raises)).unwrap();
    RiverTreeConfig {
        starting_pot: 100,
        effective_stack: 1000,
        min_bet: 10,
        sizes: [sizes.clone(), sizes],
        max_raises: 2,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 50_000,
    }
}

fn at<'a>(tree: &'a RiverTree, history: &[Action]) -> &'a RiverNode {
    let mut id = tree.root();
    for action in history {
        let node = tree.node(id).unwrap();
        let index = node.actions().iter().position(|candidate| candidate == action)
            .unwrap_or_else(|| panic!("missing {action} in {:?}", node.actions()));
        id = node.children()[index];
    }
    tree.node(id).unwrap()
}

#[test]
fn parser_and_constructor_agree_and_deduplicate() {
    let parsed = BetSizeOptions::try_from((" 50% , 20c, a,e,50% ", "2.5x,20c,50%,a")).unwrap();
    let direct = BetSizeOptions::new(
        vec![BetSize::Pot(0.5), BetSize::Additive(20), BetSize::AllIn],
        vec![BetSize::PreviousBet(2.5), BetSize::Additive(20), BetSize::Pot(0.5), BetSize::AllIn],
    ).unwrap();
    assert_eq!(parsed, direct);
    assert!(BetSizeOptions::try_from((" \t\r\n\u{b}\u{c}", "")).unwrap().bets().is_empty());
    assert_eq!(BetSizeOptions::try_from(("\u{b}50%\u{b}", "")).unwrap().bets(), &[BetSize::Pot(0.5)]);
    assert_eq!(tree::crate_name(), "tree");
}

#[test]
fn malformed_and_oversized_menus_are_rejected_before_deduplication() {
    for token in [",", "a,", ",a", "a,,e", "50 %", "2 0c", "a e", "5\u{b}0%", "５０%", "a\u{a0}", "50", "2e", "e:3", "2x:1", "A", "%", "x", "c", "0%", "-1%", "NaN%", "inf%", "1e999%", "0c", "-1c", "1000000001c", "18446744073709551616c"] {
        assert!(BetSizeOptions::try_from((token, "")).is_err(), "accepted {token:?}");
        assert!(BetSizeOptions::try_from(("", token)).is_err(), "accepted {token:?}");
    }
    for token in ["0x", "1x", "-2x", "NaNx", "infx"] {
        assert!(BetSizeOptions::try_from(("", token)).is_err());
    }
    assert!(BetSizeOptions::try_from(("2x", "")).is_err());
    assert!(BetSizeOptions::try_from(("", "2x")).is_ok());
    let entries = vec!["a"; 65].join(",");
    assert!(BetSizeOptions::try_from((entries.as_str(), "")).is_err());
    assert!(BetSizeOptions::try_from(("", entries.as_str())).is_err());
    let spaces = " ".repeat(4097);
    assert!(BetSizeOptions::try_from((spaces.as_str(), "")).is_err());
    assert!(BetSizeOptions::try_from((" ".repeat(4096).as_str(), "")).is_ok());
    assert!(BetSizeOptions::new(vec![BetSize::AllIn; 65], vec![]).is_err());
    assert!(BetSizeOptions::new(vec![], vec![BetSize::AllIn; 65]).is_err());
    for size in [BetSize::Pot(f64::NAN), BetSize::Pot(f64::INFINITY), BetSize::Pot(0.0), BetSize::Additive(0), BetSize::Additive(1_000_000_001), BetSize::PreviousBet(1.0)] {
        assert!(BetSizeOptions::new(vec![size], vec![]).is_err());
        assert!(BetSizeOptions::new(vec![], vec![size]).is_err());
    }
}

#[test]
fn percent_multiplier_additive_and_half_up_amounts_are_exact() {
    let tree = RiverTree::new(config("50%,20c", "50%,2.5x,20c")).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::Bet(20), Action::Bet(50)]);
    assert_eq!(at(&tree, &[Action::Bet(50)]).actions(), &[Action::Fold, Action::Call, Action::Raise(100), Action::Raise(125), Action::Raise(150)]);
    assert_eq!(at(&tree, &[Action::Bet(50), Action::Raise(125)]).actions(), &[Action::Fold, Action::Call, Action::Raise(200), Action::Raise(300), Action::Raise(313)]);
    let mut cfg = config("50%", "50%,2.5x,1c");
    cfg.starting_pot = 5;
    cfg.min_bet = 1;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::Bet(3)]);
    assert_eq!(at(&tree, &[Action::Bet(3)]).actions(), &[Action::Fold, Action::Call, Action::Raise(6), Action::Raise(8), Action::Raise(9)]);
}

#[test]
fn node_histories_end_in_checks_calls_or_refunded_folds() {
    let tree = RiverTree::new(config("20c", "20c")).unwrap();
    let checks = at(&tree, &[Action::Check, Action::Check]);
    assert_eq!(checks.kind(), RiverNodeKind::Terminal(Terminal::Showdown));
    assert_eq!(checks.contributions(), [0, 0]);
    let call = at(&tree, &[Action::Bet(20), Action::Call]);
    assert_eq!(call.kind(), RiverNodeKind::Terminal(Terminal::Showdown));
    assert_eq!(call.contributions(), [20, 20]);
    let fold = at(&tree, &[Action::Check, Action::Bet(20), Action::Fold]);
    assert_eq!(fold.kind(), RiverNodeKind::Terminal(Terminal::Fold { winner: 1 }));
    assert_eq!(fold.contributions(), [0, 0]);
    let raised_fold = at(&tree, &[Action::Bet(20), Action::Raise(40), Action::Fold]);
    assert_eq!(raised_fold.kind(), RiverNodeKind::Terminal(Terminal::Fold { winner: 1 }));
    assert_eq!(raised_fold.contributions(), [20, 20]);
    let reraised = at(&tree, &[Action::Bet(20), Action::Raise(40), Action::Raise(60)]);
    assert_eq!(reraised.actions(), &[Action::Fold, Action::Call]);
}

#[test]
fn all_in_gets_a_fold_call_response_even_below_the_minimum() {
    for stack in [7, 20, 1000] {
        let mut cfg = config("a,1c,1000000000c", "a,50%");
        cfg.effective_stack = stack;
        let tree = RiverTree::new(cfg).unwrap();
        let node = at(&tree, &[Action::AllIn(stack)]);
        assert_eq!(node.actions(), &[Action::Fold, Action::Call]);
        assert_eq!(at(&tree, &[Action::AllIn(stack), Action::Call]).contributions(), [stack; 2]);
        assert_eq!(at(&tree, &[Action::AllIn(stack), Action::Fold]).contributions(), [0; 2]);
        if stack == 7 {
            assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::AllIn(7)]);
        }
    }
    let mut cfg = config("15c", "1c,a");
    cfg.effective_stack = 20;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[Action::Bet(15)]).actions(), &[Action::Fold, Action::Call, Action::AllIn(20)]);
    assert_eq!(at(&tree, &[Action::Bet(15), Action::AllIn(20)]).actions(), &[Action::Fold, Action::Call]);
}

#[test]
fn action_order_and_target_deduplication_are_deterministic() {
    let first = RiverTree::new(config("a,50%,50c,20c,e", "2x,100%,a")).unwrap();
    let second = RiverTree::new(config("20c,50c,e,50%,a", "a,100%,2x")).unwrap();
    assert_eq!(first.nodes(), second.nodes());
    let actions = [Action::Fold, Action::Check, Action::Call, Action::Bet(5), Action::Raise(10), Action::AllIn(20)];
    assert_eq!(actions.map(|action| action.to_string()), ["fold", "check", "call", "bet:5", "raise:10", "allin:20"]);
}

#[test]
fn thresholds_add_or_merge_all_ins_and_respect_the_raise_cap() {
    let mut cfg = config("20c,30c", "");
    cfg.effective_stack = 100;
    cfg.force_all_in_threshold = 0.5;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::Bet(20), Action::AllIn(100)]);

    let mut cfg = config("50c", "");
    cfg.effective_stack = 200;
    cfg.add_all_in_threshold = 0.75;
    let tree = RiverTree::new(cfg.clone()).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::Bet(50)]);
    assert_eq!(at(&tree, &[Action::Bet(50)]).actions(), &[Action::Fold, Action::Call, Action::AllIn(200)]);
    cfg.max_raises = 0;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[Action::Bet(50)]).actions(), &[Action::Fold, Action::Call]);

    let mut cfg = config("", "");
    cfg.add_all_in_threshold = 10.0;
    cfg.max_raises = 0;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::AllIn(1000)]);
    assert_eq!(at(&tree, &[Action::AllIn(1000)]).actions(), &[Action::Fold, Action::Call]);
    let tree = RiverTree::new(config("", "a")).unwrap();
    assert_eq!(tree.nodes().len(), 3);
}

#[test]
fn player_menus_are_distinct_and_minimum_bets_are_clamped() {
    let mut cfg = config("1%", "");
    cfg.sizes[1] = BetSizeOptions::try_from(("20c", "a")).unwrap();
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::Bet(10)]);
    assert_eq!(at(&tree, &[Action::Check]).actions(), &[Action::Check, Action::Bet(20)]);
    assert_eq!(at(&tree, &[Action::Bet(10)]).actions(), &[Action::Fold, Action::Call, Action::AllIn(1000)]);
}

#[test]
fn terminal_only_stack_and_exact_resource_budget() {
    let mut cfg = config("a", "a");
    cfg.effective_stack = 0;
    cfg.max_nodes = 1;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(tree.nodes().len(), 1);
    assert_eq!(tree.max_depth(), 0);
    assert_eq!(at(&tree, &[]).kind(), RiverNodeKind::Terminal(Terminal::Showdown));
    assert!(at(&tree, &[]).actions().is_empty());
    assert!(tree.node(u32::MAX).is_none());
    let mut cfg = config("20c", "20c");
    let count = RiverTree::new(cfg.clone()).unwrap().nodes().len();
    cfg.max_nodes = count;
    let tree = RiverTree::new(cfg.clone()).unwrap();
    assert_eq!(tree.nodes().len(), count);
    assert!(tree.storage_bytes() >= std::mem::size_of::<RiverTree>() + count * std::mem::size_of::<RiverNode>());
    cfg.max_nodes -= 1;
    assert!(RiverTree::new(cfg).unwrap_err().to_string().contains("max_nodes"));
}

#[test]
fn maximum_raise_cap_and_retained_menu_capacity_are_accounted_for() {
    let mut cfg = config("1c", "1c");
    cfg.min_bet = 1;
    cfg.max_raises = 32;
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(tree.max_depth(), 35);
    audit(&tree);

    let mut large_menu = Vec::with_capacity(64);
    large_menu.push(BetSize::AllIn);
    let mut cfg = config("a", "");
    let smaller = RiverTree::new(cfg.clone()).unwrap();
    cfg.sizes[0] = BetSizeOptions::new(large_menu, vec![]).unwrap();
    let larger = RiverTree::new(cfg).unwrap();
    assert_eq!(smaller.nodes(), larger.nodes());
    assert_eq!(larger.storage_bytes() - smaller.storage_bytes(), 63 * std::mem::size_of::<BetSize>());
}

#[test]
fn invalid_configuration_and_nonfinite_intermediates_fail() {
    let baseline = config("a", "a");
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
        assert!(RiverTree::new(cfg).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, -1.0] {
        let mut cfg = baseline.clone();
        cfg.add_all_in_threshold = value;
        assert!(RiverTree::new(cfg).is_err());
        let mut cfg = baseline.clone();
        cfg.force_all_in_threshold = value;
        assert!(RiverTree::new(cfg).is_err());
    }
    for source in 0..3 {
        let mut cfg = baseline.clone();
        match source {
            0 => cfg.sizes[0] = BetSizeOptions::new(vec![BetSize::Pot(f64::MAX)], vec![]).unwrap(),
            1 => cfg.force_all_in_threshold = f64::MAX,
            _ => cfg.add_all_in_threshold = f64::MAX,
        }
        assert!(RiverTree::new(cfg).unwrap_err().to_string().contains("overflow"));
    }
    let mut cfg = baseline;
    cfg.sizes[0] = BetSizeOptions::new(vec![BetSize::Pot(1e100)], vec![]).unwrap();
    let tree = RiverTree::new(cfg).unwrap();
    assert_eq!(at(&tree, &[]).actions(), &[Action::Check, Action::AllIn(1000)]);
}

// Replay each history using remaining stacks and action labels, independent of
// the builder's state transitions, target generation, and private helpers.
fn audit(tree: &RiverTree) {
    let cfg = tree.config();
    let mut pending = vec![(tree.root(), Vec::<Action>::new())];
    let mut visited = vec![false; tree.nodes().len()];
    let mut max_depth = 0;
    while let Some((id, history)) = pending.pop() {
        assert!(!visited[id as usize], "shared child or cycle");
        visited[id as usize] = true;
        max_depth = max_depth.max(history.len());
        assert!(history.len() <= 128);
        let mut remaining = [cfg.effective_stack; 2];
        let mut ended = (cfg.effective_stack == 0).then_some(Terminal::Showdown);
        let mut raises = 0;
        for (turn, action) in history.iter().enumerate() {
            assert!(ended.is_none());
            let actor = turn % 2;
            let facing = remaining[actor] > remaining[1 - actor];
            match *action {
                Action::Check => {
                    assert!(!facing);
                    if turn > 0 && history[turn - 1] == Action::Check {
                        ended = Some(Terminal::Showdown);
                    }
                }
                Action::Fold => {
                    assert!(facing);
                    remaining[1 - actor] = remaining[actor];
                    ended = Some(Terminal::Fold { winner: (1 - actor) as u8 });
                }
                Action::Call => {
                    assert!(facing);
                    remaining[actor] = remaining[1 - actor];
                    ended = Some(Terminal::Showdown);
                }
                Action::Bet(target) | Action::Raise(target) | Action::AllIn(target) => {
                    assert!(!remaining.contains(&0));
                    let highest = cfg.effective_stack - remaining[1 - actor];
                    assert!(target > highest && target <= cfg.effective_stack);
                    assert_eq!(matches!(action, Action::AllIn(_)), target == cfg.effective_stack);
                    let minimum = if facing {
                        raises += 1;
                        assert!(raises <= cfg.max_raises);
                        assert!(!matches!(action, Action::Bet(_)));
                        highest + (remaining[actor] - remaining[1 - actor]).max(cfg.min_bet)
                    } else {
                        assert!(!matches!(action, Action::Raise(_)));
                        cfg.min_bet
                    };
                    assert!(target >= minimum || target == cfg.effective_stack);
                    remaining[actor] = cfg.effective_stack - target;
                }
            }
        }
        let node = tree.node(id).unwrap();
        let expected = remaining.map(|stack| cfg.effective_stack - stack);
        assert_eq!(node.contributions(), expected);
        assert_eq!(cfg.starting_pot + remaining.iter().sum::<u64>() + node.contributions().iter().sum::<u64>(), cfg.starting_pot + 2 * cfg.effective_stack);
        assert_eq!(node.actions().len(), node.children().len());
        assert!(node.actions().windows(2).all(|pair| pair[0] < pair[1]));
        if let Some(terminal) = ended {
            assert_eq!(node.kind(), RiverNodeKind::Terminal(terminal));
            assert!(node.actions().is_empty());
            assert_eq!(expected[0], expected[1]);
        } else {
            let actor = history.len() % 2;
            assert_eq!(node.kind(), RiverNodeKind::Decision { player: actor as u8 });
            if remaining[actor] > remaining[1 - actor] {
                assert!(node.actions().starts_with(&[Action::Fold, Action::Call]));
                if raises == cfg.max_raises || remaining.contains(&0) {
                    assert_eq!(node.actions(), &[Action::Fold, Action::Call]);
                }
            } else {
                assert_eq!(node.actions().first(), Some(&Action::Check));
            }
        }
        for (action, child) in node.actions().iter().zip(node.children()) {
            assert!(tree.node(*child).is_some());
            let mut next = history.clone();
            next.push(*action);
            pending.push((*child, next));
        }
    }
    assert!(visited.into_iter().all(|seen| seen));
    assert_eq!(tree.max_depth(), max_depth);
}

#[test]
fn small_trees_pass_independent_history_and_chip_audits() {
    for pot in [1, 3, 20] {
        for stack in [0, 1, 7, 25] {
            for cap in [0, 1, 3] {
                for minimum in [1, 5] {
                    for (add, force) in [(0.0, 0.0), (0.3, 0.2)] {
                        let mut cfg = config("25%,1c,a", "50%,2x,1c,a");
                        cfg.starting_pot = pot;
                        cfg.effective_stack = stack;
                        cfg.max_raises = cap;
                        cfg.min_bet = minimum;
                        cfg.add_all_in_threshold = add;
                        cfg.force_all_in_threshold = force;
                        audit(&RiverTree::new(cfg).unwrap());
                    }
                }
            }
        }
    }
}
