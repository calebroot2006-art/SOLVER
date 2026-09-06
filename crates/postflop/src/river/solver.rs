use super::{RiverGame, RiverStrategy, game::RiverTerminal, memory::Lease};
use crate::{
    Cfr, Exploitability, NodeId, Progress, SolveConfig, SolveError, SolveReport, Variant,
    allocation::reserved,
    best_response::exploitability_bound,
    solver::{SolveSession, drive},
    terminal::ShowdownScratch,
};
use std::fmt;

/// CFR state permanently bound to an owned river game, with reusable terminal scratch.
pub struct RiverSolver {
    game: RiverGame,
    core: Cfr,
    scratch: Vec<ShowdownScratch>,
    _lease: Lease,
}

impl fmt::Debug for RiverSolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RiverSolver")
            .field("game", &self.game)
            .field("iteration", &self.iteration())
            .finish()
    }
}

impl RiverSolver {
    /// Reserve the retained solver buffers before allocating or mutating anything.
    pub fn new(game: RiverGame, variant: Variant) -> Result<Self, SolveError> {
        let memory = game.inner.memory;
        let lease = game
            .inner
            .budget
            .reserve(memory.solver_bytes + memory.scratch_bytes)?;
        let core = Cfr::from_layout(game.inner.layout.clone(), variant, None)?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        Ok(Self {
            game,
            core,
            scratch,
            _lease: lease,
        })
    }

    /// Inputs retained for the complete lifetime of this solver.
    pub fn game(&self) -> &RiverGame {
        &self.game
    }
    /// Number of fully completed alternating iterations.
    pub fn iteration(&self) -> u64 {
        self.core.iteration()
    }
    /// Advance both players once; a checked numerical failure poisons further reads.
    pub fn run_iteration(&mut self) -> Result<(), SolveError> {
        let _workspace = self
            .game
            .inner
            .budget
            .reserve(self.game.inner.memory.traversal_bytes)?;
        self.core.advance(&mut RiverTerminal {
            game: &self.game.inner,
            scratch: &mut self.scratch[0],
        })
    }
    /// Retain the reach-weighted average and its exact game binding.
    pub fn average_strategy(&self) -> Result<RiverStrategy, SolveError> {
        let lease = self
            .game
            .inner
            .budget
            .reserve(self.game.inner.memory.snapshot_bytes)?;
        let policy = self.core.average_bound()?;
        Ok(RiverStrategy {
            game: self.game.clone(),
            policy,
            _lease: lease,
        })
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
}

impl SolveSession for RiverSolver {
    fn iteration(&self) -> u64 {
        self.iteration()
    }
    fn step(&mut self) -> Result<(), SolveError> {
        self.run_iteration()
    }
    fn measurement(&mut self) -> Result<Exploitability, SolveError> {
        let strategy = self.average_strategy()?;
        let _workspace = self
            .game
            .inner
            .budget
            .reserve(self.game.inner.memory.traversal_bytes)?;
        exploitability_bound(
            &mut RiverTerminal {
                game: &self.game.inner,
                scratch: &mut self.scratch[0],
            },
            &strategy.policy,
        )
    }
}
