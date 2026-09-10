//! Capture the measured turn gate policies for the joint reference comparison.
//!
//! Reads `tests/reference/turn/cases.json`, the same input the independent WASM
//! reference reads, solves each case in f64 to its configured target, and writes
//! one TOML document: the exported policies, the convergence record, the
//! repository revision, the host this ran on, and three separately measured
//! timings. `tests/reference/turn/compare.py --project --reference --review` is
//! what reads the result; `tests/reference/turn/README.md` documents the fields.
//!
//! Only the runouts each case names in `export_runouts` are exported, because
//! that is all the reference exports. Both sides still solve every runout.
//!
//! The example reads values and never recomputes them: every frequency and every
//! action EV in the output comes from `PostflopStrategy`, and the only arithmetic
//! here is over clocks and byte counts.

use cards::{Card, Combo, Range};
use postflop::{
    Exploitability, PostflopGame, PostflopOptions, PostflopSolver, PostflopStrategy, SolveConfig,
    SolveError, SolverConfig, Variant,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    path::Path,
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tree::{BetSizeOptions, PostflopNodeKind, PostflopTree, PostflopTreeConfig, Street, Terminal};

/// Default solver configuration: the memory limit, the storage width, the worker
/// count and the progress interval are all configuration, never compiled in.
const DEFAULT_CONFIG: &str = "config/solver.toml";
/// Compact-tree node budget. The gate turn tree needs a few hundred compact
/// nodes; this is the "something is very wrong" ceiling, not a tuning knob.
const MAX_COMPACT_NODES: usize = 100_000;
/// Iterations timed one at a time before the driver takes over. They are the
/// first iterations of the solve itself, so nothing is spent twice.
const TIMED_ITERATIONS: u64 = 10;
/// Iterations a cancellation probe completes before the flag is set.
const CANCEL_AFTER_POLLS: u64 = 5;
/// How long the watcher waits after the probe's last observed poll, so that the
/// flag lands inside an iteration rather than at a loop top.
const CANCEL_DELAY: Duration = Duration::from_millis(50);
/// Headroom above `CANCEL_AFTER_POLLS` for a probe, so a fast iteration cannot
/// race the watcher to the cap. A probe that stops for any other reason fails.
const CANCEL_HEADROOM: u64 = 256;
/// Refuse to write more than the 64 MiB `compare.py` is willing to read.
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

// --- the case file --------------------------------------------------------------------
//
// These mirror `tests/reference/turn/cases.json` field for field, because
// `compare.py` requires the project's `input` table and the reference's `input`
// object to be equal. `deny_unknown_fields` is what makes that a checked claim
// rather than a hope: a field added to the case file fails here instead of
// silently dropping out of the comparison.

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StreetMenu {
    oop_bet: String,
    oop_raise: String,
    /// Absent on the flop menu, present and empty on the turn and river ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oop_donk: Option<String>,
    ip_bet: String,
    ip_raise: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Menus {
    flop: StreetMenu,
    turn: StreetMenu,
    river: StreetMenu,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    street: String,
    board: [String; 4],
    ranges: [String; 2],
    range_labels: [String; 2],
    chips_per_bb: u64,
    starting_pot: u64,
    effective_stack: u64,
    donk_option: bool,
    export_runouts: Vec<String>,
    min_bet: u64,
    max_raises: u8,
    add_all_in_threshold: u64,
    force_all_in_threshold: u64,
    target_pct_of_pot: f64,
    max_iterations: u64,
    check_every: u64,
    // Last: TOML gives every bare key that follows a table to that table.
    menus: Menus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    schema_version: u64,
    street: String,
    ranges_provenance: String,
    cases: Vec<Case>,
}

// --- the capture ----------------------------------------------------------------------

/// One private hand's row at an exported node.
///
/// A decision node reports what the actor's policy does with the hand and what
/// each action is worth to it. A node where nobody acts has no action to report,
/// so it reports the value of the history itself, once per player: that is what
/// `tests/reference/turn/oracle.py` reads where its walk stops at a chance node
/// it cannot cross, and it is why `PostflopStrategy::node_values` exists.
///
/// The reported row carries only what the oracle reads. The reach that says how
/// often a history happens is on every decision row already, and this file is
/// within a few percent of the 64 MiB ceiling `compare.py` will read: a chance
/// node's row set spans both players' whole ranges, so each field costs about a
/// megabyte across the three cases.
#[derive(Serialize)]
#[serde(untagged)]
enum Hand {
    Decision {
        cards: [String; 2],
        strategy: Vec<f64>,
        action_expected_values: Vec<f64>,
        ev_available: bool,
        own_reach: f64,
        opponent_mass: f64,
    },
    Reported {
        player: u8,
        cards: [String; 2],
        /// Absent, never zero, when the hand has no value here: the key is
        /// missing exactly when `ev_available` is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_value: Option<f64>,
        ev_available: bool,
    },
}

#[derive(Serialize)]
struct Node {
    history_labels: Vec<String>,
    kind: String,
    street: String,
    /// The dealt card at a river history, empty on the turn round. TOML has no
    /// null; `compare.py` reads an empty string and the reference's `null` alike.
    runout: String,
    player: i8,
    actions: Vec<String>,
    contributions: [u64; 2],
    terminal: String,
    fold_winner: i8,
    possible_cards: Vec<String>,
    isomorphic_merged_cards: usize,
    hands: Vec<Hand>,
}

#[derive(Serialize)]
struct Checkpoint {
    iterations: u64,
    pct_of_pot: f64,
    elapsed_seconds: f64,
}

/// One cancelled run, timed from the flag to the driver's return.
#[derive(Serialize)]
struct CancelProbe {
    requested_after_observed_polls: u64,
    iterations: u64,
    stop_reason: String,
    latency_seconds: f64,
}

#[derive(Serialize)]
struct Timings {
    /// Iterations timed individually, and what they averaged. These are the
    /// first `TIMED_ITERATIONS` iterations of the solve, timed one by one before
    /// the driver continues from the same solver, so the mean carries no
    /// measurement time and costs no extra work.
    timed_iterations: u64,
    mean_iteration_seconds: f64,
    min_iteration_seconds: f64,
    max_iteration_seconds: f64,
    /// A best-response measurement over the finished average, on the snapshot
    /// path a caller outside the solver uses. `PostflopStrategy::exploitability`
    /// walks the tree on one thread whatever the worker count is; the driver's
    /// own in-solve measurement uses the parallel walk instead, so the
    /// cancellation numbers below, not this one, say what a cancel costs.
    average_strategy_snapshot_seconds: f64,
    best_response_measurement_seconds: f64,
    /// Cancellation, measured twice on purpose. `mid_iteration` sets the flag
    /// from a watcher thread while an iteration is in flight, which is what an
    /// app does; `at_poll` sets it from the poll itself at a loop top. Cancel
    /// runs no best-response measurement, so `at_poll` is the driver's return
    /// alone and the difference between the two is the iteration the
    /// mid-iteration cancel had to wait out.
    cancel_latency_seconds: f64,
    cancel_return_seconds: f64,
    cancel_iteration_remainder_seconds: f64,
    cancel_mid_iteration: CancelProbe,
    cancel_at_poll: CancelProbe,
}

#[derive(Serialize)]
struct Capture {
    iterations: u64,
    stop_reason: String,
    reached_target: bool,
    exploitability_pct_of_pot: f64,
    exploitability_chips: f64,
    root_centered_expected_values: [f64; 2],
    best_response_values: [f64; 2],
    compatible_weight: f64,
    working_set_bound_bytes: usize,
    reserved_bytes: usize,
    elapsed_seconds: f64,
    exported_nodes: usize,
    workers: usize,
    input: Case,
    timings: Timings,
    checkpoints: Vec<Checkpoint>,
    nodes: Vec<Node>,
}

#[derive(Serialize)]
struct Host {
    os: String,
    architecture: String,
    /// Digits, or the literal `unknown`: a host this cannot read is recorded as
    /// unread rather than failing a solve that is otherwise fine.
    physical_memory_bytes: String,
    physical_memory_source: String,
    logical_cpus: String,
    logical_cpus_source: String,
}

#[derive(Serialize)]
struct Output {
    schema_version: u64,
    street: String,
    project_revision: String,
    project_revision_source: String,
    execution_stop_policy: String,
    os: String,
    architecture: String,
    config_path: String,
    solver_variant: String,
    requested_threads: usize,
    resolved_workers: usize,
    /// `config/solver.toml`'s `log_every_secs`, which is what the driver was
    /// given. It decides how often the job log says where the solve has got to,
    /// and nothing else: measurements, and so the iteration a solve stops on,
    /// follow the case's own `check_every`.
    progress_interval_seconds: u64,
    ranges_provenance: String,
    host: Host,
    cases: Vec<Capture>,
}

// --- host and revision ----------------------------------------------------------------

/// Total physical memory in bytes, and where the number came from.
///
/// Linux answers from `/proc/meminfo`; Windows asks CIM through PowerShell,
/// because `wmic` is no longer installed by default. Anything else, or a query
/// that fails, is `unknown`: the capture records what it could read.
fn physical_memory() -> (String, String) {
    if let Ok(text) = fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("MemTotal:") else {
                continue;
            };
            if let Some(kib) = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok())
            {
                return (
                    kib.saturating_mul(1024).to_string(),
                    "/proc/meminfo MemTotal".into(),
                );
            }
        }
    }
    if cfg!(windows) {
        let query = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .output();
        if let Ok(result) = query
            && result.status.success()
        {
            let text = String::from_utf8_lossy(&result.stdout);
            let digits = text.trim();
            if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return (
                    digits.to_string(),
                    "Win32_ComputerSystem.TotalPhysicalMemory".into(),
                );
            }
        }
    }
    ("unknown".into(), "unavailable".into())
}

/// Cores the platform reports, and where the number came from.
fn logical_cpus() -> (String, String) {
    match thread::available_parallelism() {
        Ok(count) => (count.get().to_string(), "available_parallelism".into()),
        Err(_) => ("unknown".into(), "unavailable".into()),
    }
}

/// The commit this capture measures. `compare.py --expected-revision` refuses a
/// capture whose revision is not the workflow's own commit, so a stale artifact
/// cannot be read as evidence for a newer tree.
fn project_revision() -> (String, String) {
    if let Ok(sha) = std::env::var("GITHUB_SHA")
        && !sha.trim().is_empty()
    {
        return (sha.trim().to_string(), "GITHUB_SHA".into());
    }
    if let Ok(result) = Command::new("git").args(["rev-parse", "HEAD"]).output()
        && result.status.success()
    {
        let text = String::from_utf8_lossy(&result.stdout);
        let sha = text.trim();
        if !sha.is_empty() {
            return (sha.to_string(), "git rev-parse HEAD".into());
        }
    }
    ("unknown".into(), "unavailable".into())
}

// --- the solve ------------------------------------------------------------------------

fn parse_board(labels: &[String; 4]) -> Result<Vec<Card>, Box<dyn Error>> {
    let mut board = Vec::with_capacity(labels.len());
    for label in labels {
        board.push(label.parse::<Card>()?);
    }
    Ok(board)
}

/// The tree Decision 10 describes: one raise per street, the case's own menus,
/// and no menu of our own invention. The flop entry is never read by a turn
/// tree, and the case file leaves it empty, so an empty menu is what it gets.
fn build_tree(case: &Case) -> Result<PostflopTree, Box<dyn Error>> {
    let menu = |entry: &StreetMenu| -> Result<[BetSizeOptions; 2], Box<dyn Error>> {
        if !entry.oop_donk.as_deref().unwrap_or("").is_empty() {
            return Err("this capture has no donk menu; the case file sets one".into());
        }
        Ok([
            BetSizeOptions::try_from((entry.oop_bet.as_str(), entry.oop_raise.as_str()))?,
            BetSizeOptions::try_from((entry.ip_bet.as_str(), entry.ip_raise.as_str()))?,
        ])
    };
    Ok(PostflopTree::new(PostflopTreeConfig {
        starting_pot: case.starting_pot,
        effective_stack: case.effective_stack,
        min_bet: case.min_bet,
        start_street: Street::Turn,
        sizes: [
            menu(&case.menus.flop)?,
            menu(&case.menus.turn)?,
            menu(&case.menus.river)?,
        ],
        max_raises: case.max_raises,
        add_all_in_threshold: case.add_all_in_threshold as f64,
        force_all_in_threshold: case.force_all_in_threshold as f64,
        max_nodes: MAX_COMPACT_NODES,
    })?)
}

/// One cancelled run, timed from the moment the flag was set to the moment
/// `solve_with_cancel` returned.
///
/// `mid_iteration` decides what the number covers. With it a watcher thread sets
/// the flag while an iteration is in flight, so the latency holds the rest of
/// that iteration and the return. Without it the poll sets the flag itself at a
/// loop top, so the same latency holds only the return. Neither holds a
/// best-response measurement: the driver takes none on a cancel.
fn cancel_probe(
    game: &PostflopGame,
    variant: Variant,
    threads: usize,
    log_every_secs: u64,
    mid_iteration: bool,
) -> Result<CancelProbe, Box<dyn Error>> {
    let mut solver = PostflopSolver::new(game.clone(), variant)?;
    let config = SolveConfig {
        // Unreachable, so the probe can only stop on the cancel or the cap.
        target_pct_of_pot: 0.0,
        max_iterations: CANCEL_AFTER_POLLS + CANCEL_HEADROOM,
        // No interim measurement: the probe times cancellation, not convergence.
        check_every: u64::MAX,
        log_every_secs,
        threads,
    };
    let polls = AtomicU64::new(0);
    let cancel = AtomicBool::new(false);
    let finished = AtomicBool::new(false);
    let requested: Mutex<Option<Instant>> = Mutex::new(None);
    let observed = AtomicU64::new(0);

    let (report, returned) = thread::scope(
        |scope| -> Result<(Result<postflop::SolveReport, SolveError>, Instant), Box<dyn Error>> {
            if mid_iteration {
                scope.spawn(|| {
                    while polls.load(Ordering::Acquire) < CANCEL_AFTER_POLLS
                        && !finished.load(Ordering::Acquire)
                    {
                        thread::sleep(Duration::from_millis(1));
                    }
                    if finished.load(Ordering::Acquire) {
                        return;
                    }
                    // Land inside the iteration the solver has just started.
                    thread::sleep(CANCEL_DELAY);
                    observed.store(polls.load(Ordering::Acquire), Ordering::Release);
                    *requested.lock().expect("cancel clock") = Some(Instant::now());
                    cancel.store(true, Ordering::Release);
                });
            }
            let outcome = solver.solve_with_cancel(
                &config,
                |_| {},
                || {
                    let seen = polls.fetch_add(1, Ordering::AcqRel) + 1;
                    if !mid_iteration && seen > CANCEL_AFTER_POLLS {
                        let mut clock = requested.lock().expect("cancel clock");
                        if clock.is_none() {
                            observed.store(seen, Ordering::Release);
                            *clock = Some(Instant::now());
                        }
                        return true;
                    }
                    cancel.load(Ordering::Acquire)
                },
            );
            let returned = Instant::now();
            finished.store(true, Ordering::Release);
            Ok((outcome, returned))
        },
    )?;

    let report = report?;
    let requested = requested
        .lock()
        .expect("cancel clock")
        .ok_or("the cancellation probe finished before its flag was set")?;
    if !matches!(report.stop_reason, postflop::StopReason::Cancelled) {
        return Err(format!(
            "the cancellation probe stopped on {:?}, not on the cancel",
            report.stop_reason
        )
        .into());
    }
    Ok(CancelProbe {
        requested_after_observed_polls: observed.load(Ordering::Acquire),
        iterations: report.iterations,
        stop_reason: format!("{:?}", report.stop_reason),
        latency_seconds: returned.saturating_duration_since(requested).as_secs_f64(),
    })
}

/// Per-hand values at a node where nobody acts, for both players.
///
/// One row per hand that carries weight in that player's range and is not
/// blocked by a board card here, whether or not the policy ever brings it here:
/// a walker that stops at this node arrives down branches the hand takes with
/// probability zero and multiplies by that probability itself, so it needs the
/// value there too. A hand with no compatible opponent hand left has no value,
/// and its row says so rather than reporting a zero.
fn reported_hands(
    game: &PostflopGame,
    strategy: &PostflopStrategy,
    id: postflop::NodeId,
) -> Result<Vec<Hand>, Box<dyn Error>> {
    let values = strategy.node_values(id)?;
    let board = values
        .board()
        .iter()
        .fold(0_u64, |mask, card| mask | card.mask());
    let mut hands = Vec::new();
    for player in 0..2 {
        let weights = game
            .initial_weights(player)
            .ok_or("a postflop game reported no initial weights")?;
        for combo in Combo::all() {
            let state = usize::from(combo.id());
            if weights[state] <= 0.0 || combo.mask() & board != 0 {
                continue;
            }
            let value = values.values(player)[state];
            hands.push(Hand::Reported {
                player: player as u8,
                cards: combo.cards().map(|card| card.to_string()),
                expected_value: value,
                ev_available: value.is_some(),
            });
        }
    }
    Ok(hands)
}

/// Walk the exported public histories and read every live row off the average.
///
/// Only the runouts the case names are descended into, which is exactly what the
/// reference exports; every betting branch is exported in full.
fn export_nodes(
    game: &PostflopGame,
    strategy: &PostflopStrategy,
    case: &Case,
) -> Result<Vec<Node>, Box<dyn Error>> {
    let mut nodes = Vec::new();
    let mut exported_runouts = 0usize;
    let mut pending = vec![(game.root(), Vec::<String>::new())];
    while let Some((id, history_labels)) = pending.pop() {
        let view = game.node(id).ok_or("missing expanded node")?;
        let runout = match view.runout() {
            [] => String::new(),
            [card] => card.to_string(),
            _ => return Err("a turn tree dealt more than one card".into()),
        };
        let actions: Vec<String> = view.actions().iter().map(ToString::to_string).collect();
        let mut possible_cards = Vec::new();
        let mut hands = Vec::new();
        let (kind, player, terminal, fold_winner) = match view.kind() {
            PostflopNodeKind::Decision { player } => ("decision", player as i8, "", -1),
            PostflopNodeKind::Chance { .. } => ("chance", -1, "", -1),
            PostflopNodeKind::Terminal(Terminal::Showdown) => ("terminal", -1, "showdown", -1),
            PostflopNodeKind::Terminal(Terminal::Fold { winner }) => {
                ("terminal", -1, "fold", winner as i8)
            }
        };
        match view.kind() {
            PostflopNodeKind::Chance { .. } => {
                let cards = view.possible_cards();
                let children = view.children();
                if cards.len() != children.len() {
                    return Err("a chance node's cards and children disagree".into());
                }
                possible_cards = cards.iter().map(ToString::to_string).collect();
                for (index, label) in possible_cards.iter().enumerate().rev() {
                    if !case.export_runouts.contains(label) {
                        continue;
                    }
                    exported_runouts += 1;
                    let mut next = history_labels.clone();
                    next.push(format!("chance:{label}"));
                    pending.push((children[index], next));
                }
                hands = reported_hands(game, strategy, id)?;
            }
            PostflopNodeKind::Decision { .. } => {
                let decision = strategy.decision_values(id)?;
                for combo in Combo::all() {
                    let Some(row) = strategy.row(id, combo) else {
                        continue;
                    };
                    let state = usize::from(combo.id());
                    let values = &decision.values()[state * row.len()..(state + 1) * row.len()];
                    let available = values.iter().all(Option::is_some);
                    if !available && values.iter().any(Option::is_some) {
                        return Err("a decision row reported some but not all action EVs".into());
                    }
                    hands.push(Hand::Decision {
                        cards: combo.cards().map(|card| card.to_string()),
                        strategy: row.to_vec(),
                        action_expected_values: values.iter().flatten().copied().collect(),
                        ev_available: available,
                        own_reach: decision.own_reach()[state],
                        opponent_mass: decision.opponent_mass()[state],
                    });
                }
                for (child, action) in view.children().iter().zip(view.actions()).rev() {
                    let mut next = history_labels.clone();
                    next.push(action.to_string());
                    pending.push((*child, next));
                }
            }
            // A showdown still on the turn is the other place a walk that
            // cannot cross a deal has to stop. Our tree deals the river after a
            // called all-in rather than ending there, so these exist only if a
            // later tree change makes them; the export does not depend on that.
            PostflopNodeKind::Terminal(Terminal::Showdown) if view.street() == Street::Turn => {
                hands = reported_hands(game, strategy, id)?;
            }
            PostflopNodeKind::Terminal(_) => {}
        }
        nodes.push(Node {
            history_labels,
            kind: kind.into(),
            street: view.street().to_string(),
            runout,
            player,
            actions,
            contributions: view.contributions(),
            terminal: terminal.into(),
            fold_winner,
            possible_cards,
            // Nothing is merged yet: step 9 of docs/phase-4/PLAN.md is what
            // makes this anything but zero, and `compare.py` reports both sides.
            isomorphic_merged_cards: 0,
            hands,
        });
    }
    let chance_nodes = nodes.iter().filter(|node| node.kind == "chance").count();
    let expected = chance_nodes * case.export_runouts.len();
    if exported_runouts != expected {
        return Err(format!(
            "{}: exported {exported_runouts} runout branches across {chance_nodes} chance nodes, \
             expected {expected}; a named runout is not dealable here",
            case.id
        )
        .into());
    }
    Ok(nodes)
}

fn capture(
    case: Case,
    config: &SolverConfig,
    options: PostflopOptions,
) -> Result<Capture, Box<dyn Error>> {
    if case.street != "turn" {
        return Err(format!("{}: not a turn case", case.id).into());
    }
    let started = Instant::now();
    let board = parse_board(&case.board)?;
    let ranges = [
        Range::parse(&case.ranges[0])?,
        Range::parse(&case.ranges[1])?,
    ];
    let tree = build_tree(&case)?;
    let game = PostflopGame::new(&board, ranges, tree, options)?;
    let variant = config.dcfr.variant();
    let solve = SolveConfig {
        target_pct_of_pot: case.target_pct_of_pot,
        max_iterations: case.max_iterations,
        check_every: case.check_every,
        log_every_secs: config.solve.log_every_secs,
        threads: config.solve.threads,
    };
    solve.validate()?;
    if case.max_iterations <= TIMED_ITERATIONS {
        return Err(format!("{}: iteration cap is below the timing window", case.id).into());
    }

    let (report, workers, timings, checkpoints, nodes, ev) = {
        let mut solver = PostflopSolver::new(game.clone(), variant)?;
        let workers = solver.workers();
        eprintln!(
            "{} start workers={workers} nodes={} working_set_bound_bytes={}",
            case.id,
            game.num_nodes(),
            game.memory_usage().working_set_bound_bytes
        );

        // The first iterations of the solve, timed one at a time. The driver
        // continues from this same solver, so none of this work is repeated.
        let mut iteration_seconds = Vec::with_capacity(TIMED_ITERATIONS as usize);
        for _ in 0..TIMED_ITERATIONS {
            let clock = Instant::now();
            solver.run_iteration()?;
            iteration_seconds.push(clock.elapsed().as_secs_f64());
        }

        let mut checkpoints = Vec::new();
        // A checkpoint is a measurement, so only a fresh one becomes one. The
        // driver also emits an event every `log_every_secs`, repeating the last
        // measurement with `stale` set; those go to the job log, where they say
        // the solve is alive, and stay out of the capture, where they would make
        // the recorded schedule depend on how fast the host ran.
        let report = solver.solve(&solve, |progress| {
            let Some(measurement) = progress.exploitability else {
                eprintln!(
                    "{} {} iteration={} measured=none elapsed_seconds={}",
                    progress.timestamp,
                    case.id,
                    progress.iterations,
                    started.elapsed().as_secs_f64()
                );
                return;
            };
            if !progress.stale {
                checkpoints.push(Checkpoint {
                    iterations: progress.iterations,
                    pct_of_pot: measurement.pct_of_pot,
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                });
            }
            eprintln!(
                "{} {} iteration={} stale={} pct_of_pot={} elapsed_seconds={}",
                progress.timestamp,
                case.id,
                progress.iterations,
                progress.stale,
                measurement.pct_of_pot,
                started.elapsed().as_secs_f64()
            );
        })?;
        let driver_measurement = report.measured()?;

        let clock = Instant::now();
        let strategy = solver.average_strategy()?;
        let snapshot_seconds = clock.elapsed().as_secs_f64();
        let clock = Instant::now();
        let measured = strategy.exploitability()?;
        let best_response_seconds = clock.elapsed().as_secs_f64();
        if measured.pct_of_pot != driver_measurement.pct_of_pot {
            eprintln!(
                "{} snapshot measurement {} differs from the driver's {}",
                case.id, measured.pct_of_pot, driver_measurement.pct_of_pot
            );
        }
        let ev = [strategy.expected_value(0)?, strategy.expected_value(1)?];
        let nodes = export_nodes(&game, &strategy, &case)?;
        drop(strategy);

        // Both probes need the memory this solver holds, so it goes first.
        drop(solver);
        let log_every_secs = config.solve.log_every_secs;
        let mid = cancel_probe(&game, variant, config.solve.threads, log_every_secs, true)?;
        let at_poll = cancel_probe(&game, variant, config.solve.threads, log_every_secs, false)?;
        let total: f64 = iteration_seconds.iter().sum();
        let timings = Timings {
            timed_iterations: TIMED_ITERATIONS,
            mean_iteration_seconds: total / TIMED_ITERATIONS as f64,
            min_iteration_seconds: iteration_seconds.iter().copied().fold(f64::MAX, f64::min),
            max_iteration_seconds: iteration_seconds.iter().copied().fold(f64::MIN, f64::max),
            average_strategy_snapshot_seconds: snapshot_seconds,
            best_response_measurement_seconds: best_response_seconds,
            cancel_latency_seconds: mid.latency_seconds,
            cancel_return_seconds: at_poll.latency_seconds,
            cancel_iteration_remainder_seconds: mid.latency_seconds - at_poll.latency_seconds,
            cancel_mid_iteration: mid,
            cancel_at_poll: at_poll,
        };
        (report, workers, timings, checkpoints, nodes, ev)
    };

    let Exploitability {
        br_value,
        average,
        pct_of_pot,
        ..
    } = report.measured()?;
    Ok(Capture {
        iterations: report.iterations,
        stop_reason: format!("{:?}", report.stop_reason),
        reached_target: pct_of_pot <= case.target_pct_of_pot,
        exploitability_pct_of_pot: pct_of_pot,
        exploitability_chips: average,
        root_centered_expected_values: ev,
        best_response_values: br_value,
        compatible_weight: game.compatible_weight(),
        working_set_bound_bytes: game.memory_usage().working_set_bound_bytes,
        reserved_bytes: game.reserved_bytes(),
        elapsed_seconds: started.elapsed().as_secs_f64(),
        exported_nodes: nodes.len(),
        workers,
        input: case,
        timings,
        checkpoints,
        nodes,
    })
}

fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut positional = Vec::new();
    let mut config_path = DEFAULT_CONFIG.to_string();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                config_path = args.get(index + 1).ok_or("--config needs a path")?.clone();
                index += 1;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option {other}").into());
            }
            other => positional.push(other.to_string()),
        }
        index += 1;
    }
    if positional.len() != 2 {
        return Err("expected the case JSON and the output TOML, with an optional --config".into());
    }
    let (input_path, output_path) = (Path::new(&positional[0]), Path::new(&positional[1]));
    if fs::metadata(input_path)?.len() > 65_536 {
        return Err("the case file exceeds 64 KiB".into());
    }
    let inputs: Inputs = serde_json::from_str(&fs::read_to_string(input_path)?)?;
    if inputs.schema_version != 1 || inputs.street != "turn" {
        return Err("unknown case schema".into());
    }
    if inputs.cases.is_empty() || inputs.cases.len() > 8 {
        return Err("the case file must hold between one and eight cases".into());
    }

    let config = SolverConfig::load(&config_path)?;
    let options = PostflopOptions::from_config(&config)?;
    let (physical_memory_bytes, physical_memory_source) = physical_memory();
    let (logical_cpus, logical_cpus_source) = logical_cpus();
    let (project_revision, project_revision_source) = project_revision();
    eprintln!(
        "turn capture revision={project_revision} ({project_revision_source}) os={} cpus={} \
         memory_bytes={}",
        std::env::consts::OS,
        logical_cpus,
        physical_memory_bytes
    );

    let mut cases = Vec::new();
    let mut resolved_workers = 0;
    for case in inputs.cases {
        let measured = capture(case, &config, options)?;
        resolved_workers = measured.workers;
        eprintln!(
            "{} done iterations={} stop_reason={} pct_of_pot={} elapsed_seconds={}",
            measured.input.id,
            measured.iterations,
            measured.stop_reason,
            measured.exploitability_pct_of_pot,
            measured.elapsed_seconds
        );
        cases.push(measured);
    }

    let output = Output {
        schema_version: 1,
        street: "turn".into(),
        project_revision,
        project_revision_source,
        execution_stop_policy: "target_or_cap".into(),
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        config_path,
        solver_variant: format!("{:?}", config.dcfr.variant()),
        requested_threads: config.solve.threads,
        resolved_workers,
        progress_interval_seconds: config.solve.log_every_secs,
        ranges_provenance: inputs.ranges_provenance,
        host: Host {
            os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            physical_memory_bytes,
            physical_memory_source,
            logical_cpus,
            logical_cpus_source,
        },
        cases,
    };
    let serialized = toml::to_string(&output)?;
    if serialized.len() > MAX_OUTPUT_BYTES {
        return Err(format!(
            "the capture is {} bytes, above the 64 MiB the comparison reads",
            serialized.len()
        )
        .into());
    }
    use std::io::Write;
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?
        .write_all(serialized.as_bytes())?;

    // The capture is on disk before the gate is judged, so a failing solve is
    // still uploaded and can be read rather than guessed at.
    let missed: Vec<&Capture> = output.cases.iter().filter(|c| !c.reached_target).collect();
    if !missed.is_empty() {
        let detail: Vec<String> = missed
            .iter()
            .map(|c| {
                format!(
                    "{} stopped at {}% of pot after {} iterations ({}), target {}%",
                    c.input.id,
                    c.exploitability_pct_of_pot,
                    c.iterations,
                    c.stop_reason,
                    c.input.target_pct_of_pot
                )
            })
            .collect();
        return Err(format!("the turn gate was not met: {}", detail.join("; ")).into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}
