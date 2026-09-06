//! Focused failure and state-transition tests independent of the poker fixtures.
use crate::*;

#[derive(Clone)]
struct TinyGame {
    kinds: Vec<NodeKind>,
    children: Vec<Vec<NodeId>>,
    weights: [Vec<Real>; 2],
    probability: Real,
    mask: Vec<Real>,
    pot: Real,
    compatible: bool,
    nan: bool,
    nonzero_sum: bool,
    utility_scale: Real,
    fixed_utilities: Option<[Real; 2]>,
}

impl TinyGame {
    fn decision() -> Self {
        Self {
            kinds: vec![
                NodeKind::Player {
                    player: 0,
                    num_actions: 2,
                },
                NodeKind::Terminal,
                NodeKind::Terminal,
            ],
            children: vec![vec![1, 2], vec![], vec![]],
            weights: [vec![1.0], vec![1.0]],
            probability: 1.0,
            mask: vec![1.0],
            pot: 2.0,
            compatible: true,
            nan: false,
            nonzero_sum: false,
            utility_scale: 1.0,
            fixed_utilities: None,
        }
    }
}
impl Game for TinyGame {
    fn num_nodes(&self) -> usize {
        self.kinds.len()
    }
    fn root(&self) -> NodeId {
        0
    }
    fn kind(&self, node: NodeId) -> NodeKind {
        self.kinds[node as usize]
    }
    fn child(&self, node: NodeId, index: usize) -> NodeId {
        self.children[node as usize][index]
    }
    fn num_private_states(&self, _player: usize) -> usize {
        1
    }
    fn initial_weights(&self, player: usize) -> &[Real] {
        &self.weights[player]
    }
    fn compatible(&self, _p0: usize, _p1: usize) -> bool {
        self.compatible
    }
    fn chance_prob(&self, _node: NodeId, _outcome: usize) -> Real {
        self.probability
    }
    fn chance_mask(&self, _node: NodeId, _outcome: usize, _player: usize) -> &[Real] {
        &self.mask
    }
    fn terminal_values(&self, node: NodeId, player: usize, opponent: &[Real], out: &mut [Real]) {
        let utility = if let Some(utilities) = self.fixed_utilities {
            utilities[player]
        } else if self.nan {
            Real::NAN
        } else if self.nonzero_sum {
            1.0
        } else {
            let p0 = if node == 1 { -1.0 } else { 1.0 };
            if player == 0 { p0 } else { -p0 }
        };
        out[0] = opponent[0] * utility * self.utility_scale;
    }
    fn starting_pot(&self) -> Real {
        self.pot
    }
    fn info_label(&self, node: NodeId, player: usize, _state: usize) -> String {
        format!("{node}:{player}")
    }
}

#[test]
fn stored_regrets_distinguish_vanilla_plus_and_dcfr() {
    let game = TinyGame::decision();
    let mut vanilla = Cfr::new(&game, Variant::Vanilla).unwrap();
    let mut plus = Cfr::new(&game, Variant::Plus).unwrap();
    let mut dcfr = Cfr::new(
        &game,
        Variant::Discounted {
            alpha: 1.5,
            beta: 0.0,
            gamma: 2.0,
        },
    )
    .unwrap();
    for solver in [&mut vanilla, &mut plus, &mut dcfr] {
        solver.run_iteration(&game).unwrap();
    }
    assert_eq!(vanilla.regrets(0).unwrap(), &[-1.0, 1.0]);
    assert_eq!(plus.regrets(0).unwrap(), &[0.0, 1.0]);
    assert_eq!(dcfr.regrets(0).unwrap(), &[-0.5, 0.5]);
    for solver in [&mut vanilla, &mut plus, &mut dcfr] {
        assert_eq!(
            solver.current_strategy().unwrap().row(0).unwrap(),
            &[0.0, 1.0]
        );
        assert_eq!(
            solver.average_strategy(&game).unwrap().row(0).unwrap(),
            &[0.5, 0.5]
        );
        solver.run_iteration(&game).unwrap();
    }
    assert_eq!(vanilla.regrets(0).unwrap(), &[-3.0, 1.0]);
    assert_eq!(plus.regrets(0).unwrap(), &[0.0, 1.0]);
    assert_eq!(dcfr.regrets(0).unwrap()[0], -1.25);
    assert_eq!(
        vanilla.average_strategy(&game).unwrap().row(0).unwrap(),
        &[0.25, 0.75]
    );
    let plus_average = plus.average_strategy(&game).unwrap();
    assert!((plus_average.row(0).unwrap()[0] - 1.0 / 6.0).abs() < 1e-15);
}

#[test]
fn failed_iteration_never_exposes_partial_strategy() {
    let mut game = TinyGame::decision();
    game.nan = true;
    let mut cfr = Cfr::new(&game, Variant::Vanilla).unwrap();
    let error = cfr.run_iteration(&game).unwrap_err();
    assert!(matches!(
        error,
        SolveError::NonFinite {
            iteration: 1,
            node: 1,
            player: 0
        }
    ));
    assert_eq!(cfr.iteration(), 0);
    assert_eq!(cfr.run_iteration(&game).unwrap_err(), error);
    assert_eq!(cfr.average_strategy(&game).unwrap_err(), error);
    assert_eq!(cfr.current_strategy().unwrap_err(), error);
}

#[test]
fn driver_measures_the_final_iteration_and_names_the_cap() {
    let game = TinyGame::decision();
    let mut cfr = Cfr::new(&game, Variant::Vanilla).unwrap();
    let cfg = SolveConfig {
        target_pct_of_pot: 0.0,
        max_iterations: 2,
        check_every: 100,
        log_every_secs: 100,
        threads: 0,
    };
    let mut seen = vec![];
    let report = solve(&game, &mut cfr, &cfg, |p| seen.push(p.iterations)).unwrap();
    assert_eq!(seen, vec![2]);
    assert_eq!(report.stop_reason, StopReason::IterationCap);
    assert_eq!(report.iterations, 2);
    assert_eq!(report.exploitability.nash_conv, 0.5);
    assert_eq!(report.exploitability.pct_of_pot, 12.5);
}

#[test]
fn target_stop_requires_a_measured_accuracy() {
    let game = TinyGame::decision();
    let mut cfr = Cfr::new(&game, Variant::Vanilla).unwrap();
    let cfg = SolveConfig {
        target_pct_of_pot: 25.0,
        max_iterations: 9,
        check_every: 1,
        log_every_secs: 100,
        threads: 1,
    };
    let report = solve(&game, &mut cfr, &cfg, |_| {}).unwrap();
    assert_eq!(report.stop_reason, StopReason::TargetReached);
    assert_eq!(report.iterations, 1);
    assert_eq!(report.exploitability.pct_of_pot, 25.0);
}

#[test]
fn malformed_graph_and_probability_contracts_fail_construction() {
    let mut game = TinyGame::decision();
    game.children[0][0] = 99;
    assert!(matches!(
        Cfr::new(&game, Variant::Vanilla),
        Err(SolveError::InvalidGame(_))
    ));
    game.children[0][0] = 0;
    assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    game.children[0] = vec![1, 1];
    assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    game = TinyGame::decision();
    game.kinds[0] = NodeKind::Chance { num_outcomes: 2 };
    game.probability = 0.25;
    assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    game.probability = 0.5;
    assert!(Cfr::new(&game, Variant::Vanilla).is_ok());
    game.mask[0] = 0.3;
    assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    game = TinyGame::decision();
    game.kinds[0] = NodeKind::Player {
        player: 2,
        num_actions: 2,
    };
    assert!(Cfr::new(&game, Variant::Vanilla).is_err());
}

#[test]
fn zero_mass_and_nonpositive_pots_are_rejected() {
    let mut game = TinyGame::decision();
    game.compatible = false;
    assert!(matches!(
        Cfr::new(&game, Variant::Vanilla),
        Err(SolveError::EmptyGame)
    ));
    game.compatible = true;
    game.weights[0][0] = 0.0;
    assert!(matches!(
        Cfr::new(&game, Variant::Vanilla),
        Err(SolveError::EmptyGame)
    ));
    game.weights[0][0] = 1.0;
    for pot in [0.0, -1.0, Real::NAN, Real::INFINITY] {
        game.pot = pot;
        assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    }
}

#[test]
fn checked_strategies_reject_wrong_rows_and_changed_games() {
    let game = TinyGame::decision();
    for row in [
        vec![0.2, 0.2],
        vec![1.1, -0.1],
        vec![Real::NAN, 0.0],
        vec![1.0],
    ] {
        assert!(Strategy::from_rows(&game, vec![row, vec![], vec![]]).is_err());
    }
    let strategy = Strategy::uniform(&game).unwrap();
    let mut cfr = Cfr::new(&game, Variant::Vanilla).unwrap();
    let mut changed = game.clone();
    changed.children[0].reverse();
    assert!(expected_value(&changed, &strategy, 0).is_err());
    assert!(cfr.run_iteration(&changed).is_err());
    assert!(cfr.average_strategy(&changed).is_err());
    assert!(cfr.run_iteration(&game).is_ok());
    changed = game.clone();
    changed.weights[0][0] = 2.0;
    assert!(best_response(&changed, &strategy, 0).is_err());
    assert!(expected_value(&game, &strategy, 2).is_err());
}

#[test]
fn nonzero_sum_payoffs_are_not_zero_sum_certificates() {
    let mut game = TinyGame::decision();
    game.nonzero_sum = true;
    assert!(matches!(
        Strategy::uniform(&game),
        Err(SolveError::InvalidGame(_))
    ));
    assert!(matches!(
        Cfr::new(&game, Variant::Vanilla),
        Err(SolveError::InvalidGame(_))
    ));
}

#[test]
fn identical_shape_with_different_terminal_payoff_cannot_reuse_a_solve() {
    let game = TinyGame::decision();
    let mut solver = Cfr::new(&game, Variant::Vanilla).unwrap();
    solver.run_iteration(&game).unwrap();
    let strategy = solver.average_strategy(&game).unwrap();
    let mut changed = game.clone();
    changed.utility_scale = 2.0;
    assert!(Cfr::new(&changed, Variant::Vanilla).is_ok());
    let error = solver.run_iteration(&changed).unwrap_err();
    assert!(error.to_string().contains("terminal payoff changed"));
    assert!(solver.average_strategy(&changed).is_err());
    assert!(expected_value(&changed, &strategy, 0).is_err());
    assert!(best_response(&changed, &strategy, 1).is_err());
    assert_eq!(solver.iteration(), 1);
    assert!(solver.run_iteration(&game).is_ok());
}

#[test]
fn metric_units_convert_explicitly_in_both_directions() {
    let nash = 11.0 / 12.0;
    let pct = Exploitability::nash_conv_to_pct(nash, 2.0).unwrap();
    assert!((pct - 22.916666666666668).abs() < 1e-12);
    assert!((Exploitability::pct_to_nash_conv(pct, 2.0).unwrap() - nash).abs() < 1e-15);
    for invalid in [-1.0, Real::NAN, Real::INFINITY] {
        assert!(Exploitability::nash_conv_to_pct(invalid, 2.0).is_err());
        assert!(Exploitability::pct_to_nash_conv(invalid, 2.0).is_err());
    }
}

#[test]
fn extreme_finite_payoffs_cannot_overflow_the_zero_sum_allowance() {
    let mut game = TinyGame::decision();
    game.pot = 1e308;
    game.fixed_utilities = Some([1.5e308, -1.4e308]);
    assert!(matches!(
        Cfr::new(&game, Variant::Vanilla),
        Err(SolveError::InvalidGame(_))
    ));
    assert!(Strategy::uniform(&game).is_err());

    game.fixed_utilities = Some([1.5e308, -1.5e308]);
    let strategy = Strategy::uniform(&game).unwrap();
    let measurement = exploitability(&game, &strategy).unwrap();
    assert_eq!(measurement.nash_conv, 0.0);
    assert_eq!(measurement.pct_of_pot, 0.0);
    assert_eq!(measurement.br_value, [1.5e308, -1.5e308]);
}

#[test]
fn positive_metric_conversion_cannot_underflow_to_an_exact_zero() {
    let tiny = Real::from_bits(1);
    let error = Exploitability::nash_conv_to_pct(tiny, 1e308).unwrap_err();
    assert!(error.to_string().contains("underflow"));
    let error = Exploitability::pct_to_nash_conv(tiny, 1.0).unwrap_err();
    assert!(error.to_string().contains("underflow"));
    assert_eq!(Exploitability::nash_conv_to_pct(0.0, 1e308).unwrap(), 0.0);
    assert_eq!(Exploitability::pct_to_nash_conv(0.0, 1.0).unwrap(), 0.0);
}

#[test]
fn owned_traversal_state_cannot_enter_public_callback_apis() {
    let game = TinyGame::decision();
    let audited = crate::game::Layout::new(&game).unwrap();
    let mut core = Cfr::from_layout(audited.traversal.clone(), Variant::Vanilla, None).unwrap();
    assert!(core.run_iteration(&game).is_err());
    assert_eq!(core.iteration(), 0);
    core.advance(&mut crate::traversal::LegacyTerminal(&game))
        .unwrap();
    let policy = core.average_bound().unwrap();
    assert!(core.average_strategy(&game).is_err());
    assert!(expected_value(&game, &policy, 0).is_err());
    assert!(best_response(&game, &policy, 0).is_err());
    assert!(exploitability(&game, &policy).is_err());
    assert_eq!(core.iteration(), 1);
}

#[test]
fn fallible_terminal_boundary_poisons_the_shared_update() {
    struct FailingTerminal;
    impl crate::traversal::TerminalEvaluator for FailingTerminal {
        fn evaluate_terminal(
            &mut self,
            _node: NodeId,
            _player: usize,
            _opponent: &[Real],
            _output: &mut [Real],
            _iteration: u64,
        ) -> Result<(), SolveError> {
            Err(SolveError::InvalidGame("checked terminal underflow".into()))
        }
    }
    let game = TinyGame::decision();
    let audited = crate::game::Layout::new(&game).unwrap();
    let mut core = Cfr::from_layout(audited.traversal.clone(), Variant::Vanilla, None).unwrap();
    let failure = core.advance(&mut FailingTerminal).unwrap_err();
    assert_eq!(core.iteration(), 0);
    assert_eq!(
        core.advance(&mut crate::traversal::LegacyTerminal(&game)),
        Err(failure.clone())
    );
    assert_eq!(core.average_bound().unwrap_err(), failure);
    assert!(core.current_strategy().is_err());
}
