use postflop::{Cfr, Exploitability, Strategy, Variant, expected_value, exploitability};
use serde::Deserialize;
use toygames::ToyGame;

#[derive(Deserialize)]
pub struct Fixture {
    cfr: Run,
    cfr_plus: Run,
    dcfr: Run,
}

#[derive(Deserialize)]
struct Run {
    budget: u64,
    target_nash_conv: Option<f64>,
    checkpoints: Vec<Point>,
}

#[derive(Deserialize)]
struct Point {
    iteration: u64,
    nash_conv: f64,
    player_0_value: f64,
}

impl Fixture {
    pub fn final_reference(&self) -> (f64, f64) {
        let last = self.cfr_plus.checkpoints.last().unwrap();
        (last.player_0_value, last.nash_conv)
    }
}

pub fn check_curve(game: &ToyGame, variant: Variant, fixture: &Fixture) -> (Strategy, Exploitability, f64) {
    let reference = match variant {
        Variant::Vanilla => &fixture.cfr,
        Variant::Plus => &fixture.cfr_plus,
        Variant::Discounted { .. } => &fixture.dcfr,
    };
    let mut solver = Cfr::new(game, variant).unwrap();
    let max_iterations = reference.budget.max(reference.checkpoints.last().unwrap().iteration);
    let mut at_budget = None;
    for iteration in 1..=max_iterations {
        solver.run_iteration(game).unwrap();
        let point = reference.checkpoints.iter().find(|p| p.iteration == iteration);
        if point.is_none() && iteration != reference.budget { continue; }
        let strategy = solver.average_strategy(game).unwrap();
        let metrics = exploitability(game, &strategy).unwrap();
        let value = expected_value(game, &strategy, 0).unwrap();
        println!("{:?} {variant:?} iteration={iteration} nash_conv={:.15e} pct_of_pot={:.15e} player_0_value={value:.15e}", game.rules(), metrics.nash_conv, metrics.pct_of_pot);
        if let Some(point) = point {
            for (metric, actual, expected) in [("nash_conv", metrics.nash_conv, point.nash_conv), ("player_0_value", value, point.player_0_value)] {
                let tolerance = 1e-9 + 1e-6 * expected.abs();
                assert!((actual - expected).abs() <= tolerance,
                    "{variant:?} iteration={iteration} {metric}: {actual:.17} != reference {expected:.17}, tolerance={tolerance}");
            }
        }
        if iteration == reference.budget {
            if let Some(target) = reference.target_nash_conv {
                assert!(metrics.nash_conv < target,
                    "{variant:?} fixed budget {iteration}: {} >= {target}", metrics.nash_conv);
            }
            at_budget = Some((strategy, metrics, value));
        }
    }
    at_budget.unwrap()
}
