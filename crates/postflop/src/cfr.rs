//! Alternating CFR, regret-matching+, and signed Discounted CFR.
use crate::{
    Game, NodeId, NodeKind, Real, SolveError, Solver, Strategy, error::finite,
    game::{Layout, TraversalLayout}, traversal::{LegacyTerminal, TerminalEvaluator},
};
use std::sync::Arc;
use crate::allocation::{collect, filled, reserved, try_collect};
use crate::error::{reach_product, weighted_product};

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

    pub(crate) fn advance(&mut self, terminal: &mut dyn TerminalEvaluator) -> Result<(), SolveError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let next = self
            .iteration
            .checked_add(1)
            .ok_or_else(|| SolveError::Config("iteration counter overflow".into()))?;
        let result = self.update(terminal, next);
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

    fn update(&mut self, terminal: &mut dyn TerminalEvaluator, iteration: u64) -> Result<(), SolveError> {
        let average_weight = if self.variant == Variant::Plus {
            iteration as Real
        } else {
            1.0
        };
        for player in 0..2 {
            let own = filled(self.layout.states[player], 1.0)?;
            let live = collect(self.layout.weights[player]
                .iter()
                .map(|w| if *w > 0.0 { 1.0 } else { 0.0 }))?;
            let mut traversal = Traversal {
                terminal,
                layout: &self.layout,
                strategy: &self.current,
                accumulators: &mut self.accumulators,
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
        let mut strategy = Strategy::uniform_layout(self.layout.clone(), self.current.legacy_binding.clone())?;
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
    accumulators: &'a mut [Accumulator],
    player: usize,
    iteration: u64,
    average_weight: Real,
}

impl Traversal<'_> {
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
                self.terminal.evaluate_terminal(id, self.player, opponent, &mut out, self.iteration)?;
                finite(&out, self.iteration, id, self.player)?;
                for (value, mask) in out.iter_mut().zip(live) {
                    *value *= mask;
                }
            }
            NodeKind::Chance { .. } => {
                for (outcome, child) in node.children.iter().enumerate() {
                    let next_opponent = try_collect(opponent
                        .iter()
                        .zip(&node.masks[outcome][1 - self.player])
                        .map(|(r, m)| reach_product(r * m, node.probabilities[outcome], self.terminal.checks_reach_underflow(), self.iteration, id, 1 - self.player)))?;
                    let next_own_live = collect(live
                        .iter()
                        .zip(&node.masks[outcome][self.player])
                        .map(|(a, b)| a * b))?;
                    let values = self.walk(*child, &next_opponent, own, &next_own_live)?;
                    for (value, add) in out.iter_mut().zip(values) {
                        *value += add;
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
                        let next_own = try_collect(own
                            .iter()
                            .enumerate()
                            .map(|(h, r)| reach_product(*r, row[h * n + action], self.terminal.checks_reach_underflow(), self.iteration, id, self.player)))?;
                        let values = self.walk(*child, opponent, &next_own, live)?;
                        for (h, (value, add)) in out.iter_mut().zip(&values).enumerate() {
                            *value += weighted_product(*add, row[h * n + action], self.terminal.checks_reach_underflow(), self.iteration, id, self.player)?;
                        }
                        actions.push(values);
                    }
                    let accumulator = &mut self.accumulators[id as usize];
                    for h in 0..out.len() {
                        for (action, values) in actions.iter().enumerate() {
                            let index = h * n + action;
                            accumulator.regrets[index] += values[h] - out[h];
                            accumulator.strategy_sum[index] +=
                                self.average_weight * own[h] * live[h] * row[index];
                        }
                    }
                } else {
                    for (action, child) in node.children.iter().enumerate() {
                        let next_opponent = try_collect(opponent
                            .iter()
                            .enumerate()
                            .map(|(h, r)| reach_product(*r, row[h * n + action], self.terminal.checks_reach_underflow(), self.iteration, id, 1 - self.player)))?;
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
}
