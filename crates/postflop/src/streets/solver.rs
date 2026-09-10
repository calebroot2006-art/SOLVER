//! A CFR session bound to one expanded postflop game.

use super::game::Inner;
use super::{PostflopGame, PostflopStrategy, terminal::PostflopTerminal};
use crate::memory::Lease;
use crate::traversal::{Parallel, SharedTerminal, SubtreeRanges, TerminalEvaluator};
use crate::{
    Cfr, Exploitability, NodeId, Progress, Real, SolveConfig, SolveError, SolveReport, Variant,
    allocation::reserved,
    best_response::{exploitability_bound, exploitability_parallel},
    solver::{SolveSession, drive},
    terminal::ShowdownScratch,
};
use std::fmt;
use std::sync::{Mutex, PoisonError};

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
    scratch: Vec<Mutex<ShowdownScratch>>,
    /// Present only above one worker; its size is the resolved worker count.
    pool: Option<rayon::ThreadPool>,
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
/// [`PostflopTerminal`] needs `&mut ShowdownScratch`, so each call takes the slot
/// belonging to the worker making it. `ShowdownTable::evaluate` clears the whole
/// workspace before it reads any of it, so no value depends on which worker
/// wrote to that slot last.
struct PostflopShared<'a> {
    game: &'a Inner,
    scratch: &'a [Mutex<ShowdownScratch>],
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
            scratch: &mut workspace,
        }
        .evaluate_terminal(node, player, opponent, output, iteration)
    }
}

/// A workspace a panicking worker poisoned is still usable: every evaluation
/// clears it before reading it, so its contents carry nothing forward.
fn workspace(slot: &mut Mutex<ShowdownScratch>) -> &mut ShowdownScratch {
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
            scratch.push(Mutex::new(ShowdownScratch::default()));
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
            _lease: lease,
        })
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
                scratch: workspace(&mut self.scratch[0]),
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
    /// Read the current flattened state-major policy for a valid node.
    /// Average strategies, not this diagnostic policy, certify convergence.
    pub fn current_row(&self, node: NodeId) -> Result<Option<&[f64]>, SolveError> {
        Ok(self.core.current_strategy()?.row(node))
    }
    /// Signed cumulative regrets, available only while the solver is healthy.
    pub fn regrets(&self, node: NodeId) -> Result<Option<&[f64]>, SolveError> {
        self.core.current_strategy()?;
        Ok(self.core.regrets(node))
    }
    /// Whole cumulative strategy sums, available only while the solver is healthy.
    pub fn strategy_sum(&self, node: NodeId) -> Result<Option<&[f64]>, SolveError> {
        self.core.current_strategy()?;
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
    /// Check cancellation before each full iteration. A cancelled report contains
    /// a fresh measurement of the last complete average; the session can resume.
    pub fn solve_with_cancel(
        &mut self,
        config: &SolveConfig,
        on_progress: impl FnMut(&Progress),
        should_cancel: impl FnMut() -> bool,
    ) -> Result<SolveReport, SolveError> {
        drive(self, config, on_progress, should_cancel)
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
    fn measurement(&mut self) -> Result<Exploitability, SolveError> {
        let strategy = self.average_strategy()?;
        let _reservation = self.reserve_workspace()?;
        match &self.pool {
            Some(pool) => {
                let terminal = PostflopShared {
                    game: &self.game.inner,
                    scratch: &self.scratch,
                };
                let ranges = GameRanges(&self.game);
                exploitability_parallel(
                    &Parallel {
                        terminal: &terminal,
                        ranges: &ranges,
                        pool,
                    },
                    strategy.policy(),
                )
            }
            None => exploitability_bound(
                &mut PostflopTerminal {
                    game: &self.game.inner,
                    scratch: workspace(&mut self.scratch[0]),
                },
                strategy.policy(),
            ),
        }
    }
}
