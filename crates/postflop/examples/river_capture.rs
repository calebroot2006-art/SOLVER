//! Capture measured river policies for the independent reference comparison.
use cards::{Card, Combo, Range};
use postflop::{RiverGame, RiverSolver, SolveConfig, Variant};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs, time::Instant};
use tree::{BetSizeOptions, RiverNodeKind, RiverTree, RiverTreeConfig, Terminal};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    board: [String; 5],
    ranges: [String; 2],
    chips_per_bb: u64,
    starting_pot: u64,
    effective_stack: u64,
    bets: [String; 2],
    raises: [String; 2],
    min_bet: u64,
    max_raises: u8,
    add_all_in_threshold: u64,
    force_all_in_threshold: u64,
    target_pct_of_pot: f64,
    max_iterations: u64,
    check_every: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs { schema_version: u64, cases: Vec<Case> }

#[derive(Serialize)]
struct Hand {
    cards: [String; 2],
    strategy: Vec<f64>,
    action_expected_values: Vec<f64>,
    ev_available: bool,
    own_reach: f64,
    opponent_mass: f64,
}

#[derive(Serialize)]
struct Node {
    history_labels: Vec<String>,
    kind: String,
    player: i8,
    actions: Vec<String>,
    contributions: [u64; 2],
    terminal: String,
    fold_winner: i8,
    hands: Vec<Hand>,
}

#[derive(Serialize)]
struct Checkpoint { iterations: u64, pct_of_pot: f64, elapsed_seconds: f64 }

#[derive(Serialize)]
struct Capture {
    input: Case,
    iterations: u64,
    stop_reason: String,
    exploitability_pct_of_pot: f64,
    root_centered_expected_values: [f64; 2],
    best_response_values: [f64; 2],
    compatible_weight: f64,
    nodes: Vec<Node>,
    checkpoints: Vec<Checkpoint>,
    working_set_bound_bytes: usize,
    reserved_bytes: usize,
    elapsed_seconds: f64,
}

#[derive(Serialize)]
struct Output { schema_version: u64, os: String, architecture: String, cases: Vec<Capture> }

fn capture(input: Case) -> Result<Capture, Box<dyn Error>> {
    if input.chips_per_bb != 1 || input.max_iterations > 20_000 {
        return Err("reference capture requires chip units and at most 20,000 iterations".into());
    }
    let started = Instant::now();
    let board: [Card; 5] = input.board.iter().map(|c| c.parse()).collect::<Result<Vec<_>, _>>()?
        .try_into().map_err(|_| "invalid board length")?;
    let ranges = [Range::parse(&input.ranges[0])?, Range::parse(&input.ranges[1])?];
    let tree = RiverTree::new(RiverTreeConfig {
        starting_pot: input.starting_pot, effective_stack: input.effective_stack,
        min_bet: input.min_bet, sizes: [
            BetSizeOptions::try_from((input.bets[0].as_str(), input.raises[0].as_str()))?,
            BetSizeOptions::try_from((input.bets[1].as_str(), input.raises[1].as_str()))?,
        ], max_raises: input.max_raises,
        add_all_in_threshold: input.add_all_in_threshold as f64,
        force_all_in_threshold: input.force_all_in_threshold as f64,
        max_nodes: 10_000,
    })?;
    let game = RiverGame::new(board, ranges, tree, 512 * 1024 * 1024)?;
    let mut solver = RiverSolver::new(game.clone(), Variant::Discounted { alpha: 1.5, beta: 0.0, gamma: 2.0 })?;
    let mut checkpoints = Vec::new();
    let report = solver.solve(&SolveConfig {
        target_pct_of_pot: input.target_pct_of_pot, max_iterations: input.max_iterations,
        check_every: input.check_every, log_every_secs: 30, threads: 1,
    }, |progress| {
        checkpoints.push(Checkpoint { iterations: progress.iterations,
            pct_of_pot: progress.exploitability.pct_of_pot,
            elapsed_seconds: started.elapsed().as_secs_f64() });
        eprintln!("{} iteration={} pct_of_pot={}", input.id, progress.iterations, progress.exploitability.pct_of_pot);
    })?;
    let strategy = solver.average_strategy()?;
    let ev = [strategy.expected_value(0)?, strategy.expected_value(1)?];
    let mut nodes = Vec::new();
    let mut pending = vec![(game.tree().root(), Vec::new())];
    while let Some((id, history_labels)) = pending.pop() {
        let node = game.tree().node(id).ok_or("missing tree node")?;
        let (kind, player, terminal, fold_winner) = match node.kind() {
            RiverNodeKind::Decision { player } => ("decision", player as i8, "", -1),
            RiverNodeKind::Terminal(Terminal::Showdown) => ("terminal", -1, "showdown", -1),
            RiverNodeKind::Terminal(Terminal::Fold { winner }) => ("terminal", -1, "fold", winner as i8),
        };
        let mut hands = Vec::new();
        if player >= 0 {
            let decision = strategy.decision_values(id)?;
            for combo in Combo::all() {
                let Some(row) = strategy.row(id, combo) else { continue; };
                let h = usize::from(combo.id());
                let values = &decision.values()[h * row.len()..(h + 1) * row.len()];
                let available = values.iter().all(Option::is_some);
                if !available && values.iter().any(Option::is_some) { return Err("partially available action EV".into()); }
                hands.push(Hand { cards: combo.cards().map(|c| c.to_string()), strategy: row.to_vec(),
                    action_expected_values: values.iter().flatten().copied().collect(), ev_available: available,
                    own_reach: decision.own_reach()[h], opponent_mass: decision.opponent_mass()[h] });
            }
        }
        for (child, action) in node.children().iter().zip(node.actions()).rev() {
            let mut next = history_labels.clone();
            next.push(action.to_string());
            pending.push((*child, next));
        }
        nodes.push(Node { history_labels, kind: kind.into(), player,
            actions: node.actions().iter().map(ToString::to_string).collect(),
            contributions: node.contributions(), terminal: terminal.into(), fold_winner, hands });
    }
    let output = Capture { input, iterations: report.iterations,
        stop_reason: format!("{:?}", report.stop_reason), exploitability_pct_of_pot: report.exploitability.pct_of_pot,
        root_centered_expected_values: ev, best_response_values: report.exploitability.br_value,
        compatible_weight: game.compatible_weight(), nodes, checkpoints,
        working_set_bound_bytes: game.memory_usage().working_set_bound_bytes,
        reserved_bytes: game.reserved_bytes(), elapsed_seconds: started.elapsed().as_secs_f64() };
    if output.exploitability_pct_of_pot >= 0.5 { return Err(format!("{} failed the 0.5% gate", output.input.id).into()); }
    Ok(output)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 { return Err("expected input TOML and output TOML paths".into()); }
    if fs::metadata(&args[0])?.len() > 65_536 { return Err("input exceeds 64 KiB".into()); }
    let input: Inputs = toml::from_str(&fs::read_to_string(&args[0])?)?;
    if input.schema_version != 1 || input.cases.is_empty() || input.cases.len() > 8 { return Err("invalid input schema".into()); }
    let output = Output { schema_version: 1, os: std::env::consts::OS.into(), architecture: std::env::consts::ARCH.into(),
        cases: input.cases.into_iter().map(capture).collect::<Result<_, _>>()? };
    let serialized = toml::to_string(&output)?;
    if serialized.len() > 64 * 1024 * 1024 { return Err("output exceeds 64 MiB".into()); }
    use std::io::Write;
    fs::OpenOptions::new().write(true).create_new(true).open(&args[1])?.write_all(serialized.as_bytes())?;
    Ok(())
}
