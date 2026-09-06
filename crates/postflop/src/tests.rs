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
}

impl TinyGame {
    fn decision() -> Self {
        Self {
            kinds: vec![NodeKind::Player { player:0, num_actions:2 }, NodeKind::Terminal, NodeKind::Terminal],
            children: vec![vec![1,2],vec![],vec![]],
            weights: [vec![1.0],vec![1.0]],
            probability: 1.0,
            mask: vec![1.0],
            pot: 2.0,
            compatible: true,
            nan: false,
            nonzero_sum: false,
        }
    }
}
impl Game for TinyGame {
    fn num_nodes(&self)->usize { self.kinds.len() }
    fn root(&self)->NodeId { 0 }
    fn kind(&self,node:NodeId)->NodeKind { self.kinds[node as usize] }
    fn child(&self,node:NodeId,index:usize)->NodeId { self.children[node as usize][index] }
    fn num_private_states(&self,_player:usize)->usize { 1 }
    fn initial_weights(&self,player:usize)->&[Real] { &self.weights[player] }
    fn compatible(&self,_p0:usize,_p1:usize)->bool { self.compatible }
    fn chance_prob(&self,_node:NodeId,_outcome:usize)->Real { self.probability }
    fn chance_mask(&self,_node:NodeId,_outcome:usize,_player:usize)->&[Real] { &self.mask }
    fn terminal_values(&self,node:NodeId,player:usize,opponent:&[Real],out:&mut [Real]) {
        let utility = if self.nan {Real::NAN} else if self.nonzero_sum {1.0} else {
            let p0 = if node == 1 {-1.0} else {1.0};
            if player == 0 {p0} else {-p0}
        };
        out[0] = opponent[0] * utility;
    }
    fn starting_pot(&self)->Real { self.pot }
    fn info_label(&self,node:NodeId,player:usize,_state:usize)->String { format!("{node}:{player}") }
}

#[test]
fn stored_regrets_distinguish_vanilla_plus_and_dcfr() {
    let game=TinyGame::decision();
    let mut vanilla=Cfr::new(&game,Variant::Vanilla).unwrap();
    let mut plus=Cfr::new(&game,Variant::Plus).unwrap();
    let mut dcfr=Cfr::new(&game,Variant::Discounted{alpha:1.5,beta:0.0,gamma:2.0}).unwrap();
    for solver in [&mut vanilla,&mut plus,&mut dcfr] { solver.run_iteration(&game).unwrap(); }
    assert_eq!(vanilla.regrets(0).unwrap(), &[-1.0,1.0]);
    assert_eq!(plus.regrets(0).unwrap(), &[0.0,1.0]);
    assert_eq!(dcfr.regrets(0).unwrap(), &[-0.5,0.5]);
    for solver in [&mut vanilla,&mut plus,&mut dcfr] {
        assert_eq!(solver.current_strategy().unwrap().row(0).unwrap(), &[0.0,1.0]);
        assert_eq!(solver.average_strategy(&game).unwrap().row(0).unwrap(), &[0.5,0.5]);
        solver.run_iteration(&game).unwrap();
    }
    assert_eq!(vanilla.regrets(0).unwrap(), &[-3.0,1.0]);
    assert_eq!(plus.regrets(0).unwrap(), &[0.0,1.0]);
    assert_eq!(dcfr.regrets(0).unwrap()[0], -1.25);
    assert_eq!(vanilla.average_strategy(&game).unwrap().row(0).unwrap(), &[0.25,0.75]);
    let plus_average = plus.average_strategy(&game).unwrap();
    assert!((plus_average.row(0).unwrap()[0]-1.0/6.0).abs()<1e-15);
}

#[test]
fn failed_iteration_never_exposes_partial_strategy() {
    let mut game=TinyGame::decision();
    game.nan=true;
    let mut cfr=Cfr::new(&game,Variant::Vanilla).unwrap();
    let error=cfr.run_iteration(&game).unwrap_err();
    assert!(matches!(error,SolveError::NonFinite{iteration:1,node:1,player:0}));
    assert_eq!(cfr.iteration(),0);
    assert_eq!(cfr.run_iteration(&game).unwrap_err(),error);
    assert_eq!(cfr.average_strategy(&game).unwrap_err(),error);
    assert_eq!(cfr.current_strategy().unwrap_err(),error);
}

#[test]
fn driver_measures_the_final_iteration_and_names_the_cap() {
    let game=TinyGame::decision();
    let mut cfr=Cfr::new(&game,Variant::Vanilla).unwrap();
    let cfg=SolveConfig{target_pct_of_pot:0.0,max_iterations:2,check_every:100,log_every_secs:100,threads:0};
    let mut seen=vec![];
    let report=solve(&game,&mut cfr,&cfg,|p| seen.push(p.iterations)).unwrap();
    assert_eq!(seen,vec![2]);
    assert_eq!(report.stop_reason,StopReason::IterationCap);
    assert_eq!(report.iterations,2);
    assert_eq!(report.exploitability.nash_conv,0.5);
    assert_eq!(report.exploitability.pct_of_pot,12.5);
}

#[test]
fn target_stop_requires_a_measured_accuracy() {
    let game=TinyGame::decision();
    let mut cfr=Cfr::new(&game,Variant::Vanilla).unwrap();
    let cfg=SolveConfig{target_pct_of_pot:25.0,max_iterations:9,check_every:1,log_every_secs:100,threads:1};
    let report=solve(&game,&mut cfr,&cfg,|_|{}).unwrap();
    assert_eq!(report.stop_reason,StopReason::TargetReached);
    assert_eq!(report.iterations,1);
    assert_eq!(report.exploitability.pct_of_pot,25.0);
}

#[test]
fn malformed_graph_and_probability_contracts_fail_construction() {
    let mut game=TinyGame::decision();
    game.children[0][0]=99;
    assert!(matches!(Cfr::new(&game,Variant::Vanilla),Err(SolveError::InvalidGame(_))));
    game.children[0][0]=0;
    assert!(Cfr::new(&game,Variant::Vanilla).is_err());
    game.children[0]=vec![1,1];
    assert!(Cfr::new(&game,Variant::Vanilla).is_err());
    game=TinyGame::decision();
    game.kinds[0]=NodeKind::Chance{num_outcomes:2};
    game.probability=0.25;
    assert!(Cfr::new(&game,Variant::Vanilla).is_err());
    game.probability=0.5;
    assert!(Cfr::new(&game,Variant::Vanilla).is_ok());
    game.mask[0]=0.3;
    assert!(Cfr::new(&game,Variant::Vanilla).is_err());
    game=TinyGame::decision();
    game.kinds[0]=NodeKind::Player{player:2,num_actions:2};
    assert!(Cfr::new(&game,Variant::Vanilla).is_err());
}

#[test]
fn zero_mass_and_nonpositive_pots_are_rejected() {
    let mut game=TinyGame::decision();
    game.compatible=false;
    assert!(matches!(Cfr::new(&game,Variant::Vanilla),Err(SolveError::EmptyGame)));
    game.compatible=true;
    game.weights[0][0]=0.0;
    assert!(matches!(Cfr::new(&game,Variant::Vanilla),Err(SolveError::EmptyGame)));
    game.weights[0][0]=1.0;
    for pot in [0.0,-1.0,Real::NAN,Real::INFINITY] {
        game.pot=pot;
        assert!(Cfr::new(&game,Variant::Vanilla).is_err());
    }
}

#[test]
fn checked_strategies_reject_wrong_rows_and_changed_games() {
    let game=TinyGame::decision();
    for row in [vec![0.2,0.2],vec![1.1,-0.1],vec![Real::NAN,0.0],vec![1.0]] {
        assert!(Strategy::from_rows(&game,vec![row,vec![],vec![]]).is_err());
    }
    let strategy=Strategy::uniform(&game).unwrap();
    let mut cfr=Cfr::new(&game,Variant::Vanilla).unwrap();
    let mut changed=game.clone();
    changed.children[0].reverse();
    assert!(expected_value(&changed,&strategy,0).is_err());
    assert!(cfr.run_iteration(&changed).is_err());
    assert!(cfr.average_strategy(&changed).is_err());
    assert!(cfr.run_iteration(&game).is_ok());
    changed=game.clone();
    changed.weights[0][0]=2.0;
    assert!(best_response(&changed,&strategy,0).is_err());
    assert!(expected_value(&game,&strategy,2).is_err());
}

#[test]
fn nonzero_sum_payoffs_are_not_zero_sum_certificates() {
    let mut game=TinyGame::decision();
    game.nonzero_sum=true;
    let strategy=Strategy::uniform(&game).unwrap();
    assert!(matches!(exploitability(&game,&strategy),Err(SolveError::InvalidGame(_))));
}

#[test]
fn metric_units_convert_explicitly_in_both_directions() {
    let nash=11.0/12.0;
    let pct=Exploitability::nash_conv_to_pct(nash,2.0).unwrap();
    assert!((pct-22.916666666666668).abs()<1e-12);
    assert!((Exploitability::pct_to_nash_conv(pct,2.0).unwrap()-nash).abs()<1e-15);
    for invalid in [-1.0,Real::NAN,Real::INFINITY] {
        assert!(Exploitability::nash_conv_to_pct(invalid,2.0).is_err());
        assert!(Exploitability::pct_to_nash_conv(invalid,2.0).is_err());
    }
}