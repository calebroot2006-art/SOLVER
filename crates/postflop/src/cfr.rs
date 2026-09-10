//! Alternating CFR, regret-matching+, and signed Discounted CFR.
use crate::allocation::{collect, filled, reserved, try_collect};
use crate::error::{reach_product, weighted_product};
use crate::{
    Game, NodeId, NodeKind, Real, SolveError, Solver, Strategy,
    error::finite,
    game::{Layout, TraversalLayout},
    strategy::{RowPool, normalize_positive},
    traversal::{LegacyTerminal, Parallel, SharedRef, TerminalEvaluator},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use std::ops::Range;
use std::sync::Arc;

/// Regret and averaging update rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variant {
    /// Signed cumulative regrets with uniform iteration weighting.
    Vanilla,
    /// Stored regrets floored after an infoset update; linear iteration weights.
    Plus,
    /// Signed regrets and whole-accumulator discounting after each iteration.
    Discounted {
        /// Exponent for positive regret discounting.
        alpha: Real,
        /// Exponent for negative regret discounting.
        beta: Real,
        /// Exponent for cumulative strategy discounting.
        gamma: Real,
    },
}

/// CFR state bound to one immutable public-tree layout.
///
/// Two arrays are stored, not three. The current policy is regret matching over
/// the regrets, so it is derived at the moment a walk reads a node rather than
/// kept in a third array and refreshed after every update. That is the same
/// number either way: a walk reads a node's row before it touches that node's
/// regrets, so the row it derives is the row the stored copy would have held.
///
/// Both arrays are flat. Node `n` owns `layout.row_offsets[n]..[n + 1]` of each,
/// which is what lets one chance outcome's worker take a contiguous slice of
/// both and no other worker touch it.
///
/// A failed numerical update poisons the solver: subsequent updates and average
/// reads return the original error instead of exposing a partial iteration.
#[derive(Clone, Debug)]
pub struct Cfr {
    layout: Arc<TraversalLayout>,
    /// Present only for callback games, which must be re-checked every call.
    legacy_binding: Option<Arc<Layout>>,
    regrets: Vec<Real>,
    strategy_sum: Vec<Real>,
    variant: Variant,
    iteration: u64,
    failure: Option<SolveError>,
}

impl Cfr {
    /// Validates the complete tree and initializes uniform play and zero regrets.
    pub fn new(game: &dyn Game, variant: Variant) -> Result<Self, SolveError> {
        validate_variant(variant)?;
        let binding = Arc::new(Layout::new(game)?);
        Self::from_layout(binding.traversal.clone(), variant, Some(binding))
    }

    pub(crate) fn from_layout(
        layout: Arc<TraversalLayout>,
        variant: Variant,
        legacy_binding: Option<Arc<Layout>>,
    ) -> Result<Self, SolveError> {
        validate_variant(variant)?;
        let entries = layout.entries();
        Ok(Self {
            layout,
            legacy_binding,
            regrets: filled(entries, 0.0)?,
            strategy_sum: filled(entries, 0.0)?,
            variant,
            iteration: 0,
            failure: None,
        })
    }

    /// Number of fully completed alternating iterations.
    #[must_use]
    pub fn iteration(&self) -> u64 {
        self.iteration
    }

    /// The original error once an update has failed, so a caller cannot read a
    /// half-finished iteration as though it were a policy.
    pub fn health(&self) -> Result<(), SolveError> {
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn check_game(&self, game: &dyn Game) -> Result<(), SolveError> {
        self.legacy_binding
            .as_ref()
            .ok_or_else(|| {
                SolveError::InvalidGame(
                    "owned river strategies cannot be rebound to callback games".into(),
                )
            })?
            .check_game(game)
    }

    /// Runs a player-zero update followed by a player-one update.
    pub fn run_iteration(&mut self, game: &dyn Game) -> Result<(), SolveError> {
        self.health()?;
        self.check_game(game)?;
        self.advance(&mut LegacyTerminal(game))
    }

    pub(crate) fn advance(
        &mut self,
        terminal: &mut dyn TerminalEvaluator,
    ) -> Result<(), SolveError> {
        self.advance_with(terminal, None)
    }

    /// Runs one iteration with every chance node's outcomes spread over the
    /// context's workers.
    ///
    /// The result is bit for bit the serial result: each outcome is walked by
    /// the same code, and the values come back in outcome order and are reduced
    /// in outcome order regardless of which worker finished first.
    pub(crate) fn advance_parallel(&mut self, context: &Parallel<'_>) -> Result<(), SolveError> {
        context.pool.install(|| {
            let mut terminal = SharedRef(context.terminal);
            self.advance_with(&mut terminal, Some(context))
        })
    }

    fn advance_with(
        &mut self,
        terminal: &mut dyn TerminalEvaluator,
        parallel: Option<&Parallel<'_>>,
    ) -> Result<(), SolveError> {
        self.health()?;
        let next = self
            .iteration
            .checked_add(1)
            .ok_or_else(|| SolveError::Config("iteration counter overflow".into()))?;
        let result = self.update(terminal, parallel, next);
        match result {
            Ok(()) => {
                self.iteration = next;
                Ok(())
            }
            Err(error) => {
                self.failure = Some(error.clone());
                Err(error)
            }
        }
    }

    fn update(
        &mut self,
        terminal: &mut dyn TerminalEvaluator,
        parallel: Option<&Parallel<'_>>,
        iteration: u64,
    ) -> Result<(), SolveError> {
        let average_weight = if self.variant == Variant::Plus {
            iteration as Real
        } else {
            1.0
        };
        let nodes = self.layout.num_nodes();
        for player in 0..2 {
            let own = filled(self.layout.states[player], 1.0)?;
            let live = collect(
                self.layout.weights[player]
                    .iter()
                    .map(|w| if *w > 0.0 { 1.0 } else { 0.0 }),
            )?;
            let mut traversal = Traversal {
                terminal,
                layout: &self.layout,
                regrets: &mut self.regrets,
                strategy_sum: &mut self.strategy_sum,
                owned: 0..nodes as NodeId,
                base_entry: 0,
                rows: RowPool::default(),
                parallel,
                player,
                iteration,
                average_weight,
            };
            traversal.walk(
                self.layout.root,
                &self.layout.weights[1 - player],
                &own,
                &live,
            )?;
            for id in 0..nodes {
                if let NodeKind::Player { player: actor, .. } = self.layout.kinds[id]
                    && actor as usize == player
                {
                    let range = self.layout.row_range(id);
                    if self.variant == Variant::Plus {
                        for regret in &mut self.regrets[range.clone()] {
                            *regret = regret.max(0.0);
                        }
                    }
                    finite(
                        &self.regrets[range.clone()],
                        iteration,
                        id as NodeId,
                        player,
                    )?;
                    finite(&self.strategy_sum[range], iteration, id as NodeId, player)?;
                }
            }
        }
        if let Variant::Discounted { alpha, beta, gamma } = self.variant {
            let t = iteration as Real;
            // Reciprocal form avoids infinity/infinity for large finite exponents.
            let positive = 1.0 / (1.0 + t.powf(-alpha));
            let negative = 1.0 / (1.0 + t.powf(-beta));
            let strategy = (t / (t + 1.0)).powf(gamma);
            discount(
                &mut self.regrets,
                &mut self.strategy_sum,
                positive,
                negative,
                strategy,
            );
        }
        for id in 0..nodes {
            if let NodeKind::Player { player, .. } = self.layout.kinds[id] {
                let range = self.layout.row_range(id);
                finite(
                    &self.regrets[range.clone()],
                    iteration,
                    id as NodeId,
                    player as usize,
                )?;
                finite(
                    &self.strategy_sum[range],
                    iteration,
                    id as NodeId,
                    player as usize,
                )?;
            }
        }
        Ok(())
    }

    /// Returns the reach-weighted average, rejecting failed or mismatched games.
    pub fn average_strategy(&self, game: &dyn Game) -> Result<Strategy, SolveError> {
        self.health()?;
        self.check_game(game)?;
        self.average_bound()
    }

    pub(crate) fn average_bound(&self) -> Result<Strategy, SolveError> {
        self.health()?;
        self.normalised(&self.strategy_sum)
    }

    /// Current policy for diagnostics. Use the average for convergence claims.
    /// A failed iteration returns its error instead of a partial policy.
    ///
    /// This materialises a whole array. Nothing in a solve does: the walks
    /// derive one node's row at a time from the regrets they are already
    /// holding, which is why there is no third stored array to read here.
    pub fn current_strategy(&self) -> Result<Strategy, SolveError> {
        self.health()?;
        self.normalised(&self.regrets)
    }

    /// One node's current policy row, derived where it is asked for.
    pub fn current_row(&self, node: NodeId) -> Result<Option<Vec<Real>>, SolveError> {
        self.health()?;
        let Some(kind) = self.layout.kinds.get(node as usize).copied() else {
            return Ok(None);
        };
        let NodeKind::Player { num_actions, .. } = kind else {
            return Ok(Some(Vec::new()));
        };
        let source = &self.regrets[self.layout.row_range(node as usize)];
        let mut row = filled(source.len(), 0.0)?;
        for (values, out) in source
            .chunks_exact(num_actions as usize)
            .zip(row.chunks_exact_mut(num_actions as usize))
        {
            normalize_positive(values, out);
        }
        Ok(Some(row))
    }

    /// Regret matching over one stored array, as a checked strategy.
    fn normalised(&self, source: &[Real]) -> Result<Strategy, SolveError> {
        let mut values = filled(self.layout.entries(), 0.0)?;
        for id in 0..self.layout.num_nodes() {
            if let NodeKind::Player { num_actions, .. } = self.layout.kinds[id] {
                let range = self.layout.row_range(id);
                let n = num_actions as usize;
                for (sum, row) in source[range.clone()]
                    .chunks_exact(n)
                    .zip(values[range.clone()].chunks_exact_mut(n))
                {
                    normalize_positive(sum, row);
                }
            }
        }
        Strategy::from_values(self.layout.clone(), self.legacy_binding.clone(), values)
    }

    /// Read-only signed cumulative regrets, for numerical trace tests.
    #[must_use]
    pub fn regrets(&self, node: NodeId) -> Option<&[Real]> {
        let node = node as usize;
        (node < self.layout.num_nodes()).then(|| &self.regrets[self.layout.row_range(node)])
    }
    /// Read-only whole cumulative strategy sums, for numerical trace tests.
    #[must_use]
    pub fn strategy_sum(&self, node: NodeId) -> Option<&[Real]> {
        let node = node as usize;
        (node < self.layout.num_nodes()).then(|| &self.strategy_sum[self.layout.row_range(node)])
    }

    /// The cumulative strategy sums a best-response walk normalises as it reads
    /// them, so a measurement needs no retained average of its own.
    pub(crate) fn sums(&self) -> &[Real] {
        &self.strategy_sum
    }

    pub(crate) fn layout(&self) -> &Arc<TraversalLayout> {
        &self.layout
    }
}

impl Solver for Cfr {
    fn iteration(&self) -> u64 {
        self.iteration()
    }
    fn run_iteration(&mut self, game: &dyn Game) -> Result<(), SolveError> {
        self.run_iteration(game)
    }
    fn average_strategy(&self, game: &dyn Game) -> Result<Strategy, SolveError> {
        self.average_strategy(game)
    }
}

fn discount(
    regrets: &mut [Real],
    strategy_sum: &mut [Real],
    positive: Real,
    negative: Real,
    strategy: Real,
) {
    for regret in regrets {
        *regret *= if *regret >= 0.0 { positive } else { negative };
    }
    for sum in strategy_sum {
        *sum *= strategy;
    }
}

struct Traversal<'a> {
    terminal: &'a mut dyn TerminalEvaluator,
    layout: &'a TraversalLayout,
    /// Stored rows for the nodes this walk owns. A serial walk owns every node;
    /// a chance outcome's worker owns the contiguous block that outcome
    /// expanded into, and no other worker can reach it.
    regrets: &'a mut [Real],
    strategy_sum: &'a mut [Real],
    /// Node range those two slices cover.
    owned: Range<NodeId>,
    /// Entry index of `owned.start` in the whole-tree arrays.
    base_entry: usize,
    /// Reused buffers for the policy rows this walk derives.
    rows: RowPool,
    parallel: Option<&'a Parallel<'a>>,
    player: usize,
    iteration: u64,
    average_weight: Real,
}

impl Traversal<'_> {
    /// Where a node this walk owns keeps its row inside the slices above.
    ///
    /// A serial walk owns every node, so the offset is the layout's own. Inside
    /// a chance outcome's worker the slices start at that outcome's first node,
    /// and a node outside the block is a split that did not match the tree
    /// rather than a silent write into a neighbour's rows.
    fn local(&self, id: NodeId) -> Result<Range<usize>, SolveError> {
        if id < self.owned.start || id >= self.owned.end {
            return Err(SolveError::InvalidGame(format!(
                "node {id} is outside the rows this walk owns from node {}",
                self.owned.start
            )));
        }
        let range = self.layout.row_range(id as usize);
        Ok(range.start - self.base_entry..range.end - self.base_entry)
    }

    /// Regret matching over this node's own regrets, into a pooled buffer.
    ///
    /// The row is read before the same node's regrets are updated, so it is the
    /// row the previous iteration's stored policy held. Nothing is retained:
    /// the buffer goes back to the pool as soon as the node is done.
    fn policy_row(&mut self, id: NodeId, actions: usize) -> Result<Vec<Real>, SolveError> {
        let range = self.local(id)?;
        let mut row = self.rows.take(range.len())?;
        for (regrets, out) in self.regrets[range]
            .chunks_exact(actions)
            .zip(row.chunks_exact_mut(actions))
        {
            normalize_positive(regrets, out);
        }
        Ok(row)
    }

    /// Walks one chance node's outcomes on the context's workers.
    ///
    /// Each outcome gets the rows its own subtree owns, walks through the same
    /// `walk` any serial traversal uses, and returns its value vector.
    /// `collect` on an indexed parallel iterator preserves outcome order, so
    /// the sum below and the first error reported are the serial ones: the
    /// outcome with the lowest index that failed, never whichever worker failed
    /// first.
    #[allow(clippy::too_many_arguments)]
    fn chance_in_parallel(
        &mut self,
        context: &Parallel<'_>,
        id: NodeId,
        opponent: &[Real],
        own: &[Real],
        live: &[Real],
        out: &mut [Real],
    ) -> Result<(), SolveError> {
        let layout = self.layout;
        let player = self.player;
        let iteration = self.iteration;
        let average_weight = self.average_weight;
        let checked = context.terminal.checks_reach_underflow();
        let children = layout.children(id);
        let probabilities = layout.probabilities(id);
        let ranges = context.split(self.owned.clone(), children)?;
        let base_entry = self.base_entry;
        let mut regrets = &mut *self.regrets;
        let mut sums = &mut *self.strategy_sum;
        let mut consumed = base_entry;
        let mut parts = reserved(children.len())?;
        for (child, end) in &ranges {
            let range = layout.row_offsets[*child as usize] as usize
                ..layout.row_offsets[*end as usize] as usize;
            let skip = range.start - consumed;
            let take = range.end - range.start;
            let (_, tail) = regrets.split_at_mut(skip);
            let (own_regrets, rest) = tail.split_at_mut(take);
            regrets = rest;
            let (_, tail) = sums.split_at_mut(skip);
            let (own_sums, rest) = tail.split_at_mut(take);
            sums = rest;
            consumed = range.end;
            parts.push((*child, *end, range.start, own_regrets, own_sums));
        }
        let values: Vec<Result<Vec<Real>, SolveError>> = parts
            .into_par_iter()
            .enumerate()
            .map(|(outcome, (child, end, entry, own_regrets, own_sums))| {
                let masks = layout.masks(id, outcome);
                let next_opponent =
                    try_collect(opponent.iter().zip(&masks[1 - player]).map(|(r, m)| {
                        reach_product(
                            r * m,
                            probabilities[outcome],
                            checked,
                            iteration,
                            id,
                            1 - player,
                        )
                    }))?;
                let next_own_live = collect(live.iter().zip(&masks[player]).map(|(a, b)| a * b))?;
                let mut terminal = SharedRef(context.terminal);
                let mut traversal = Traversal {
                    terminal: &mut terminal,
                    layout,
                    regrets: own_regrets,
                    strategy_sum: own_sums,
                    owned: child..end,
                    base_entry: entry,
                    rows: RowPool::default(),
                    parallel: Some(context),
                    player,
                    iteration,
                    average_weight,
                };
                traversal.walk(child, &next_opponent, own, &next_own_live)
            })
            .collect();
        for outcome in values {
            for (value, add) in out.iter_mut().zip(outcome?) {
                *value += add;
            }
        }
        Ok(())
    }

    fn walk(
        &mut self,
        id: NodeId,
        opponent: &[Real],
        own: &[Real],
        live: &[Real],
    ) -> Result<Vec<Real>, SolveError> {
        let layout = self.layout;
        let kind = layout.kinds[id as usize];
        let mut out = filled(self.layout.states[self.player], 0.0)?;
        match kind {
            NodeKind::Terminal => {
                out.fill(Real::NAN);
                self.terminal.evaluate_terminal(
                    id,
                    self.player,
                    opponent,
                    &mut out,
                    self.iteration,
                )?;
                finite(&out, self.iteration, id, self.player)?;
                for (value, mask) in out.iter_mut().zip(live) {
                    *value *= mask;
                }
            }
            NodeKind::Chance { num_outcomes } => {
                // A chance node with a mask pool and more than one outcome is a
                // runout deal, and its outcomes own disjoint rows. With a
                // parallel context they go to the workers; the reduction below
                // is the same sum in the same outcome order either way.
                let deals = num_outcomes > 1 && layout.deals(id);
                if let Some(context) = self.parallel
                    && deals
                {
                    self.chance_in_parallel(context, id, opponent, own, live, &mut out)?;
                } else {
                    for outcome in 0..num_outcomes as usize {
                        let child = layout.children(id)[outcome];
                        let probability = layout.probabilities(id)[outcome];
                        let masks = layout.masks(id, outcome);
                        let next_opponent = try_collect(
                            opponent.iter().zip(&masks[1 - self.player]).map(|(r, m)| {
                                reach_product(
                                    r * m,
                                    probability,
                                    self.terminal.checks_reach_underflow(),
                                    self.iteration,
                                    id,
                                    1 - self.player,
                                )
                            }),
                        )?;
                        let next_own_live =
                            collect(live.iter().zip(&masks[self.player]).map(|(a, b)| a * b))?;
                        let values = self.walk(child, &next_opponent, own, &next_own_live)?;
                        for (value, add) in out.iter_mut().zip(values) {
                            *value += add;
                        }
                    }
                }
            }
            NodeKind::Player {
                player,
                num_actions,
            } => {
                let n = num_actions as usize;
                let row = self.policy_row(id, n)?;
                if player as usize == self.player {
                    let mut actions = reserved(n)?;
                    for action in 0..n {
                        let child = layout.children(id)[action];
                        let next_own = try_collect(own.iter().enumerate().map(|(h, r)| {
                            reach_product(
                                *r,
                                row[h * n + action],
                                self.terminal.checks_reach_underflow(),
                                self.iteration,
                                id,
                                self.player,
                            )
                        }))?;
                        let values = self.walk(child, opponent, &next_own, live)?;
                        for (h, (value, add)) in out.iter_mut().zip(&values).enumerate() {
                            *value += weighted_product(
                                *add,
                                row[h * n + action],
                                self.terminal.checks_reach_underflow(),
                                self.iteration,
                                id,
                                self.player,
                            )?;
                        }
                        actions.push(values);
                    }
                    let average_weight = self.average_weight;
                    let range = self.local(id)?;
                    let regrets = &mut self.regrets[range.clone()];
                    let sums = &mut self.strategy_sum[range];
                    for h in 0..out.len() {
                        for (action, values) in actions.iter().enumerate() {
                            let index = h * n + action;
                            regrets[index] += values[h] - out[h];
                            sums[index] += average_weight * own[h] * live[h] * row[index];
                        }
                    }
                } else {
                    for action in 0..n {
                        let child = layout.children(id)[action];
                        let next_opponent =
                            try_collect(opponent.iter().enumerate().map(|(h, r)| {
                                reach_product(
                                    *r,
                                    row[h * n + action],
                                    self.terminal.checks_reach_underflow(),
                                    self.iteration,
                                    id,
                                    1 - self.player,
                                )
                            }))?;
                        let values = self.walk(child, &next_opponent, own, live)?;
                        for (value, add) in out.iter_mut().zip(values) {
                            *value += add;
                        }
                    }
                }
                self.rows.give(row);
            }
        }
        finite(&out, self.iteration, id, self.player)?;
        Ok(out)
    }
}

fn validate_variant(variant: Variant) -> Result<(), SolveError> {
    if let Variant::Discounted { alpha, beta, gamma } = variant {
        for (name, value) in [("alpha", alpha), ("beta", beta), ("gamma", gamma)] {
            if !value.is_finite() || value < 0.0 {
                return Err(SolveError::Config(format!(
                    "dcfr.{name} must be finite and nonnegative"
                )));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discount_applies_to_whole_strategy_accumulator() {
        let mut regrets = vec![0.0; 2];
        let mut strategy_sum = vec![0.0; 2];
        for t in 1..=3 {
            let contribution = if t == 1 { [1.0, 0.0] } else { [0.0, 1.0] };
            for (sum, add) in strategy_sum.iter_mut().zip(contribution) {
                *sum += add;
            }
            let t = t as Real;
            discount(
                &mut regrets,
                &mut strategy_sum,
                1.0,
                0.5,
                (t / (t + 1.0)).powi(2),
            );
        }
        let mut row = [0.0; 2];
        normalize_positive(&strategy_sum, &mut row);
        assert!((row[0] - 1.0 / 14.0).abs() < 1e-15);
        assert!((row[0] - 36.0 / 181.0).abs() > 0.1);
    }
    #[test]
    fn negative_dcfr_regrets_decay_without_flooring() {
        let mut regrets = vec![-8.0, 4.0];
        let mut strategy_sum: Vec<Real> = Vec::new();
        for expected in [-4.0, -2.0, -1.0] {
            discount(&mut regrets, &mut strategy_sum, 1.0, 0.5, 1.0);
            assert_eq!(regrets[0], expected);
            let mut strategy = [0.0; 2];
            normalize_positive(&regrets, &mut strategy);
            assert_eq!(strategy, [0.0, 1.0]);
        }
    }

    /// Runouts in miniature: one chance node dealing `OUTCOMES` cards, each into
    /// its own decision and its own pair of terminals, numbered depth first the
    /// way `PostflopGame` expands a street. It is the smallest game that can
    /// tell a correct outcome split from a plausible one.
    const OUTCOMES: usize = 8;
    const STATES: usize = 3;

    struct Runouts;

    impl Runouts {
        /// Depth-first node IDs: outcome `k` owns `1 + 3k` and the two terminals
        /// under it, so its half-open range is `1 + 3k .. 4 + 3k`.
        fn decision(outcome: usize) -> NodeId {
            (1 + 3 * outcome) as NodeId
        }
        fn outcome_of(node: NodeId) -> usize {
            (node as usize - 1) / 3
        }
        /// Antisymmetric and linear in reach: own state minus opponent state,
        /// scaled per terminal so the two actions are not interchangeable.
        fn scale(node: NodeId) -> Real {
            1.0 + Real::from(node % 5)
        }
    }

    impl Game for Runouts {
        fn num_nodes(&self) -> usize {
            1 + 3 * OUTCOMES
        }
        fn root(&self) -> NodeId {
            0
        }
        fn kind(&self, node: NodeId) -> NodeKind {
            if node == 0 {
                NodeKind::Chance {
                    num_outcomes: OUTCOMES as u16,
                }
            } else if (node as usize - 1).is_multiple_of(3) {
                NodeKind::Player {
                    player: 0,
                    num_actions: 2,
                }
            } else {
                NodeKind::Terminal
            }
        }
        fn child(&self, node: NodeId, index: usize) -> NodeId {
            if node == 0 {
                Self::decision(index)
            } else {
                node + 1 + index as NodeId
            }
        }
        fn num_private_states(&self, _player: usize) -> usize {
            STATES
        }
        fn initial_weights(&self, _player: usize) -> &[Real] {
            &[1.0, 2.0, 3.0]
        }
        fn compatible(&self, _p0_state: usize, _p1_state: usize) -> bool {
            true
        }
        fn chance_prob(&self, _node: NodeId, _outcome: usize) -> Real {
            1.0 / OUTCOMES as Real
        }
        fn chance_mask(&self, _node: NodeId, _outcome: usize, _player: usize) -> &[Real] {
            &[1.0, 1.0, 1.0]
        }
        fn terminal_values(
            &self,
            node: NodeId,
            _player: usize,
            opp_reach: &[Real],
            out: &mut [Real],
        ) {
            let scale = Self::scale(node);
            for (own, value) in out.iter_mut().enumerate() {
                *value = opp_reach
                    .iter()
                    .enumerate()
                    .map(|(opponent, reach)| reach * scale * (own as Real - opponent as Real))
                    .sum();
            }
        }
        fn starting_pot(&self) -> Real {
            10.0
        }
        fn info_label(&self, node: NodeId, player: usize, state: usize) -> String {
            format!("n{node}/p{player}/s{state}")
        }
    }

    /// The depth-first ranges the real game reads off `PostflopGame::subtree`.
    struct RunoutRanges;

    impl crate::traversal::SubtreeRanges for RunoutRanges {
        fn end(&self, node: NodeId) -> Option<NodeId> {
            let total = (1 + 3 * OUTCOMES) as NodeId;
            match node {
                0 => Some(total),
                _ if node >= total => None,
                _ if (node as usize - 1).is_multiple_of(3) => Some(node + 3),
                _ => Some(node + 1),
            }
        }
    }

    /// The shared boundary, optionally poisoned inside named outcomes.
    struct SharedRunouts {
        game: Runouts,
        poisoned: &'static [usize],
    }

    impl crate::traversal::SharedTerminal for SharedRunouts {
        fn evaluate_terminal(
            &self,
            node: NodeId,
            player: usize,
            opponent: &[Real],
            output: &mut [Real],
            iteration: u64,
        ) -> Result<(), SolveError> {
            let outcome = Runouts::outcome_of(node);
            if self.poisoned.contains(&outcome) {
                return Err(SolveError::Terminal {
                    iteration,
                    node,
                    player,
                    reason: format!("poisoned outcome {outcome}"),
                });
            }
            self.game.terminal_values(node, player, opponent, output);
            Ok(())
        }
    }

    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a test thread pool")
    }

    fn solved(threads: Option<usize>, iterations: u64) -> Cfr {
        let game = Runouts;
        let audited = Arc::new(Layout::new(&game).expect("a valid tiny chance game"));
        let mut core = Cfr::from_layout(
            audited.traversal.clone(),
            Variant::Discounted {
                alpha: 1.5,
                beta: 0.0,
                gamma: 2.0,
            },
            None,
        )
        .expect("a CFR session");
        let terminal = SharedRunouts {
            game: Runouts,
            poisoned: &[],
        };
        let ranges = RunoutRanges;
        let workers = threads.map(pool);
        for _ in 0..iterations {
            match &workers {
                Some(pool) => core
                    .advance_parallel(&Parallel {
                        terminal: &terminal,
                        ranges: &ranges,
                        pool,
                    })
                    .expect("a parallel iteration"),
                None => core
                    .advance(&mut LegacyTerminal(&game))
                    .expect("a serial iteration"),
            }
        }
        core
    }

    #[test]
    fn spreading_outcomes_over_workers_changes_no_bit_of_the_result() {
        let serial = solved(None, 12);
        let nodes = 1 + 3 * OUTCOMES;
        for threads in [1, 2, 4, 7] {
            let parallel = solved(Some(threads), 12);
            assert_eq!(parallel.iteration(), serial.iteration());
            for id in 0..nodes as NodeId {
                // Bit patterns, not tolerances: the reduction order at the
                // chance node is outcome order however the workers finished.
                assert_eq!(
                    parallel.regrets(id).map(bits),
                    serial.regrets(id).map(bits),
                    "regrets at node {id} with {threads} workers"
                );
                assert_eq!(
                    parallel.strategy_sum(id).map(bits),
                    serial.strategy_sum(id).map(bits),
                    "strategy sums at node {id} with {threads} workers"
                );
            }
            let (average, expected) = (
                parallel.average_bound().unwrap(),
                serial.average_bound().unwrap(),
            );
            for id in 0..nodes as NodeId {
                assert_eq!(
                    average.row(id).map(bits),
                    expected.row(id).map(bits),
                    "average row at node {id}"
                );
            }
        }
    }

    fn bits(values: &[Real]) -> Vec<u64> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    #[test]
    fn a_failed_outcome_is_reported_by_the_lowest_outcome_index() {
        let game = Runouts;
        let audited = Arc::new(Layout::new(&game).unwrap());
        let terminal = SharedRunouts {
            game: Runouts,
            // Outcome 7 is walked by another worker while outcome 3 is still
            // running, so "whichever failed first" and "the first by outcome"
            // are different answers.
            poisoned: &[7, 3],
        };
        let ranges = RunoutRanges;
        for threads in [1, 2, 4] {
            let pool = pool(threads);
            let mut core =
                Cfr::from_layout(audited.traversal.clone(), Variant::Plus, None).unwrap();
            let failure = core
                .advance_parallel(&Parallel {
                    terminal: &terminal,
                    ranges: &ranges,
                    pool: &pool,
                })
                .unwrap_err();
            match &failure {
                SolveError::Terminal { node, reason, .. } => {
                    assert_eq!(Runouts::outcome_of(*node), 3, "{threads} workers: {reason}");
                }
                other => panic!("{threads} workers: expected a terminal failure, got {other}"),
            }
            // The iteration is discarded whole: no partial strategy, no partial
            // regrets, and the poisoning survives a healthy call afterwards.
            assert_eq!(core.iteration(), 0);
            assert!(core.current_strategy().is_err());
            assert_eq!(core.average_bound().unwrap_err(), failure);
            assert_eq!(core.advance(&mut LegacyTerminal(&game)), Err(failure));
        }
    }

    #[test]
    fn an_outcome_range_that_does_not_match_the_tree_is_refused() {
        /// Ranges that overlap: every outcome claims the whole subtree, which
        /// would hand the same regret rows to several workers at once. The
        /// second outcome starts before the first one ended, so this is caught
        /// as an ordering failure.
        struct Overlapping;
        impl crate::traversal::SubtreeRanges for Overlapping {
            fn end(&self, _node: NodeId) -> Option<NodeId> {
                Some((1 + 3 * OUTCOMES) as NodeId)
            }
        }
        /// A range that is ordered but runs past the nodes the parent owns.
        struct TooLong;
        impl crate::traversal::SubtreeRanges for TooLong {
            fn end(&self, node: NodeId) -> Option<NodeId> {
                Some(node + 1000)
            }
        }
        /// Ranges that own nothing, which would leave an outcome's decision
        /// nodes with no rows to write and no complaint about it.
        struct Empty;
        impl crate::traversal::SubtreeRanges for Empty {
            fn end(&self, node: NodeId) -> Option<NodeId> {
                Some(node)
            }
        }
        /// A tree that does not recognise the node it was asked about.
        struct Unknown;
        impl crate::traversal::SubtreeRanges for Unknown {
            fn end(&self, _node: NodeId) -> Option<NodeId> {
                None
            }
        }
        let game = Runouts;
        let audited = Arc::new(Layout::new(&game).unwrap());
        let terminal = SharedRunouts {
            game: Runouts,
            poisoned: &[],
        };
        let pool = pool(2);
        let cases: [(&dyn crate::traversal::SubtreeRanges, &str); 4] = [
            (&Overlapping, "non-empty and increasing"),
            (&TooLong, "leaves the parent's own nodes"),
            (&Empty, "non-empty and increasing"),
            (&Unknown, "does not know this node"),
        ];
        for (ranges, expected) in cases {
            let mut core =
                Cfr::from_layout(audited.traversal.clone(), Variant::Plus, None).unwrap();
            let error = core
                .advance_parallel(&Parallel {
                    terminal: &terminal,
                    ranges,
                    pool: &pool,
                })
                .unwrap_err();
            let message = error.to_string();
            assert!(message.contains("private accumulator slice"), "{message}");
            assert!(message.contains(expected), "{message}");
            // A refused split is a failed iteration like any other.
            assert_eq!(core.iteration(), 0);
            assert!(core.current_strategy().is_err());
        }
    }
}
