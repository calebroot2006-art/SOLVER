use super::{RiverGame, game::RiverTerminal, memory::Lease};
use crate::{
    Exploitability, NodeId, SolveError, Strategy,
    allocation::{collect, filled, reserved},
    best_response::{evaluate, exploitability_bound, walk},
    error::finite,
    terminal::{ShowdownScratch, evaluate_fold},
};
use cards::Combo;
use std::sync::Arc;
use tree::RiverNodeKind;

/// Read-only average or imported policy retaining its immutable game and memory reservation.
#[derive(Debug)]
pub struct RiverStrategy {
    pub(super) game: RiverGame,
    pub(super) policy: Strategy,
    pub(super) _lease: Lease,
}

/// Conditional net-chip action EVs at one decision history.
#[derive(Debug)]
pub struct DecisionValues {
    player: usize,
    actions: usize,
    values: Vec<Option<f64>>,
    own_reach: Vec<f64>,
    opponent_mass: Vec<f64>,
    _lease: Lease,
}

impl DecisionValues {
    /// Acting player, zero for out of position.
    pub fn player(&self) -> usize {
        self.player
    }
    /// Number of actions per canonical combo.
    pub fn action_count(&self) -> usize {
        self.actions
    }
    /// State-major EVs: combo ID times action count plus action index.
    /// Missing values identify a zero-range/zero-reach or impossible decision.
    pub fn values(&self) -> &[Option<f64>] {
        &self.values
    }
    /// Acting range times their own preceding action probabilities.
    pub fn own_reach(&self) -> &[f64] {
        &self.own_reach
    }
    /// Compatible opponent range and preceding action mass, before normalization.
    pub fn opponent_mass(&self) -> &[f64] {
        &self.opponent_mass
    }
}

impl RiverStrategy {
    /// Build a checked uniform policy with this game's immutable identity.
    pub fn uniform(game: &RiverGame) -> Result<Self, SolveError> {
        let lease = game
            .inner
            .budget
            .reserve(game.inner.memory.snapshot_bytes)?;
        let policy = Strategy::uniform_layout(game.inner.layout.clone(), None)?;
        Ok(Self {
            game: game.clone(),
            policy,
            _lease: lease,
        })
    }
    /// Validate imported state-major rows for this exact game.
    ///
    /// The input's capacities and the new flat snapshot overlap while rows are
    /// copied. Both are reserved before flattening; the input reservation is
    /// released when the consumed rows are freed. The returned policy retains
    /// only the flat snapshot reservation. Excess input capacity can therefore
    /// cause refusal even when the final snapshot would fit.
    pub fn from_rows(game: &RiverGame, rows: Vec<Vec<f64>>) -> Result<Self, SolveError> {
        let capacity_error =
            || SolveError::Allocation("imported strategy capacity overflow".into());
        let mut input_bytes = rows
            .capacity()
            .checked_mul(std::mem::size_of::<Vec<f64>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Vec<f64>>>()))
            .ok_or_else(capacity_error)?;
        for row in &rows {
            input_bytes = row
                .capacity()
                .checked_mul(std::mem::size_of::<f64>())
                .and_then(|n| input_bytes.checked_add(n))
                .ok_or_else(capacity_error)?;
        }
        // Tuple fields drop in declaration order. If the destination refuses,
        // free the input rows before releasing their charge. Keeping the lease
        // separate would drop it before the rows function parameter.
        let input = (rows, game.inner.budget.reserve(input_bytes)?);
        let lease = game
            .inner
            .budget
            .reserve(game.inner.memory.snapshot_bytes)?;
        let input = Strategy::from_node_rows(game.inner.layout.clone(), None, input.0)?;
        Ok(Self {
            game: game.clone(),
            policy: input,
            _lease: lease,
        })
    }
    /// Retained game and provenance for this policy.
    pub fn game(&self) -> &RiverGame {
        &self.game
    }
    /// True only for clones of the exact owned game, including its ranges and payoffs.
    pub fn is_bound_to(&self, game: &RiverGame) -> bool {
        Arc::ptr_eq(&self.game.inner, &game.inner)
    }
    /// Every node's state-major row end to end, in node order. One node's slice
    /// is [`Self::node_row`]; a combo's is [`Self::row`].
    pub fn values(&self) -> &[f64] {
        self.policy.values()
    }
    /// One node's whole flattened state-major row; terminal rows are empty.
    pub fn node_row(&self, node: NodeId) -> Option<&[f64]> {
        self.policy.row(node)
    }
    /// Action probabilities for a positive-range combo at a decision node.
    pub fn row(&self, node: NodeId, combo: Combo) -> Option<&[f64]> {
        let tree_node = self.game.tree().node(node)?;
        let RiverNodeKind::Decision { player } = tree_node.kind() else {
            return None;
        };
        let id = usize::from(combo.id());
        if self.game.inner.layout.weights[player as usize][id] == 0.0 {
            return None;
        }
        let n = tree_node.actions().len();
        Some(&self.policy.row(node)?[id * n..(id + 1) * n])
    }
    /// Expected net chips under this complete average policy.
    pub fn expected_value(&self, player: usize) -> Result<f64, SolveError> {
        self.value(player, false)
    }
    /// Maximum net chips against the opponent, maximizing per own information set.
    pub fn best_response(&self, player: usize) -> Result<f64, SolveError> {
        self.value(player, true)
    }
    fn value(&self, player: usize, maximize: bool) -> Result<f64, SolveError> {
        let _workspace = self.game.inner.budget.reserve(
            self.game.inner.memory.traversal_bytes + self.game.inner.memory.scratch_bytes,
        )?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        evaluate(
            &mut RiverTerminal {
                game: &self.game.inner,
                scratch: &mut scratch[0],
            },
            &self.policy,
            player,
            maximize,
        )
    }
    /// Measure exploitability within these ranges, this tree and zero-rake payoff model.
    pub fn exploitability(&self) -> Result<Exploitability, SolveError> {
        let _workspace = self.game.inner.budget.reserve(
            self.game.inner.memory.traversal_bytes + self.game.inner.memory.scratch_bytes,
        )?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        exploitability_bound(
            &mut RiverTerminal {
                game: &self.game.inner,
                scratch: &mut scratch[0],
            },
            &self.policy,
        )
    }
    /// Conditional action EVs after the specified public history, following this
    /// policy thereafter. Values use the fixed root origin; they are not per-hand
    /// error bounds. Impossible and zero-own-reach decisions are unavailable.
    pub fn decision_values(&self, node: NodeId) -> Result<DecisionValues, SolveError> {
        let game = &self.game.inner;
        let tree_node = game
            .tree
            .node(node)
            .ok_or_else(|| SolveError::InvalidGame("unknown river node".into()))?;
        let RiverNodeKind::Decision { player } = tree_node.kind() else {
            return Err(SolveError::InvalidGame(
                "action EV query requires a decision node".into(),
            ));
        };
        let player = player as usize;
        let lease = game.budget.reserve(game.memory.decision_bytes)?;
        let _workspace = game
            .budget
            .reserve(game.memory.traversal_bytes + game.memory.scratch_bytes)?;
        let mut path = reserved(game.tree.max_depth() + 1)?;
        let mut cursor = node;
        while let Some((parent, action)) = game.parents[cursor as usize] {
            path.push((parent, action));
            cursor = parent;
        }
        let mut reaches = [
            collect(game.layout.weights[0].iter().copied())?,
            collect(game.layout.weights[1].iter().copied())?,
        ];
        for (parent, action) in path.into_iter().rev() {
            let ancestor = &game.tree.nodes()[parent as usize];
            let RiverNodeKind::Decision { player: actor } = ancestor.kind() else {
                return Err(SolveError::InvalidGame(
                    "terminal ancestor in river tree".into(),
                ));
            };
            let n = ancestor.actions().len();
            for (id, reach) in reaches[actor as usize].iter_mut().enumerate() {
                *reach = crate::error::reach_product(
                    *reach,
                    self.policy.row(parent).expect("ancestor node")[id * n + action],
                    true,
                    0,
                    parent,
                    actor as usize,
                )?;
            }
        }
        let mut mass = [0.0; 1326];
        let opponent: &[f64; 1326] = reaches[1 - player]
            .as_slice()
            .try_into()
            .expect("fixed combo vector");
        evaluate_fold(game.dead, opponent, 1.0, &mut mass).map_err(|e| SolveError::Terminal {
            iteration: 0,
            node,
            player,
            reason: e.to_string(),
        })?;
        let n = tree_node.actions().len();
        let mut values = filled(1326 * n, None)?;
        let mut scratch = reserved(1)?;
        scratch.push(ShowdownScratch::default());
        let live = filled(1326, 1.0)?;
        for (action, child) in tree_node.children().iter().enumerate() {
            let conditional = walk(
                &mut RiverTerminal {
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
            for id in 0..1326 {
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
        let [zero, one] = reaches;
        let own_reach = if player == 0 { zero } else { one };
        Ok(DecisionValues {
            player,
            actions: n,
            values,
            own_reach,
            opponent_mass: collect(mass.into_iter())?,
            _lease: lease,
        })
    }
}
