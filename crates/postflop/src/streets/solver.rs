//! A CFR session bound to one expanded postflop game.

use super::game::Inner;
use super::{
    PostflopGame, PostflopStrategy,
    terminal::{PostflopTerminal, TerminalWorkspace},
};
use crate::memory::Lease;
use crate::traversal::{Parallel, SharedTerminal, SubtreeRanges, TerminalEvaluator};
use crate::{
    Cfr, Exploitability, NodeId, Progress, Real, SolveConfig, SolveError, SolveReport, Variant,
    allocation::reserved,
    best_response::{exploitability_sums, exploitability_sums_parallel},
    solver::{SolveSession, drive},
};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// Job identities issued for this process, never reused.
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// Which solve attempt a result was produced under.
///
/// `docs/phase-4/job-contract.md` gives every accepted request a `JobId` and a
/// generation counter, and rejects a completion or measurement whose pair is
/// not the driver's current one. That is what makes a result from a cancelled
/// worker harmless: the cancel moves the generation on, so the old pair no
/// longer matches and [`PostflopSolver::accept`] refuses the report instead of
/// letting a stale strategy be read as the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JobId {
    job: u64,
    generation: u32,
}

impl JobId {
    /// Process-unique identifier of the solver this attempt runs on.
    #[must_use]
    pub fn job(self) -> u64 {
        self.job
    }
    /// Attempts started or cancelled on that solver before this one.
    #[must_use]
    pub fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "job {} generation {}", self.job, self.generation)
    }
}

/// CFR state permanently bound to an owned postflop game.
///
/// One reusable terminal workspace per resolved traversal worker is allocated up
/// front, so no iteration allocates a showdown scratch and the memory estimate
/// charged for exactly that many. More than one worker also builds a thread pool
/// of that size, and every chance node with more than one dealt card spreads its
/// runouts across it. One worker builds no pool at all and runs the serial walk
/// unchanged. Either way the answer is the same to the bit: outcomes are reduced
/// in outcome order and each one is walked by the same code.
pub struct PostflopSolver {
    game: PostflopGame,
    core: Cfr,
    /// One showdown workspace per worker. The serial path takes `[0]` through
    /// `get_mut`, which locks nothing; a parallel worker takes the slot its own
    /// pool index names, so the locks below are never contended.
    scratch: Vec<Mutex<TerminalWorkspace>>,
    /// Present only above one worker; its size is the resolved worker count.
    pool: Option<rayon::ThreadPool>,
    /// This solver's process-unique job number and the generation counter a
    /// cancel moves on, together the identity a result has to carry.
    job: JobId,
    _lease: Lease,
}

impl fmt::Debug for PostflopSolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostflopSolver")
            .field("game", &self.game)
            .field("iteration", &self.iteration())
            .field("workers", &self.workers())
            .finish()
    }
}

/// The expanded tree's depth-first node ranges, which is what a parallel chance
/// node splits accumulators along. Each outcome owns `outcome_range(chance, k)`.
struct GameRanges<'a>(&'a PostflopGame);

impl SubtreeRanges for GameRanges<'_> {
    fn end(&self, node: NodeId) -> Option<NodeId> {
        self.0.subtree(node).map(|range| range.end)
    }
}

/// The terminal boundary every worker shares, one workspace behind it per worker.
///
/// [`PostflopTerminal`] needs `&mut TerminalWorkspace`, so each call takes the slot
/// belonging to the worker making it. `ShowdownTable::evaluate` clears the whole
/// workspace before it reads any of it, so no value depends on which worker
/// wrote to that slot last.
struct PostflopShared<'a> {
    game: &'a Inner,
    scratch: &'a [Mutex<TerminalWorkspace>],
}

impl SharedTerminal for PostflopShared<'_> {
    fn checks_reach_underflow(&self) -> bool {
        true
    }

    fn evaluate_terminal(
        &self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError> {
        let slot = rayon::current_thread_index().unwrap_or(0) % self.scratch.len();
        let mut workspace = self.scratch[slot]
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        PostflopTerminal {
            game: self.game,
            workspace: &mut workspace,
        }
        .evaluate_terminal(node, player, opponent, output, iteration)
    }
}

/// A workspace a panicking worker poisoned is still usable: every evaluation
/// clears it before reading it, so its contents carry nothing forward.
fn workspace(slot: &mut Mutex<TerminalWorkspace>) -> &mut TerminalWorkspace {
    slot.get_mut().unwrap_or_else(PoisonError::into_inner)
}

impl PostflopSolver {
    /// Reserve the retained solver buffers before allocating or mutating anything.
    pub fn new(game: PostflopGame, variant: Variant) -> Result<Self, SolveError> {
        let memory = game.inner.memory;
        let workers = game.inner.workers;
        let reservation = memory
            .scratch_bytes
            .checked_mul(workers)
            .and_then(|bytes| bytes.checked_add(memory.solver_bytes))
            .ok_or_else(|| SolveError::Allocation("solver reservation overflow".into()))?;
        let lease = game.inner.budget.reserve(reservation)?;
        let core = Cfr::from_layout(game.inner.layout.clone(), variant, None)?;
        let mut scratch = reserved(workers)?;
        for _ in 0..workers {
            scratch.push(Mutex::new(TerminalWorkspace::default()));
        }
        // The pool size and the estimate's worker term are the same number, from
        // the one `streets::resolve_workers` answer the game recorded.
        let pool = if workers > 1 {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .thread_name(|index| format!("postflop-runout-{index}"))
                    .build()
                    .map_err(|error| {
                        SolveError::Allocation(format!(
                            "cannot start {workers} traversal workers: {error}"
                        ))
                    })?,
            )
        } else {
            None
        };
        Ok(Self {
            game,
            core,
            scratch,
            pool,
            job: JobId {
                job: NEXT_JOB.fetch_add(1, Ordering::Relaxed),
                generation: 0,
            },
            _lease: lease,
        })
    }

    /// Identity every result of the current attempt carries.
    ///
    /// A cancelled `solve_with_cancel` moves the generation on before it
    /// returns, so the pair a caller read before the cancel no longer names the
    /// current attempt and [`Self::accept`] refuses anything produced under it.
    #[must_use]
    pub fn job(&self) -> JobId {
        self.job
    }

    /// Passes a result through only when it was produced under the current
    /// attempt, and names the mismatch when it was not.
    pub fn accept<T>(&self, job: JobId, result: T) -> Result<T, SolveError> {
        if job == self.job {
            return Ok(result);
        }
        Err(SolveError::InvalidGame(format!(
            "a result from {job} arrived after the solver moved on to {}; it is rejected, \
             not delivered",
            self.job
        )))
    }

    /// Traversal workers this solve runs on, which is the size of its thread
    /// pool and the worker count the memory estimate charged for.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(1, rayon::ThreadPool::current_num_threads)
    }

    /// Inputs retained for the complete lifetime of this solver.
    #[must_use]
    pub fn game(&self) -> &PostflopGame {
        &self.game
    }
    /// Number of fully completed alternating iterations.
    #[must_use]
    pub fn iteration(&self) -> u64 {
        self.core.iteration()
    }
    /// Advance both players once; a checked numerical failure poisons further reads.
    pub fn run_iteration(&mut self) -> Result<(), SolveError> {
        let _reservation = self.reserve_workspace()?;
        match &self.pool {
            Some(pool) => {
                let terminal = PostflopShared {
                    game: &self.game.inner,
                    scratch: &self.scratch,
                };
                let ranges = GameRanges(&self.game);
                self.core.advance_parallel(&Parallel {
                    terminal: &terminal,
                    ranges: &ranges,
                    pool,
                })
            }
            None => self.core.advance(&mut PostflopTerminal {
                game: &self.game.inner,
                workspace: workspace(&mut self.scratch[0]),
            }),
        }
    }
    /// Retain the reach-weighted average and its exact game binding.
    pub fn average_strategy(&self) -> Result<PostflopStrategy, SolveError> {
        let lease = self
            .game
            .inner
            .budget
            .reserve(self.game.inner.memory.snapshot_bytes)?;
        let policy = self.core.average_bound()?;
        Ok(PostflopStrategy::bind(self.game.clone(), policy, lease))
    }
    /// Derive the current flattened state-major policy for a valid node.
    /// Average strategies, not this diagnostic policy, certify convergence.
    /// It is owned rather than borrowed because nothing stores it: it is regret
    /// matching over this node's regrets, computed where it is asked for.
    pub fn current_row(&self, node: NodeId) -> Result<Option<Vec<f64>>, SolveError> {
        self.core.current_row(node)
    }
    /// Signed cumulative regrets, available only while the solver is healthy.
    pub fn regrets(&self, node: NodeId) -> Result<Option<&[f64]>, SolveError> {
        self.core.health()?;
        Ok(self.core.regrets(node))
    }
    /// Whole cumulative strategy sums, available only while the solver is healthy.
    pub fn strategy_sum(&self, node: NodeId) -> Result<Option<&[f64]>, SolveError> {
        self.core.health()?;
        Ok(self.core.strategy_sum(node))
    }
    /// Run to the explicit target or total iteration cap, emitting measured progress.
    pub fn solve(
        &mut self,
        config: &SolveConfig,
        on_progress: impl FnMut(&Progress),
    ) -> Result<SolveReport, SolveError> {
        self.solve_with_cancel(config, on_progress, || false)
    }
    /// Check cancellation before each full iteration.
    ///
    /// Cancel runs no best-response measurement. The report carries the last
    /// measurement this invocation took, the iteration that measurement covers,
    /// and `stale_measurement` when that iteration is behind the one the solve
    /// stopped on; a cancel before the first measurement reports no measurement
    /// at all. The session can resume, but the attempt cannot: a cancelled
    /// return moves this solver's generation on, so [`Self::accept`] refuses
    /// anything still carrying the old [`JobId`].
    pub fn solve_with_cancel(
        &mut self,
        config: &SolveConfig,
        on_progress: impl FnMut(&Progress),
        should_cancel: impl FnMut() -> bool,
    ) -> Result<SolveReport, SolveError> {
        let report = drive(self, config, on_progress, should_cancel);
        if matches!(
            report,
            Ok(SolveReport {
                stop_reason: crate::StopReason::Cancelled,
                ..
            })
        ) {
            self.job.generation = self.job.generation.saturating_add(1);
        }
        report
    }

    fn reserve_workspace(&self) -> Result<Lease, SolveError> {
        let memory = self.game.inner.memory;
        let bytes = memory
            .traversal_bytes
            .checked_mul(self.game.inner.workers)
            .ok_or_else(|| SolveError::Allocation("traversal reservation overflow".into()))?;
        self.game.inner.budget.reserve(bytes)
    }
}

impl SolveSession for PostflopSolver {
    fn iteration(&self) -> u64 {
        self.iteration()
    }
    fn step(&mut self) -> Result<(), SolveError> {
        self.run_iteration()
    }
    /// Measures the average strategy without retaining one.
    ///
    /// Regret matching over the cumulative strategy sums is the average
    /// strategy, row by row, so the best-response walk normalises the sums as
    /// it reads them. That is the same arithmetic on the same numbers as
    /// measuring a materialised average, and it is why a running solve holds no
    /// snapshot at all: on the gate flop tree one would be 17.8 GB.
    fn measurement(&mut self) -> Result<Exploitability, SolveError> {
        self.core.health()?;
        let _reservation = self.reserve_workspace()?;
        let layout = self.core.layout().clone();
        match &self.pool {
            Some(pool) => {
                let terminal = PostflopShared {
                    game: &self.game.inner,
                    scratch: &self.scratch,
                };
                let ranges = GameRanges(&self.game);
                exploitability_sums_parallel(
                    &Parallel {
                        terminal: &terminal,
                        ranges: &ranges,
                        pool,
                    },
                    &layout,
                    self.core.sums(),
                )
            }
            None => {
                let sums = self.core.sums();
                exploitability_sums(
                    &mut PostflopTerminal {
                        game: &self.game.inner,
                        workspace: workspace(&mut self.scratch[0]),
                    },
                    &layout,
                    sums,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streets::{PostflopGame, PostflopOptions};
    use crate::{Precision, StopReason};
    use cards::{Card, Range};
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;
    use tree::{BetSizeOptions, PostflopTree, PostflopTreeConfig, Street};

    const LIMIT: usize = 1024 * 1024 * 1024;

    /// The same small turn tree the strategy tests use: one jam, one call, one
    /// river deal. Big enough that an iteration and a best-response walk are
    /// worth timing, small enough to run several of them in a unit test.
    fn game(memory_limit_bytes: usize) -> PostflopGame {
        let menu = || {
            let sizes = BetSizeOptions::try_from(("a", "")).unwrap();
            [sizes.clone(), sizes]
        };
        let tree = PostflopTree::new(PostflopTreeConfig {
            starting_pot: 10,
            effective_stack: 20,
            min_bet: 1,
            start_street: Street::Turn,
            sizes: [menu(), menu(), menu()],
            max_raises: 0,
            add_all_in_threshold: 0.0,
            force_all_in_threshold: 0.0,
            max_nodes: 100_000,
        })
        .unwrap();
        let board: Vec<Card> = "9c 5d 2h Ks"
            .split_ascii_whitespace()
            .map(|card| card.parse().unwrap())
            .collect();
        PostflopGame::new(
            &board,
            [
                Range::parse("AA, QQ, JTs").unwrap(),
                Range::parse("KK, 99, 76s").unwrap(),
            ],
            tree,
            PostflopOptions {
                memory_limit_bytes,
                precision: Precision::F64,
                threads: 1,
            },
        )
        .unwrap()
    }

    /// Never reachable, so a probe can only stop on its cancel or its cap.
    fn probe_config(check_every: u64, max_iterations: u64) -> SolveConfig {
        SolveConfig {
            target_pct_of_pot: 0.0,
            max_iterations,
            check_every,
            // Far above the probe's own run: these tests are about the stop
            // test, and a timed event would only add log lines.
            log_every_secs: 3600,
            threads: 1,
        }
    }

    /// Runs until the cancel flag has been observed `polls` times, then sets it.
    fn cancel_after(polls: u64) -> impl FnMut() -> bool {
        let mut seen = 0_u64;
        move || {
            seen += 1;
            seen > polls
        }
    }

    #[test]
    fn cancel_takes_no_measurement_and_reports_the_last_one_with_its_iteration() {
        let game = game(LIMIT);
        let mut solver = PostflopSolver::new(game, Variant::Plus).unwrap();
        let mut fresh = 0;
        let report = solver
            .solve_with_cancel(
                &probe_config(2, 100),
                |progress| {
                    if !progress.stale {
                        fresh += 1;
                    }
                },
                cancel_after(3),
            )
            .unwrap();
        // Iterations one, two and three ran; the fourth poll cancelled. Only
        // iteration two was a multiple of `check_every`, so that is the one and
        // only measurement, and the cancel added none of its own.
        assert_eq!(report.stop_reason, StopReason::Cancelled);
        assert_eq!(report.iterations, 3);
        assert_eq!(fresh, 1);
        assert_eq!(report.measured_at, Some(2));
        assert!(report.stale_measurement);
        assert!(report.exploitability.is_some());
    }

    #[test]
    fn a_cancel_on_the_measured_iteration_reports_a_fresh_measurement() {
        let game = game(LIMIT);
        let mut solver = PostflopSolver::new(game, Variant::Plus).unwrap();
        // The flag goes up inside the block that has just measured, which is
        // the only place a measurement can be interrupted from: the driver's
        // single observation point is the top of the next loop, so a flag set
        // during an iteration and one set during a measurement are observed
        // there alike.
        let cancel = AtomicBool::new(false);
        let report = solver
            .solve_with_cancel(
                &probe_config(2, 100),
                |progress| {
                    if !progress.stale {
                        cancel.store(true, Ordering::Release);
                    }
                },
                || cancel.load(Ordering::Acquire),
            )
            .unwrap();
        assert_eq!(report.stop_reason, StopReason::Cancelled);
        assert_eq!(report.iterations, 2);
        assert_eq!(report.measured_at, Some(2));
        assert!(!report.stale_measurement);
    }

    #[test]
    fn a_cancel_before_the_first_measurement_reports_no_measurement_at_all() {
        let game = game(LIMIT);
        let mut solver = PostflopSolver::new(game, Variant::Plus).unwrap();
        let report = solver
            .solve_with_cancel(&probe_config(u64::MAX, 100), |_| {}, cancel_after(2))
            .unwrap();
        assert_eq!(report.stop_reason, StopReason::Cancelled);
        assert_eq!(report.iterations, 2);
        assert_eq!(report.exploitability, None);
        assert_eq!(report.measured_at, None);
        assert!(!report.stale_measurement);
        // The report says so rather than inventing a number for the caller.
        let error = report.measured().unwrap_err().to_string();
        assert!(error.contains("before any measurement"), "{error}");
    }

    #[test]
    fn a_cancel_returns_without_paying_for_a_best_response_walk() {
        let game = game(LIMIT);
        let mut solver = PostflopSolver::new(game, Variant::Plus).unwrap();
        solver.run_iteration().unwrap();
        let clock = Instant::now();
        let measurement = solver.measurement().unwrap();
        let measured = clock.elapsed();
        assert!(measurement.pct_of_pot.is_finite());

        let clock = Instant::now();
        let report = solver
            .solve_with_cancel(&probe_config(u64::MAX, 100), |_| {}, cancel_after(0))
            .unwrap();
        let cancelled = clock.elapsed();
        // The flag was up at the first poll, so no iteration ran either. What
        // is left is the return itself, which cannot be a measurement: a
        // best-response walk over every runout is the slowest thing here.
        assert_eq!(report.stop_reason, StopReason::Cancelled);
        assert_eq!(report.iterations, 1);
        assert!(
            cancelled * 2 < measured,
            "a cancel took {cancelled:?} against a measurement's {measured:?}"
        );
    }

    #[test]
    fn a_cancelled_attempt_rejects_its_own_late_result_and_a_replacement_reserves() {
        let game = game(LIMIT);
        let mut solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
        let attempt = solver.job();
        let report = solver
            .solve_with_cancel(&probe_config(2, 100), |_| {}, cancel_after(3))
            .unwrap();
        assert_eq!(report.stop_reason, StopReason::Cancelled);

        // The cancel moved the generation on, so the identity the caller was
        // holding no longer names the current attempt.
        assert_eq!(solver.job().job(), attempt.job());
        assert_eq!(solver.job().generation(), attempt.generation() + 1);
        let rejected = solver
            .accept(attempt, report.iterations)
            .unwrap_err()
            .to_string();
        assert!(
            rejected.contains("is rejected, not delivered"),
            "{rejected}"
        );
        assert_eq!(solver.accept(solver.job(), report.iterations).unwrap(), 3);

        // A replacement job is a different job number, never a reused one.
        let replacement = PostflopSolver::new(game, Variant::Plus).unwrap();
        assert_ne!(replacement.job().job(), attempt.job());
    }

    #[test]
    fn a_replacement_reserves_only_once_the_cancelled_job_has_released() {
        // Sized to the estimate, which is what a configured limit is meant to
        // be: the budget then admits the solver the bound charges for and
        // refuses a second one, which is the case this test is about.
        let bound = game(LIMIT).memory_usage().working_set_bound_bytes;
        let game = game(bound);
        let shared = game.reserved_bytes();
        let mut held = Vec::new();
        let refusal = loop {
            match PostflopSolver::new(game.clone(), Variant::Plus) {
                Ok(mut solver) => {
                    solver
                        .solve_with_cancel(&probe_config(2, 100), |_| {}, cancel_after(1))
                        .unwrap();
                    held.push(solver);
                    assert!(
                        held.len() < 64,
                        "a limit sized to the estimate admitted 64 solvers on one game"
                    );
                }
                Err(error) => break error,
            }
        };
        assert!(
            matches!(refusal, SolveError::MemoryLimit { .. }),
            "a replacement over the limit is refused by name, not by wait: {refusal}"
        );
        assert!(held.len() > 1, "the budget admitted only {}", held.len());
        assert!(game.reserved_bytes() > shared);

        // Release one, and the replacement that was refused a moment ago fits.
        held.pop();
        let replacement = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
        drop(replacement);
        held.clear();
        // Every buffer every attempt held is back in the budget.
        assert_eq!(game.reserved_bytes(), shared);
    }
}
