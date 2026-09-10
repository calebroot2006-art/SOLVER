use std::io::Write;
use std::path::Path;

use postflop::{
    Cfr, Exploitability, Game, NodeKind, Strategy, Variant, expected_value, exploitability,
};
use serde::Deserialize;
use toygames::{Rules, ToyGame};

fn dump_trace(game: &ToyGame, solver: &Cfr, variant: Variant, directory: &Path) {
    let name = match variant {
        Variant::Vanilla => "cfr",
        Variant::Plus => "cfr_plus",
        Variant::Discounted { .. } => "dcfr",
    };
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(format!("leduc_{name}_{:04}.csv", solver.iteration()));
    let mut output = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    writeln!(
        output,
        "iteration,history,player,hand,action,regret,current,strategy_sum"
    )
    .unwrap();
    let current = solver.current_strategy().unwrap();
    for node in 0..game.num_nodes() as u32 {
        let NodeKind::Player {
            player,
            num_actions,
        } = game.kind(node)
        else {
            continue;
        };
        let regrets = solver.regrets(node).unwrap();
        let sums = solver.strategy_sum(node).unwrap();
        for hand in 0..game.num_private_states(usize::from(player)) {
            if game.board(node) == Some(hand) {
                continue;
            }
            let label = game.info_label(node, usize::from(player), hand);
            let history = label.split_once("history=").unwrap().1;
            for (action, character) in game.actions(node).iter().enumerate() {
                let entry = hand * usize::from(num_actions) + action;
                writeln!(
                    output,
                    "{},{history},{player},{hand},{character},{:.17e},{:.17e},{:.17e}",
                    solver.iteration(),
                    regrets[entry],
                    current.row(node).unwrap()[entry],
                    sums[entry]
                )
                .unwrap();
            }
        }
    }
    output.flush().unwrap();
    let average = solver.average_strategy(game).unwrap();
    let mut metadata =
        std::io::BufWriter::new(std::fs::File::create(path.with_extension("metrics.csv")).unwrap());
    writeln!(
        metadata,
        "schema_version,game,players,suit_isomorphism,starting_player,action_mapping,iteration,profile,player_0_value,br0,br1,nash_conv"
    )
    .unwrap();
    for (profile, strategy) in [("current", &current), ("average", &average)] {
        let metrics = exploitability(game, strategy).unwrap();
        let value = expected_value(game, strategy, 0).unwrap();
        writeln!(
            metadata,
            "1,leduc_poker,2,false,0,false,{},{profile},{value:.17e},{:.17e},{:.17e},{:.17e}",
            solver.iteration(),
            metrics.br_value[0],
            metrics.br_value[1],
            metrics.nash_conv
        )
        .unwrap();
    }
    metadata.flush().unwrap();
}

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
    strict_curve_through: u64,
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

pub fn check_curve(
    game: &ToyGame,
    variant: Variant,
    fixture: &Fixture,
) -> (Strategy, Exploitability, f64) {
    let reference = match variant {
        Variant::Vanilla => &fixture.cfr,
        Variant::Plus => &fixture.cfr_plus,
        Variant::Discounted { .. } => &fixture.dcfr,
    };
    let mut solver = Cfr::new(game, variant).unwrap();
    let max_iterations = reference
        .budget
        .max(reference.checkpoints.last().unwrap().iteration);
    let mut at_budget = None;
    let mut mismatches = Vec::new();
    let trace_directory =
        std::env::var_os("ASTRA_CFR_TRACE_DIR").filter(|_| game.rules() == Rules::Leduc);
    if let Some(directory) = &trace_directory {
        dump_trace(game, &solver, variant, Path::new(directory));
    }
    for iteration in 1..=max_iterations {
        solver.run_iteration(game).unwrap();
        if let Some(directory) = &trace_directory
            && [
                1, 2, 5, 10, 20, 50, 51, 100, 101, 200, 201, 500, 1000, 1001, 2000, 5000, 10000,
            ]
            .contains(&iteration)
        {
            dump_trace(game, &solver, variant, Path::new(directory));
        }
        let point = reference
            .checkpoints
            .iter()
            .find(|p| p.iteration == iteration);
        if point.is_none() && iteration != reference.budget {
            continue;
        }
        let strategy = solver.average_strategy(game).unwrap();
        let metrics = exploitability(game, &strategy).unwrap();
        let value = expected_value(game, &strategy, 0).unwrap();
        println!(
            "{:?} {variant:?} iteration={iteration} nash_conv={:.15e} pct_of_pot={:.15e} player_0_value={value:.15e}",
            game.rules(),
            metrics.nash_conv,
            metrics.pct_of_pot
        );
        if let Some(point) = point {
            for (metric, actual, expected) in [
                ("nash_conv", metrics.nash_conv, point.nash_conv),
                ("player_0_value", value, point.player_0_value),
            ] {
                let tolerance = 1e-9 + 1e-6 * expected.abs();
                if (actual - expected).abs() > tolerance {
                    let message = format!(
                        "{variant:?} iteration={iteration} {metric}: {actual:.17} != reference {expected:.17}, tolerance={tolerance}"
                    );
                    if iteration <= reference.strict_curve_through {
                        mismatches.push(message);
                    } else {
                        println!("Recorded trajectory drift: {message}");
                    }
                }
            }
        }
        if iteration >= reference.budget
            && let Some(target) = reference.target_nash_conv
            && metrics.nash_conv >= target
        {
            mismatches.push(format!(
                "{variant:?} iteration={iteration} after budget={}: {} >= {target}",
                reference.budget, metrics.nash_conv
            ));
        }
        if iteration == reference.budget {
            at_budget = Some((strategy, metrics, value));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    at_budget.unwrap()
}
