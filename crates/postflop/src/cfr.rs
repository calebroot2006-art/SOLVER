//! Alternating CFR, regret-matching+, and signed Discounted CFR.
use crate::allocation::{collect, filled, reserved, try_collect};
use crate::error::{reach_product, weighted_product};
use crate::{
    Game, NodeId, NodeKind, Real, SolveError, Solver, Strategy,
    error::finite,
    game::{Layout, Node, TraversalLayout},
    traversal::{LegacyTerminal, Parallel, SharedRef, TerminalEvaluator},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
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

#[derive(Clone, Debug)]
struct Accumulator {
    regrets: Vec<Real>,
    strategy_sum: Vec<Real>,
}

/// CFR state bound to one immutable public-tree layout.
/// A failed numerical update poisons the solver: subsequent updates and average
/// reads return the original error instead of exposing a partial iteration.
#[derive(Clone, Debug)]
pub struct Cfr {
    layout: Arc<TraversalLayout>,
    current: Strategy,
    accumulators: Vec<Accumulator>,
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
        let current = Strategy::uniform_layout(layout.clone(), legacy_binding)?;
        let mut accumulators = reserved(current.rows.len())?;
        for row in &current.rows {
            accumulators.push(Accumulator {
                regrets: filled(row.len(), 0.0)?,
                strategy_sum: filled(row.len(), 0.0)?,
            });
        }
        Ok(Self {
            layout,
            current,
            accumulators,
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

    /// Runs a player-zero update followed by a player-one update.
    pub fn run_iteration(&mut self, game: &dyn Game) -> Result<(), SolveError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.current.check_game(game)?;
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
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
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
                strategy: &self.current,
                accumulators: &mut self.accumulators,
                base: 0,
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
            for (id, accumulator) in self.accumulators.iter_mut().enumerate() {
                if let NodeKind::Player { player: actor, .. } = self.layout.nodes[id].kind
                    && actor as usize == player
                {
                    if self.variant == Variant::Plus {
                        for regret in &mut accumulator.regrets {
                            *regret = regret.max(0.0);
                        }
                    }
                    finite(&accumulator.regrets, iteration, id as NodeId, player)?;
                    finite(&accumulator.strategy_sum, iteration, id as NodeId, player)?;
                }
            }
            self.refresh_player(player)?;
        }
        if let Variant::Discounted { alpha, beta, gamma } = self.variant {
            let t = iteration as Real;
            // Reciprocal form avoids infinity/infinity for large finite exponents.
            let positive = 1.0 / (1.0 + t.powf(-alpha));
            let negative = 1.0 / (1.0 + t.powf(-beta));
            let strategy = (t / (t + 1.0)).powf(gamma);
            for accumulator in &mut self.accumulators {
                discount(accumulator, positive, negative, strategy);
            }
            self.refresh_player(0)?;
            self.refresh_player(1)?;
        }
        for (id, accumulator) in self.accumulators.iter().enumerate() {
            if let NodeKind::Player { player, .. } = self.layout.nodes[id].kind {
                finite(
                    &accumulator.regrets,
                    iteration,
                    id as NodeId,
                    player as usize,
                )?;
                finite(
                    &accumulator.strategy_sum,
                    iteration,
                    id as NodeId,
                    player as usize,
                )?;
            }
        }
        Ok(())
    }

    fn refresh_player(&mut self, player: usize) -> Result<(), SolveError> {
        for (id, node) in self.layout.nodes.iter().enumerate() {
            if let NodeKind::Player {
                player: actor,
                num_actions,
            } = node.kind
                && actor as usize == player
            {
                for (regrets, policy) in self.accumulators[id]
                    .regrets
                    .chunks_exact(num_actions as usize)
                    .zip(self.current.rows[id].chunks_exact_mut(num_actions as usize))
                {
                    normalize_positive(regrets, policy);
                }
                finite(
                    &self.current.rows[id],
                    self.iteration + 1,
                    id as NodeId,
                    player,
                )?;
            }
        }
        Ok(())
    }

    /// Returns the reach-weighted average, rejecting failed or mismatched games.
    pub fn average_strategy(&self, game: &dyn Game) -> Result<Strategy, SolveError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.current.check_game(game)?;
        self.average_bound()
    }

    pub(crate) fn average_bound(&self) -> Result<Strategy, SolveError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let mut strategy =
            Strategy::uniform_layout(self.layout.clone(), self.current.legacy_binding.clone())?;
        for (id, node) in self.layout.nodes.iter().enumerate() {
            if let NodeKind::Player { num_actions, .. } = node.kind {
                for (sum, row) in self.accumulators[id]
                    .strategy_sum
                    .chunks_exact(num_actions as usize)
                    .zip(strategy.rows[id].chunks_exact_mut(num_actions as usize))
                {
                    normalize_positive(sum, row);
                }
            }
        }
        strategy.validate_rows()?;
        Ok(strategy)
    }

    /// Current policy for diagnostics. Use the average for convergence claims.
    /// A failed iteration returns its error instead of a partial policy.
    pub fn current_strategy(&self) -> Result<&Strategy, SolveError> {
        if let Some(error) = &self.failure {
            Err(error.clone())
        } else {
            Ok(&self.current)
        }
    }

    /// Read-only signed cumulative regrets, for numerical trace tests.
    #[must_use]
    pub fn regrets(&self, node: NodeId) -> Option<&[Real]> {
        self.accumulators
            .get(node as usize)
            .map(|a| a.regrets.as_slice())
    }
    /// Read-only whole cumulative strategy sums, for numerical trace tests.
    #[must_use]
    pub fn strategy_sum(&self, node: NodeId) -> Option<&[Real]> {
        self.accumulators
            .get(node as usize)
            .map(|a| a.strategy_sum.as_slice())
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

fn normalize_positive(values: &[Real], out: &mut [Real]) {
    let sum: Real = values.iter().map(|value| value.max(0.0)).sum();
    if sum > 0.0 && sum.is_finite() {
        for (value, target) in values.iter().zip(out) {
            *target = value.max(0.0) / sum;
        }
    } else if sum == 0.0 {
        let uniform = 1.0 / out.len() as Real;
        out.fill(uniform);
    } else {
        // Scale before summing when finite positive entries overflow their sum.
        let scale = values.iter().copied().fold(0.0, Real::max);
        let scaled_sum: Real = values.iter().map(|v| v.max(0.0) / scale).sum();
        for (value, target) in values.iter().zip(out) {
            *target = (value.max(0.0) / scale) / scaled_sum;
        }
    }
}

fn discount(accumulator: &mut Accumulator, positive: Real, negative: Real, strategy: Real) {
    for regret in &mut accumulator.regrets {
        *regret *= if *regret >= 0.0 { positive } else { negative };
    }
    for sum in &mut accumulator.strategy_sum {
        *sum *= strategy;
    }
}

struct Traversal<'a> {
    terminal: &'a mut dyn TerminalEvaluator,
    layout: &'a TraversalLayout,
    strategy: &'a Strategy,
    /// Accumulators for the nodes this walk owns, starting at node `base`.
    /// A serial walk owns all of them; a chance outcome's worker owns only the
    /// contiguous block that outcome expanded into.
    accumulators: &'a mut [Accumulator],
    /// Node ID of `accumulators[0]`, zero for a walk that owns the whole tree.
    base: NodeId,
    parallel: Option<&'a Parallel<'a>>,
    player: usize,
    iteration: u64,
    average_weight: Real,
}

impl Traversal<'_> {
    /// This walk's accumulator for a node it owns.
    ///
    /// A serial walk owns every node, so `base` is zero and the index is the
    /// node ID. Inside a chance outcome's worker the slice starts at that
    /// outcome's first node, and a node outside the block is a split that did
    /// not match the tree rather than a silent write into a neighbour's rows.
    fn accumulator(&mut self, id: NodeId) -> Result<&mut Accumulator, SolveError> {
        let base = self.base;
        id.checked_sub(base)
            .and_then(|offset| self.accumulators.get_mut(offset as usize))
            .ok_or_else(|| {
                SolveError::InvalidGame(format!(
                    "node {id} is outside the accumulators this walk owns from node {base}"
                ))
            })
    }

    /// Walks one chance node's outcomes on the context's workers.
    ///
    /// Each outcome gets the accumulators its own subtree expanded into, walks
    /// through the same `walk` any serial traversal uses, and returns its value
    /// vector. `collect` on an indexed parallel iterator preserves outcome
    /// order, so the sum below and the first error reported are the serial
    /// ones: the outcome with the lowest index that failed, never whichever
    /// worker failed first.
    #[allow(clippy::too_many_arguments)]
    fn chance_in_parallel(
        &mut self,
        context: &Parallel<'_>,
        id: NodeId,
        node: &Node,
        opponent: &[Real],
        own: &[Real],
        live: &[Real],
        out: &mut [Real],
    ) -> Result<(), SolveError> {
        let layout = self.layout;
        let strategy = self.strategy;
        let player = self.player;
        let iteration = self.iteration;
        let average_weight = self.average_weight;
        let checked = context.terminal.checks_reach_underflow();
        let parts = context.split(self.base, &node.children, &mut *self.accumulators)?;
        let values: Vec<Result<Vec<Real>, SolveError>> = parts
            .into_par_iter()
            .enumerate()
            .map(|(outcome, (child, accumulators))| {
                let masks = layout.masks(node, outcome);
                let next_opponent =
                    try_collect(opponent.iter().zip(&masks[1 - player]).map(|(r, m)| {
                        reach_product(
                            r * m,
                            node.probabilities[outcome],
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
                    strategy,
                    accumulators,
                    base: child,
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
        let strategy = self.strategy;
        let node = &layout.nodes[id as usize];
        let mut out = filled(self.layout.states[self.player], 0.0)?;
        match node.kind {
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
                // runout deal, and its outcomes own disjoint accumulators. With
                // a parallel context they go to the workers; the reduction
                // below is the same sum in the same outcome order either way.
                if let Some(context) = self.parallel
                    && num_outcomes > 1
                    && !node.masks.is_empty()
                {
                    self.chance_in_parallel(context, id, node, opponent, own, live, &mut out)?;
                } else {
                    for (outcome, child) in node.children.iter().enumerate() {
                        let masks = layout.masks(node, outcome);
                        let next_opponent = try_collect(
                            opponent.iter().zip(&masks[1 - self.player]).map(|(r, m)| {
                                reach_product(
                                    r * m,
                                    node.probabilities[outcome],
                                    self.terminal.checks_reach_underflow(),
                                    self.iteration,
                                    id,
                                    1 - self.player,
                                )
                            }),
                        )?;
                        let next_own_live =
                            collect(live.iter().zip(&masks[self.player]).map(|(a, b)| a * b))?;
                        let values = self.walk(*child, &next_opponent, own, &next_own_live)?;
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
                let row = &strategy.rows[id as usize];
                if player as usize == self.player {
                    let mut actions = reserved(n)?;
                    for (action, child) in node.children.iter().enumerate() {
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
                        let values = self.walk(*child, opponent, &next_own, live)?;
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
                    let accumulator = self.accumulator(id)?;
                    for h in 0..out.len() {
                        for (action, values) in actions.iter().enumerate() {
                            let index = h * n + action;
                            accumulator.regrets[index] += values[h] - out[h];
                            accumulator.strategy_sum[index] +=
                                average_weight * own[h] * live[h] * row[index];
                        }
                    }
                } else {
                    for (action, child) in node.children.iter().enumerate() {
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
                        let values = self.walk(*child, &next_opponent, own, live)?;
                        for (value, add) in out.iter_mut().zip(values) {
                            *value += add;
                        }
                    }
                }
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
        let mut accumulator = Accumulator {
            regrets: vec![0.0; 2],
            strategy_sum: vec![0.0; 2],
        };
        for t in 1..=3 {
            let contribution = if t == 1 { [1.0, 0.0] } else { [0.0, 1.0] };
            for (sum, add) in accumulator.strategy_sum.iter_mut().zip(contribution) {
                *sum += add;
            }
            let t = t as Real;
            discount(&mut accumulator, 1.0, 0.5, (t / (t + 1.0)).powi(2));
        }
        let mut row = [0.0; 2];
        normalize_positive(&accumulator.strategy_sum, &mut row);
        assert!((row[0] - 1.0 / 14.0).abs() < 1e-15);
        assert!((row[0] - 36.0 / 181.0).abs() > 0.1);
    }
    #[test]
    fn negative_dcfr_regrets_decay_without_flooring() {
        let mut accumulator = Accumulator {
            regrets: vec![-8.0, 4.0],
            strategy_sum: vec![],
        };
        for expected in [-4.0, -2.0, -1.0] {
            discount(&mut accumulator, 1.0, 0.5, 1.0);
            assert_eq!(accumulator.regrets[0], expected);
            let mut strategy = [0.0; 2];
            normalize_positive(&accumulator.regrets, &mut strategy);
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
            for (id, (row, want)) in average.rows().iter().zip(expected.rows()).enumerate() {
                assert_eq!(bits(row), bits(want), "average row at node {id}");
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
        /// would hand the same regret rows to several workers at once.
        struct Overlapping;
        impl crate::traversal::SubtreeRanges for Overlapping {
            fn end(&self, _node: NodeId) -> Option<NodeId> {
                Some((1 + 3 * OUTCOMES) as NodeId)
            }
        }
        let game = Runouts;
        let audited = Arc::new(Layout::new(&game).unwrap());
        let mut core = Cfr::from_layout(audited.traversal.clone(), Variant::Plus, None).unwrap();
        let terminal = SharedRunouts {
            game: Runouts,
            poisoned: &[],
        };
        let pool = pool(2);
        let error = core
            .advance_parallel(&Parallel {
                terminal: &terminal,
                ranges: &Overlapping,
                pool: &pool,
            })
            .unwrap_err();
        assert!(
            error.to_string().contains("private accumulator slice"),
            "{error}"
        );
    }
}
