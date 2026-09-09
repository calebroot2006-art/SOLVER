//! A CFR session bound to one expanded postflop game.

use super::{PostflopGame, PostflopStrategy, terminal::PostflopTerminal};
use crate::memory::Lease;
use crate::{
    Cfr, Exploitability, NodeId, Progress, SolveConfig, SolveError, SolveReport, Variant,
    allocation::reserved,
    best_response::exploitability_bound,
    solver::{SolveSession, drive},
    terminal::ShowdownScratch,
};
use std::fmt;

/// CFR state permanently bound to an owned postflop game.
///
/// One reusable terminal workspace per resolved traversal worker is allocated up
/// front, so no iteration allocates a showdown scratch, and the memory estimate
/// charged for exactly that many. The traversal is still serial: every iteration
/// and every measurement runs on `scratch[0]` alone, because `Traversal` holds
/// one `&mut dyn TerminalEvaluator` over one shared scratch. Step 4 of
/// `docs/phase-4/PLAN.md` is what gives each worker its own evaluator.
pub struct PostflopSolver {
    game: PostflopGame,
    core: Cfr,
    scratch: Vec<ShowdownScratch>,
    _lease: Lease,
}

impl fmt::Debug for PostflopSolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostflopSolver")
            .field("game", &self.game)
            .field("iteration", &self.iteration())
            .finish()
    }
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
            scratch.push(ShowdownScratch::default());
        }
        Ok(Self {
            game,
            core,
            scratch,
            _lease: lease,
        })
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
        let _workspace = self.reserve_workspace()?;
        self.core.advance(&mut PostflopTerminal {
            game: &self.game.inner,
            scratch: &mut self.scratch[0],
        })
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
        let _workspace = self.reserve_workspace()?;
        exploitability_bound(
            &mut PostflopTerminal {
                game: &self.game.inner,
                scratch: &mut self.scratch[0],
            },
            strategy.policy(),
        )
    }
}
