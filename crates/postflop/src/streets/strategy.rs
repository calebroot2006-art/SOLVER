//! Read-only postflop policies and the conditional values read off them.
//!
//! A postflop history spans more than one street, so a policy row means nothing
//! without the cards that were dealt to reach it. Every query here reports the
//! board and the runout beside the numbers, and a decision-value report carries
//! the acting player's own reach and the compatible opponent mass, so a
//! consumer cannot quote a river frequency without the context that says how
//! often the history happens.

use super::STATES;
use super::game::PostflopGame;
use super::terminal::PostflopTerminal;
use crate::memory::Lease;
use crate::{
    Exploitability, NodeId, Real, SolveError, Strategy,
    allocation::{collect, filled, reserved},
    best_response::{evaluate, exploitability_bound, walk},
    error::{finite, reach_product},
    terminal::{ShowdownScratch, evaluate_fold},
};
use cards::{Card, Combo};
use std::sync::Arc;
use tree::{PostflopNodeKind, Street};

/// Read-only average or imported policy retaining its game and reservation.
#[derive(Debug)]
pub struct PostflopStrategy {
    game: PostflopGame,
    policy: Strategy,
    _lease: Lease,
}

/// Conditional net-chip action EVs at one decision history, with its context.
#[derive(Debug)]
pub struct PostflopDecisionValues {
    player: usize,
    actions: usize,
    street: Street,
    board: Vec<Card>,
    runout_len: usize,
    values: Vec<Option<f64>>,
    own_reach: Vec<f64>,
    opponent_mass: Vec<f64>,
    _lease: Lease,
}

impl PostflopDecisionValues {
    /// Acting player, zero for out of position.
    #[must_use]
    pub fn player(&self) -> usize {
        self.player
    }
    /// Number of actions per canonical combo.
    #[must_use]
    pub fn action_count(&self) -> usize {
        self.actions
    }
    /// Street this history belongs to.
    #[must_use]
    pub fn street(&self) -> Street {
        self.street
    }
    /// Every board card known at this history, in deal order.
    #[must_use]
    pub fn board(&self) -> &[Card] {
        &self.board
    }
    /// Board cards dealt after the root, in deal order.
    #[must_use]
    pub fn runout(&self) -> &[Card] {
        &self.board[self.board.len() - self.runout_len..]
    }
    /// State-major EVs: combo ID times action count plus action index.
    /// Missing values identify a zero-range, zero-reach or blocked decision.
    #[must_use]
    pub fn values(&self) -> &[Option<f64>] {
        &self.values
    }
    /// Acting range times their own preceding action probabilities and masks.
    #[must_use]
    pub fn own_reach(&self) -> &[f64] {
        &self.own_reach
    }
    /// Compatible opponent range, action mass and chance factors, unnormalized.
    #[must_use]
    pub fn opponent_mass(&self) -> &[f64] {
        &self.opponent_mass
    }
}

impl PostflopStrategy {
    pub(super) fn bind(game: PostflopGame, policy: Strategy, lease: Lease) -> Self {
        Self {
            game,
            policy,
            _lease: lease,
        }
    }

    pub(super) fn policy(&self) -> &Strategy {
        &self.policy
    }

    /// Build a checked uniform policy with this game's immutable identity.
    pub fn uniform(game: &PostflopGame) -> Result<Self, SolveError> {
        let lease = game
            .inner
            .budget
            .reserve(game.inner.memory.snapshot_bytes)?;
        let policy = Strategy::uniform_layout(game.inner.layout.clone(), None)?;
        Ok(Self::bind(game.clone(), policy, lease))
    }

    /// Validate imported canonical state-major rows for this exact game.
    /// All retained buffer capacities are charged to the shared game budget.
    pub fn from_rows(game: &PostflopGame, rows: Vec<Vec<f64>>) -> Result<Self, SolveError> {
        let input = Strategy {
            layout: game.inner.layout.clone(),
            legacy_binding: None,
            rows,
        };
        input.validate_rows()?;
        let capacity_error =
            || SolveError::Allocation("imported strategy capacity overflow".into());
        let mut bytes = input
            .rows
            .capacity()
            .checked_mul(std::mem::size_of::<Vec<f64>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Strategy>() + 256))
            .ok_or_else(capacity_error)?;
        for row in &input.rows {
            bytes = row
                .capacity()
                .checked_mul(std::mem::size_of::<f64>())
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(capacity_error)?;
        }
        let lease = game.inner.budget.reserve(bytes)?;
        Ok(Self::bind(game.clone(), input, lease))
    }

    /// Retained game and provenance for this policy.
    #[must_use]
    pub fn game(&self) -> &PostflopGame {
        &self.game
    }
    /// True only for clones of the exact owned game, including its ranges.
    #[must_use]
    pub fn is_bound_to(&self, game: &PostflopGame) -> bool {
        Arc::ptr_eq(&self.game.inner, &game.inner)
    }
    /// Every flattened state-major row; chance and terminal rows are empty.
    #[must_use]
    pub fn rows(&self) -> &[Vec<f64>] {
        self.policy.rows()
    }
    /// Action probabilities for a live combo at an expanded decision node.
    ///
    /// `None` when the node is not a decision, when the combo has no weight, or
    /// when a board card in this history blocks it.
    #[must_use]
    pub fn row(&self, node: NodeId, combo: Combo) -> Option<&[f64]> {
        let view = self.game.node(node)?;
        let PostflopNodeKind::Decision { player } = view.kind() else {
            return None;
        };
        let id = usize::from(combo.id());
        if self.game.inner.layout.weights[player as usize][id] == 0.0 {
            return None;
        }
        if view
            .board()
            .iter()
            .any(|card| combo.mask() & card.mask() != 0)
        {
            return None;
        }
        let n = view.actions().len();
        Some(&self.policy.rows()[node as usize][id * n..(id + 1) * n])
    }
    /// Expected net chips under this complete average policy.
    pub fn expected_value(&self, player: usize) -> Result<f64, SolveError> {
        self.value(player, false)
    }
    /// Maximum net chips against the opponent, maximizing per own information set.
    pub fn best_response(&self, player: usize) -> Result<f64, SolveError> {
        self.value(player, true)
    }
    /// Measure exploitability within these ranges, this tree and zero-rake model.
    /// The best-response walk covers every runout in f64, merged or not.
    pub fn exploitability(&self) -> Result<Exploitability, SolveError> {
        let _workspace = self.reserve_workspace()?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        exploitability_bound(
            &mut PostflopTerminal {
                game: &self.game.inner,
                scratch: &mut scratch[0],
            },
            &self.policy,
        )
    }

    /// One traversal buffer set and one terminal scratch: a query walks the
    /// tree serially whatever the game's worker count is, and the estimate's
    /// bound covers one of these on top of a solver iteration's per-worker set.
    fn reserve_workspace(&self) -> Result<Lease, SolveError> {
        let memory = self.game.inner.memory;
        self.game
            .inner
            .budget
            .reserve(memory.traversal_bytes + memory.scratch_bytes)
    }

    fn value(&self, player: usize, maximize: bool) -> Result<f64, SolveError> {
        let _workspace = self.reserve_workspace()?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        evaluate(
            &mut PostflopTerminal {
                game: &self.game.inner,
                scratch: &mut scratch[0],
            },
            &self.policy,
            player,
            maximize,
        )
    }

    /// Conditional action EVs after the specified public history, following this
    /// policy thereafter. Values use the fixed root origin; they are not
    /// per-hand error bounds. Impossible, blocked and zero-reach decisions are
    /// unavailable, and the report carries the reach that says how rare the
    /// history is.
    pub fn decision_values(&self, node: NodeId) -> Result<PostflopDecisionValues, SolveError> {
        let game = &self.game.inner;
        let view = self
            .game
            .node(node)
            .ok_or_else(|| SolveError::InvalidGame("unknown postflop node".into()))?;
        let PostflopNodeKind::Decision { player } = view.kind() else {
            return Err(SolveError::InvalidGame(
                "action EV query requires a decision node".into(),
            ));
        };
        let player = player as usize;
        let lease = game.budget.reserve(game.memory.decision_bytes)?;
        let _workspace = self.reserve_workspace()?;

        let mut path = reserved(game.tree.max_depth() + 1)?;
        let mut cursor = node;
        while let Some((parent, index)) = game.parents[cursor as usize] {
            path.push((parent, index));
            cursor = parent;
        }
        let mut reaches = [
            collect(game.layout.weights[0].iter().copied())?,
            collect(game.layout.weights[1].iter().copied())?,
        ];
        let mut live = filled(STATES, 1.0_f64)?;
        let mut chance_weight = 1.0_f64;
        for (parent, index) in path.into_iter().rev() {
            let ancestor = &game.layout.nodes[parent as usize];
            match game
                .compact(parent)
                .ok_or_else(|| SolveError::InvalidGame("unknown ancestor node".into()))?
                .kind()
            {
                PostflopNodeKind::Decision { player: actor } => {
                    let n = ancestor.children.len();
                    for (id, reach) in reaches[actor as usize].iter_mut().enumerate() {
                        *reach = reach_product(
                            *reach,
                            self.policy.rows()[parent as usize][id * n + index],
                            true,
                            0,
                            parent,
                            actor as usize,
                        )?;
                    }
                }
                PostflopNodeKind::Chance { .. } => {
                    let masks = game.layout.masks(ancestor, index);
                    for (side, mask) in reaches.iter_mut().zip(masks) {
                        for (reach, entry) in side.iter_mut().zip(mask) {
                            *reach *= entry;
                        }
                    }
                    for (entry, mask) in live.iter_mut().zip(&masks[player]) {
                        *entry *= mask;
                    }
                    chance_weight *= ancestor.probabilities[index];
                }
                PostflopNodeKind::Terminal(_) => {
                    return Err(SolveError::InvalidGame(
                        "terminal ancestor in postflop tree".into(),
                    ));
                }
            }
        }
        let opponent: Vec<Real> = collect(
            reaches[1 - player]
                .iter()
                .map(|reach| reach * chance_weight),
        )?;
        let opponent: &[f64; STATES] = opponent.as_slice().try_into().expect("fixed combo vector");
        let dead = game
            .payoff(node)
            .ok_or_else(|| SolveError::InvalidGame("unknown postflop node".into()))?
            .dead;
        let mut mass = [0.0; STATES];
        evaluate_fold(dead, opponent, 1.0, &mut mass).map_err(|e| SolveError::Terminal {
            iteration: 0,
            node,
            player,
            reason: e.to_string(),
        })?;

        let n = view.actions().len();
        let mut values = filled(STATES * n, None)?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        for (action, child) in view.children().iter().enumerate() {
            let conditional = walk(
                &mut PostflopTerminal {
                    game,
                    scratch: &mut scratch[0],
                },
                &self.policy,
                *child,
                player,
                opponent,
                &live,
                false,
            )?;
            for id in 0..STATES {
                if reaches[player][id] > 0.0 && mass[id] > 0.0 {
                    let value = conditional[id] / mass[id];
                    if conditional[id] != 0.0 && value == 0.0 {
                        return Err(SolveError::Arithmetic {
                            iteration: 0,
                            node,
                            player,
                            reason: "conditional action value underflow",
                        });
                    }
                    finite(&[value], 0, node, player)?;
                    values[id * n + action] = Some(value);
                }
            }
        }
        let board = collect(view.board().iter().copied())?;
        let runout_len = view.runout().len();
        let [zero, one] = reaches;
        let own_reach = if player == 0 { zero } else { one };
        Ok(PostflopDecisionValues {
            player,
            actions: n,
            street: view.street(),
            board,
            runout_len,
            values,
            own_reach,
            opponent_mass: collect(mass.into_iter())?,
            _lease: lease,
        })
    }
}
