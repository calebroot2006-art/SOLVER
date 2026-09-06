use postflop::{Cfr, Game, NodeId, NodeKind, Real, SolveConfig, SolveError, SolverConfig, StopReason, Strategy, Variant, solve};
use toygames::{ToyGame, kuhn, nan_game::NanGame};

#[test]
fn nan_terminal_fails_loudly_and_cannot_return_a_strategy() {
    let mut solver = Cfr::new(&NanGame, Variant::Vanilla).unwrap();
    let config = SolveConfig { target_pct_of_pot: 0.0, max_iterations: 1, check_every: 1, log_every_secs: 1, threads: 0 };
    let error = solve(&NanGame, &mut solver, &config, |_| panic!("nonfinite values must not be progress")).unwrap_err();
    assert!(matches!(error, SolveError::NonFinite { iteration: 1, .. }), "{error:?}");
    assert!(solver.average_strategy(&NanGame).is_err());
    assert!(solver.current_strategy().is_err());
}

#[test]
fn iteration_cap_and_target_are_distinct_and_measured() {
    let game = kuhn::game();
    for (target, expected) in [(0.0, StopReason::IterationCap), (100.0, StopReason::TargetReached)] {
        let mut solver = Cfr::new(&game, Variant::Vanilla).unwrap();
        let config = SolveConfig { target_pct_of_pot: target, max_iterations: 1, check_every: 1, log_every_secs: 1, threads: 0 };
        let mut callbacks = 0;
        let report = solve(&game, &mut solver, &config, |_| callbacks += 1).unwrap();
        assert_eq!(report.stop_reason, expected);
        assert_eq!(report.iterations, 1);
        assert_eq!(callbacks, 1);
        assert!((report.exploitability.nash_conv - 11.0 / 12.0).abs() < 1e-12);
    }
}

#[test]
fn private_weights_and_strategy_rows_are_checked() {
    for weights in [
        [vec![1.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]],
        [vec![0.0; 3], vec![1.0; 3]],
    ] {
        assert!(matches!(Cfr::new(&kuhn::game().with_weights(weights), Variant::Vanilla), Err(SolveError::EmptyGame)));
    }
    for weights in [
        [vec![1.0; 2], vec![1.0; 3]],
        [vec![-1.0, 1.0, 1.0], vec![1.0; 3]],
        [vec![Real::NAN, 1.0, 1.0], vec![1.0; 3]],
        [vec![Real::INFINITY, 1.0, 1.0], vec![1.0; 3]],
    ] {
        assert!(Cfr::new(&kuhn::game().with_weights(weights), Variant::Vanilla).is_err());
    }
    let game = kuhn::game();
    for bad in [Real::NAN, Real::INFINITY, -0.1, 0.7] {
        let mut rows = Strategy::uniform(&game).unwrap().rows().to_vec();
        rows[0][0] = bad;
        assert!(Strategy::from_rows(&game, rows).is_err());
    }
    assert!(Strategy::from_rows(&game, Vec::new()).is_err());
}

const VALID: &str = "[solve]\ntarget_pct_of_pot=0.5\nmax_iterations=10\ncheck_every=1\nlog_every_secs=1\nthreads=0\n[dcfr]\nalpha=1.5\nbeta=0.0\ngamma=2.0\n";

#[test]
fn configuration_failures_name_the_field() {
    SolverConfig::from_toml(VALID).unwrap();
    for (old, new, field) in [
        ("check_every=1\n", "", "check_every"),
        ("check_every=1", "check_every=0", "check_every"),
        ("max_iterations=10", "max_iterations=0", "max_iterations"),
        ("log_every_secs=1", "log_every_secs=0", "log_every_secs"),
        ("target_pct_of_pot=0.5", "target_pct_of_pot=nan", "target_pct_of_pot"),
        ("target_pct_of_pot=0.5", "target_pct_of_pot=inf", "target_pct_of_pot"),
        ("target_pct_of_pot=0.5", "target_pct_of_pot=-0.5", "target_pct_of_pot"),
        ("alpha=1.5", "alpha=nan", "alpha"),
    ] {
        let error = SolverConfig::from_toml(&VALID.replace(old, new)).unwrap_err();
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
}

struct Malformed { inner: ToyGame, pot: Real, cycle: bool }

impl Game for Malformed {
    fn num_nodes(&self) -> usize { self.inner.num_nodes() }
    fn root(&self) -> NodeId { self.inner.root() }
    fn kind(&self, node: NodeId) -> NodeKind { self.inner.kind(node) }
    fn child(&self, node: NodeId, index: usize) -> NodeId { if self.cycle && node == 0 { 0 } else { self.inner.child(node, index) } }
    fn num_private_states(&self, player: usize) -> usize { self.inner.num_private_states(player) }
    fn initial_weights(&self, player: usize) -> &[Real] { self.inner.initial_weights(player) }
    fn compatible(&self, first: usize, second: usize) -> bool { self.inner.compatible(first, second) }
    fn chance_prob(&self, node: NodeId, outcome: usize) -> Real { self.inner.chance_prob(node, outcome) }
    fn chance_mask(&self, node: NodeId, outcome: usize, player: usize) -> &[Real] { self.inner.chance_mask(node, outcome, player) }
    fn terminal_values(&self, node: NodeId, player: usize, opp: &[Real], out: &mut [Real]) { self.inner.terminal_values(node, player, opp, out); }
    fn starting_pot(&self) -> Real { self.pot }
    fn info_label(&self, node: NodeId, player: usize, state: usize) -> String { self.inner.info_label(node, player, state) }
}

#[test]
fn malformed_tree_and_pot_are_rejected_at_construction() {
    for pot in [0.0, -1.0, Real::NAN, Real::INFINITY] {
        let game = Malformed { inner: kuhn::game(), pot, cycle: false };
        assert!(Cfr::new(&game, Variant::Vanilla).is_err());
    }
    let cycle = Malformed { inner: kuhn::game(), pot: 2.0, cycle: true };
    assert!(Cfr::new(&cycle, Variant::Vanilla).is_err());
}
