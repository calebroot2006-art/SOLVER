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
///
/// A solve that stopped on its target or its cap always carries a measurement
/// taken at the iteration it stopped on. A cancelled solve does not: cancel
/// runs no best-response walk (`docs/phase-4/job-contract.md`), so it reports
/// the last measurement it happens to hold, with the iteration that
/// measurement covers, and [`Self::stale_measurement`] set when that iteration
/// is behind [`Self::iterations`]. A solve cancelled before any measurement
/// reports `None`, which is the honest answer: nothing was measured.
#[derive(Clone, Debug)]
pub struct SolveReport {
    /// Total completed iterations, including any run before this invocation.
    pub iterations: u64,
    /// Measured accuracy within this game's tree, or `None` when a cancel
    /// arrived before the first measurement.
    pub exploitability: Option<Exploitability>,
    /// Iteration [`Self::exploitability`] covers, `None` with it.
    pub measured_at: Option<u64>,
    /// Whether the measurement is older than [`Self::iterations`].
    pub stale_measurement: bool,
    /// Wall time spent in this driver invocation.
    pub elapsed: Duration,
    /// Target, iteration cap, or cancellation, explicitly distinguished.
    pub stop_reason: StopReason,
}

impl SolveReport {
    /// The measurement, or a named error when the solve measured nothing.
    ///
    /// Callers that only ever stop on a target or a cap use this instead of
    /// unwrapping: those two paths always measured, so a `None` here means the
    /// caller cancelled and should have read [`Self::stop_reason`] first.
    pub fn measured(&self) -> Result<Exploitability, SolveError> {
        self.exploitability.ok_or_else(|| {
            SolveError::InvalidGame(format!(
                "the solve stopped as {:?} at iteration {} before any measurement was taken",
                self.stop_reason, self.iterations
            ))
        })
    }
}

/// Runs to a measured target or total iteration cap.
///
/// Accuracy is measured after `check_every` iterations and on the final
/// iteration, and nowhere else: those are the only measurements the stop test
/// can see, so the iteration a solve stops on depends on the configured
/// schedule and not on how fast the host ran. `log_every_secs` drives the
/// progress callback alone and never takes a measurement of its own; an event
/// it emits between measurements repeats the last one with
/// [`Progress::stale`] set. Checks occur between iterations, so an individual
/// slow iteration can exceed the requested logging interval.
pub fn solve(
    game: &dyn Game,
    solver: &mut dyn Solver,
    cfg: &SolveConfig,
    on_progress: impl FnMut(&Progress),
) -> Result<SolveReport, SolveError> {
    drive(
        &mut LegacySession { game, solver },
        cfg,
        on_progress,
        || false,
    )
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
    fn iteration(&self) -> u64 {
        self.solver.iteration()
    }
    fn step(&mut self) -> Result<(), SolveError> {
        self.solver.run_iteration(self.game)
    }
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
    // The last measurement taken and the iteration it covers. Only a
    // measurement whose iteration is the current one can satisfy the target.
    let mut measurement: Option<(Exploitability, u64)> = None;
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
        // Cancel takes no measurement: a best-response walk over every runout
        // is the most expensive thing this crate does, and a caller who asked
        // to stop is not asking for one more of them.
        let measure = !cancelled && (at_cap || iterations.is_multiple_of(cfg.check_every));
        if measure {
            let taken = session
                .measurement()
                .map_err(|error| stamp(error, iterations))?;
            measurement = Some((taken, iterations));
        }
        let timed = last_progress.elapsed() >= Duration::from_secs(cfg.log_every_secs);
        let stopping = cancelled || at_cap;
        let fresh = measurement.is_some_and(|(_, at)| at == iterations);
        let reached = fresh
            && measurement.is_some_and(|(taken, _)| taken.pct_of_pot <= cfg.target_pct_of_pot);
        if measure || timed || stopping {
            let progress = Progress::record(iterations, measurement, start.elapsed());
            on_progress(&progress);
            last_progress = Instant::now();
            if stopping || reached {
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
                    exploitability: measurement.map(|(taken, _)| taken),
                    measured_at: measurement.map(|(_, at)| at),
                    stale_measurement: measurement.is_some_and(|(_, at)| at != iterations),
                    elapsed: start.elapsed(),
                    stop_reason,
                });
            }
        }
    }
}

/// Puts this iteration's number on an error raised inside a measurement.
fn stamp(error: SolveError, iterations: u64) -> SolveError {
    match error {
        SolveError::NonFinite { node, player, .. } => SolveError::NonFinite {
            iteration: iterations,
            node,
            player,
        },
        SolveError::Terminal {
            node,
            player,
            reason,
            ..
        } => SolveError::Terminal {
            iteration: iterations,
            node,
            player,
            reason,
        },
        SolveError::Arithmetic {
            node,
            player,
            reason,
            ..
        } => SolveError::Arithmetic {
            iteration: iterations,
            node,
            player,
            reason,
        },
        other => other,
    }
}
