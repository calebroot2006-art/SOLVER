//! Bounded solve driver with explicit accuracy and stopping reasons.
use crate::{Exploitability, Game, Progress, SolveConfig, SolveError, Strategy, exploitability};
use std::time::{Duration, Instant};

/// Algorithm operations used by the bounded driver.
pub trait Solver {
    /// Number of completely finished alternating iterations.
    fn iteration(&self) -> u64;
    /// Advances exactly one iteration, or fails without certifying a strategy.
    fn run_iteration(&mut self, game: &dyn Game) -> Result<(), SolveError>;
    /// Produces a checked average for this immutable game.
    fn average_strategy(&self, game: &dyn Game) -> Result<Strategy, SolveError>;
}

/// Why a successful driver invocation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// Measured exploitability reached the configured target.
    TargetReached,
    /// The iteration cap was reached with exploitability above target.
    IterationCap,
    /// Cancellation requested between complete alternating iterations.
    Cancelled,
}

/// Final measurement for a successful driver invocation.
#[derive(Clone, Debug)]
pub struct SolveReport {
    /// Total completed iterations, including any run before this invocation.
    pub iterations: u64,
    /// Final measured accuracy within this game's tree.
    pub exploitability: Exploitability,
    /// Wall time spent in this driver invocation.
    pub elapsed: Duration,
    /// Target, iteration cap, or cancellation, explicitly distinguished.
    pub stop_reason: StopReason,
}

/// Runs to a measured target or total iteration cap.
///
/// Progress is measured after `check_every` iterations, when the wall-clock
/// interval expires, and on the final iteration. Checks occur between iterations;
/// an individual slow iteration can exceed the requested logging interval.
/// The callback receives only complete measurements. Phase 1 is serial.
pub fn solve(
    game: &dyn Game,
    solver: &mut dyn Solver,
    cfg: &SolveConfig,
    on_progress: impl FnMut(&Progress),
) -> Result<SolveReport, SolveError> {
    drive(&mut LegacySession { game, solver }, cfg, on_progress, || false)
}

pub(crate) trait SolveSession {
    fn iteration(&self) -> u64;
    fn step(&mut self) -> Result<(), SolveError>;
    fn measurement(&mut self) -> Result<Exploitability, SolveError>;
}

struct LegacySession<'a> {
    game: &'a dyn Game,
    solver: &'a mut dyn Solver,
}

impl SolveSession for LegacySession<'_> {
    fn iteration(&self) -> u64 { self.solver.iteration() }
    fn step(&mut self) -> Result<(), SolveError> { self.solver.run_iteration(self.game) }
    fn measurement(&mut self) -> Result<Exploitability, SolveError> {
        let strategy = self.solver.average_strategy(self.game)?;
        exploitability(self.game, &strategy)
    }
}

pub(crate) fn drive(
    session: &mut dyn SolveSession,
    cfg: &SolveConfig,
    mut on_progress: impl FnMut(&Progress),
    mut should_cancel: impl FnMut() -> bool,
) -> Result<SolveReport, SolveError> {
    cfg.validate()?;
    let start = Instant::now();
    let mut last_progress = start;
    loop {
        let cancelled = should_cancel();
        let before = session.iteration();
        if before < cfg.max_iterations && !cancelled {
            session.step()?;
            if session.iteration() != before + 1 {
                return Err(SolveError::InvalidGame(
                    "solver did not advance exactly one iteration".into(),
                ));
            }
        }
        let iterations = session.iteration();
        let at_cap = iterations >= cfg.max_iterations;
        let timed = last_progress.elapsed() >= Duration::from_secs(cfg.log_every_secs);
        if cancelled || at_cap || iterations.is_multiple_of(cfg.check_every) || timed {
            let measurement = session.measurement().map_err(|error| match error {
                SolveError::NonFinite { node, player, .. } => SolveError::NonFinite {
                    iteration: iterations,
                    node,
                    player,
                },
                SolveError::Terminal { node, player, reason, .. } => SolveError::Terminal {
                    iteration: iterations, node, player, reason,
                },
                SolveError::Arithmetic { node, player, reason, .. } => SolveError::Arithmetic {
                    iteration: iterations, node, player, reason,
                },
                other => other,
            })?;
            let progress = Progress::record(iterations, measurement, start.elapsed());
            on_progress(&progress);
            last_progress = Instant::now();
            let reached = measurement.pct_of_pot <= cfg.target_pct_of_pot;
            if cancelled || reached || at_cap {
                let stop_reason = if cancelled {
                    StopReason::Cancelled
                } else if reached {
                    StopReason::TargetReached
                } else {
                    StopReason::IterationCap
                };
                log::info!(
                    "{} stop_reason={stop_reason:?} iteration={iterations}",
                    progress.timestamp
                );
                return Ok(SolveReport {
                    iterations,
                    exploitability: measurement,
                    elapsed: start.elapsed(),
                    stop_reason,
                });
            }
        }
    }
}
