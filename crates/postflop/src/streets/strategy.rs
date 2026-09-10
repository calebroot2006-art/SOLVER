//! Read-only postflop policies and the conditional values read off them.
//!
//! A postflop history spans more than one street, so a policy row means nothing
//! without the cards that were dealt to reach it. Every query here reports the
//! board and the runout beside the numbers, and a decision-value report carries
//! the acting player's own reach and the compatible opponent mass, so a
//! consumer cannot quote a river frequency without the context that says how
//! often the history happens.

use super::STATES;
use super::game::{Inner, PostflopGame};
use super::terminal::{PostflopTerminal, TerminalWorkspace};
use crate::memory::Lease;
use crate::{
    Exploitability, NodeId, SolveError, Strategy,
    allocation::{collect, filled, reserved},
    best_response::{evaluate, exploitability_bound, walk},
    error::{finite, reach_product},
    terminal::evaluate_fold,
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

/// Per-hand net-chip values at one public history, for both players.
///
/// [`PostflopDecisionValues`] says what each action is worth to whoever is
/// about to act. This says what the history itself is worth to each player if
/// the policy is followed from here, and it says it at a chance node and at a
/// terminal too, where no action exists to ask about.
///
/// The value is conditional on arriving here holding that hand, so the hand's
/// own probability of arriving does not enter it. [`Self::reach`] carries that
/// probability beside the value, and a caller weighting histories against each
/// other has to use it: a value alone says nothing about how often the history
/// happens.
#[derive(Debug)]
pub struct PostflopNodeValues {
    street: Street,
    board: Vec<Card>,
    runout_len: usize,
    values: [Vec<Option<f64>>; 2],
    reach: [Vec<f64>; 2],
    opponent_mass: [Vec<f64>; 2],
    _lease: Lease,
}

impl PostflopNodeValues {
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
    /// One value per combo ID for `player`, or `None` where the hand has no
    /// value to report: it carries no weight in that player's range, a board
    /// card in this history blocks it, or no compatible opponent hand is left
    /// for it to have a value against. `None` is missing, never zero.
    ///
    /// A hand the policy never brings here still has a value, because the value
    /// is conditional on arriving. Its [`Self::reach`] is then zero, and that,
    /// not a missing value, is what says the history is off this hand's path.
    /// [`PostflopStrategy::decision_values`] answers a narrower question and
    /// withholds those rows; the two therefore report values for different hand
    /// sets at the same node, and agree on every hand both report.
    #[must_use]
    pub fn values(&self, player: usize) -> &[Option<f64>] {
        &self.values[player]
    }
    /// `player`'s range times their own preceding action probabilities and masks.
    #[must_use]
    pub fn reach(&self, player: usize) -> &[f64] {
        &self.reach[player]
    }
    /// Compatible opposing range, action mass and chance factors, unnormalized.
    #[must_use]
    pub fn opponent_mass(&self, player: usize) -> &[f64] {
        &self.opponent_mass[player]
    }
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
    ///
    /// All retained buffer capacities are charged to the shared game budget, and
    /// never less than one snapshot: an import is a retained average like any
    /// other, so it draws on the same row of the memory table
    /// (`MemoryReservation::Snapshot`). Rows arriving with spare capacity are
    /// charged what they actually hold, which is more.
    pub fn from_rows(game: &PostflopGame, rows: Vec<Vec<f64>>) -> Result<Self, SolveError> {
        let input = Strategy::from_node_rows(game.inner.layout.clone(), None, rows)?;
        Self::import(game, input)
    }

    /// Validate one flat buffer of state-major rows, in node order.
    pub fn from_values(game: &PostflopGame, values: Vec<f64>) -> Result<Self, SolveError> {
        let input = Strategy::from_values(game.inner.layout.clone(), None, values)?;
        Self::import(game, input)
    }

    fn import(game: &PostflopGame, input: Strategy) -> Result<Self, SolveError> {
        let capacity_error =
            || SolveError::Allocation("imported strategy capacity overflow".into());
        let bytes = input
            .values()
            .len()
            .checked_mul(std::mem::size_of::<f64>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Strategy>() + 256))
            .ok_or_else(capacity_error)?;
        let lease = game
            .inner
            .budget
            .reserve(bytes.max(game.inner.memory.snapshot_bytes))?;
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
    /// Every node's state-major row end to end, in node order. Rows run over
    /// this game's live combos, not all 1326; [`Self::row`] takes a combo and
    /// finds its slot.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        self.policy.values()
    }
    /// One node's whole state-major row; chance and terminal rows are empty.
    #[must_use]
    pub fn node_row(&self, node: NodeId) -> Option<&[f64]> {
        self.policy.row(node)
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
        let state = self.game.inner.slot[player as usize][usize::from(combo.id())];
        if state == Inner::BLOCKED {
            return None;
        }
        if view
            .board()
            .iter()
            .any(|card| combo.mask() & card.mask() != 0)
        {
            return None;
        }
        let state = usize::from(state);
        let n = view.actions().len();
        Some(&self.policy.row(node)?[state * n..(state + 1) * n])
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
        let mut workspace = TerminalWorkspace::default();
        exploitability_bound(
            &mut PostflopTerminal {
                game: &self.game.inner,
                workspace: &mut workspace,
            },
            &self.policy,
        )
    }

    /// One traversal buffer set and one terminal scratch: a query walks the
    /// tree serially whatever the game's worker count is.
    ///
    /// `traversal_bytes` is the estimate's per-walk term, and above one worker
    /// it is sized by the widest deal rather than the widest bet menu, because a
    /// parallel chance node gathers every outcome's value vector. A serial query
    /// holds one at a time, so this reservation is an upper bound on what the
    /// query actually takes. It is not extra: the bound already charges
    /// `workers + 1` of these, and the spare one is this query. A query
    /// overlapping an iteration therefore cannot be refused by this reservation
    /// under a limit that admitted the game.
    fn reserve_workspace(&self) -> Result<Lease, SolveError> {
        let memory = self.game.inner.memory;
        self.game
            .inner
            .budget
            .reserve(memory.traversal_bytes + memory.scratch_bytes)
    }

    fn value(&self, player: usize, maximize: bool) -> Result<f64, SolveError> {
        let _workspace = self.reserve_workspace()?;
        let mut workspace = TerminalWorkspace::default();
        evaluate(
            &mut PostflopTerminal {
                game: &self.game.inner,
                workspace: &mut workspace,
            },
            &self.policy,
            player,
            maximize,
        )
    }

    /// Scatters one compacted per-combo vector over all 1326 combo IDs.
    ///
    /// The walk carries live combos; every report this module returns is
    /// indexed by combo ID, because that is what a consumer of a solved spot
    /// asks in. Entries with no live slot stay at `blank`.
    fn scatter<T: Copy>(
        &self,
        player: usize,
        values: &[T],
        blank: T,
    ) -> Result<Vec<T>, SolveError> {
        let mut wide = filled(STATES, blank)?;
        for (value, id) in values.iter().zip(&self.game.inner.live[player]) {
            wide[usize::from(*id)] = *value;
        }
        Ok(wide)
    }

    /// Both players' reach at `node`, the live masks the deals on the way left
    /// behind, and the product of the chance probabilities taken.
    ///
    /// Shared by [`Self::decision_values`] and [`Self::node_values`] so that the
    /// two reports cannot drift apart: a value means nothing without the reach
    /// that says how often its history happens, and there is one right way to
    /// accumulate that reach.
    ///
    /// `masks_for` names the players whose live mask the caller will actually
    /// walk with, so a decision query no longer builds and discards the other
    /// one. Everything here is in compact indices; the reports scatter.
    #[allow(clippy::type_complexity)]
    fn path_reaches(
        &self,
        node: NodeId,
        masks_for: &[usize],
    ) -> Result<([Vec<f64>; 2], [Vec<f64>; 2], f64), SolveError> {
        let game = &self.game.inner;
        let states = game.layout.states;
        let mut path = reserved(game.tree.max_depth() + 1)?;
        let mut cursor = node;
        while let Some((parent, index)) = game.parent(cursor) {
            path.push((parent, index));
            cursor = parent;
        }
        let mut reaches = [
            collect(game.layout.weights[0].iter().copied())?,
            collect(game.layout.weights[1].iter().copied())?,
        ];
        let mut live: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        for player in masks_for {
            live[*player] = filled(states[*player], 1.0)?;
        }
        let mut chance_weight = 1.0_f64;
        for (parent, index) in path.into_iter().rev() {
            match game
                .compact(parent)
                .ok_or_else(|| SolveError::InvalidGame("unknown ancestor node".into()))?
                .kind()
            {
                PostflopNodeKind::Decision { player: actor } => {
                    let n = game.layout.children(parent).len();
                    let row = self.policy.row(parent).expect("ancestor node");
                    for (state, reach) in reaches[actor as usize].iter_mut().enumerate() {
                        *reach = reach_product(
                            *reach,
                            row[state * n + index],
                            true,
                            0,
                            parent,
                            actor as usize,
                        )?;
                    }
                }
                PostflopNodeKind::Chance { .. } => {
                    let masks = game.layout.masks(parent, index);
                    for (side, mask) in reaches.iter_mut().zip(masks) {
                        for (reach, entry) in side.iter_mut().zip(mask) {
                            *reach *= entry;
                        }
                    }
                    for player in masks_for {
                        for (entry, factor) in live[*player].iter_mut().zip(&masks[*player]) {
                            *entry *= factor;
                        }
                    }
                    chance_weight *= game.layout.probabilities(parent)[index];
                }
                PostflopNodeKind::Terminal(_) => {
                    return Err(SolveError::InvalidGame(
                        "terminal ancestor in postflop tree".into(),
                    ));
                }
            }
        }
        Ok((reaches, live, chance_weight))
    }

    /// The opposing mass one query walks against, over all 1326 combo IDs.
    ///
    /// This is the one place the two reports would otherwise duplicate: scatter
    /// the opponent's compacted reach, weight it by the chance probabilities
    /// taken to get here, and ask the fold evaluator how much compatible
    /// opposing range each of this player's hands faces.
    fn opposing_mass(
        &self,
        node: NodeId,
        player: usize,
        reaches: &[Vec<f64>; 2],
        chance_weight: f64,
    ) -> Result<([f64; STATES], Vec<f64>), SolveError> {
        let weighted: Vec<f64> = collect(
            reaches[1 - player]
                .iter()
                .map(|reach| reach * chance_weight),
        )?;
        let wide = self.scatter(1 - player, &weighted, 0.0)?;
        let wide: &[f64; STATES] = wide.as_slice().try_into().expect("fixed combo vector");
        let mut mass = [0.0; STATES];
        evaluate_fold(
            self.game
                .inner
                .payoff(node)
                .ok_or_else(|| SolveError::InvalidGame("unknown postflop node".into()))?
                .dead,
            wide,
            1.0,
            &mut mass,
        )
        .map_err(|e| SolveError::Terminal {
            iteration: 0,
            node,
            player,
            reason: e.to_string(),
        })?;
        Ok((mass, weighted))
    }

    /// Divides one walk's conditional values by the opposing mass, in place of
    /// the block both reports used to hold a copy of.
    ///
    /// `reported` decides which hands get an answer: `node_values` answers for
    /// every hand in the range, because a hand the policy never brings here
    /// still has a value if it arrives; `decision_values` answers only for
    /// hands the policy does bring here, because a player who never arrives is
    /// not about to take an action.
    fn conditional(
        &self,
        node: NodeId,
        player: usize,
        conditional: &[f64],
        mass: &[f64; STATES],
        reported: &[f64],
        mut record: impl FnMut(usize, f64),
    ) -> Result<(), SolveError> {
        for (state, value) in conditional.iter().enumerate() {
            let id = usize::from(self.game.inner.live[player][state]);
            if reported[state] > 0.0 && mass[id] > 0.0 {
                let quotient = value / mass[id];
                if *value != 0.0 && quotient == 0.0 {
                    return Err(SolveError::Arithmetic {
                        iteration: 0,
                        node,
                        player,
                        reason: "conditional value underflow",
                    });
                }
                finite(&[quotient], 0, node, player)?;
                record(id, quotient);
            }
        }
        Ok(())
    }

    /// Per-hand net-chip values for both players at any public history,
    /// following this policy from there.
    ///
    /// [`Self::decision_values`] answers what each action is worth to whoever
    /// acts next, and refuses any node where nobody acts. This answers what the
    /// history itself is worth, so it also answers at a chance node and at a
    /// terminal. A tool that has to value a continuation it cannot walk for
    /// itself needs exactly that: `tests/reference/turn/oracle.py` treats a
    /// chance node as a leaf and reads the value the capture reported there.
    ///
    /// Same origin and same walk as [`Self::decision_values`], so at a decision
    /// node the acting player's value here is that report's action values
    /// averaged under this policy, up to the order the sum is taken in.
    ///
    /// It does not have the same refusals. `decision_values` answers "what is
    /// each action worth to the player about to take it", and a player who never
    /// arrives is not about to take anything, so a hand with zero reach on the
    /// path is withheld there. This answers "what is this history worth to a
    /// hand that arrives", which is defined whether or not the policy ever
    /// brings the hand here, and is exactly what a walker that stops at a chance
    /// node needs: it reaches that node down branches this hand may take with
    /// probability zero, and it multiplies by that probability itself. So the
    /// value is reported wherever it exists, and [`PostflopNodeValues::reach`]
    /// beside it is what says the hand never arrives. `None` means the hand
    /// carries no weight in the range, a board card blocks it, or no compatible
    /// opponent hand is left; never zero for missing.
    ///
    /// The report holds one value, reach and opposing-mass vector per player
    /// rather than one value vector per action, so it is its own row of the
    /// memory table (`MemoryReservation::NodeReport`) and reserves exactly
    /// `node_bytes`. The bound counts that row beside the decision report,
    /// because nothing stops a caller holding one of each.
    pub fn node_values(&self, node: NodeId) -> Result<PostflopNodeValues, SolveError> {
        let game = &self.game.inner;
        let view = self
            .game
            .node(node)
            .ok_or_else(|| SolveError::InvalidGame("unknown postflop node".into()))?;
        let lease = game.budget.reserve(game.memory.node_bytes)?;
        let _workspace = self.reserve_workspace()?;

        let (reaches, live, chance_weight) = self.path_reaches(node, &[0, 1])?;
        let mut workspace = TerminalWorkspace::default();
        let mut values = [filled(STATES, None)?, filled(STATES, None)?];
        let mut opponent_mass = [Vec::new(), Vec::new()];
        for player in 0..2 {
            let (mass, weighted) = self.opposing_mass(node, player, &reaches, chance_weight)?;
            let walked = walk(
                &mut PostflopTerminal {
                    game,
                    workspace: &mut workspace,
                },
                &self.policy,
                node,
                player,
                &weighted,
                &live[player],
                false,
            )?;
            // The range weight, not the reach: a hand the policy never brings
            // here still has a conditional value, and the walk above never
            // multiplied by the hand's own probability of arriving. Every
            // compacted state has positive weight, so every one is reported.
            let reported = filled(game.layout.states[player], 1.0)?;
            let row = &mut values[player];
            self.conditional(node, player, &walked, &mass, &reported, |id, value| {
                row[id] = Some(value);
            })?;
            opponent_mass[player] = collect(mass.into_iter())?;
        }
        let board = collect(view.board().iter().copied())?;
        let [zero, one] = reaches;
        Ok(PostflopNodeValues {
            street: view.street(),
            board,
            runout_len: view.runout().len(),
            values,
            reach: [self.scatter(0, &zero, 0.0)?, self.scatter(1, &one, 0.0)?],
            opponent_mass,
            _lease: lease,
        })
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

        let (reaches, all_live, chance_weight) = self.path_reaches(node, &[player])?;
        let live = &all_live[player];
        let (mass, weighted) = self.opposing_mass(node, player, &reaches, chance_weight)?;

        let n = view.actions().len();
        let mut values = filled(STATES * n, None)?;
        let mut workspace = TerminalWorkspace::default();
        for action in 0..n {
            let child = view.children()[action];
            let walked = walk(
                &mut PostflopTerminal {
                    game,
                    workspace: &mut workspace,
                },
                &self.policy,
                child,
                player,
                &weighted,
                live,
                false,
            )?;
            // Only hands the policy actually brings here: `reaches` is the own
            // reach along the path, so a zero withholds the row.
            self.conditional(
                node,
                player,
                &walked,
                &mass,
                &reaches[player],
                |id, value| {
                    values[id * n + action] = Some(value);
                },
            )?;
        }
        let board = collect(view.board().iter().copied())?;
        let runout_len = view.runout().len();
        let own_reach = self.scatter(player, &reaches[player], 0.0)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streets::{PostflopOptions, PostflopSolver};
    use crate::{Precision, Variant};
    use cards::Range;
    use tree::{Action, BetSizeOptions, PostflopTree, PostflopTreeConfig};

    const LIMIT: usize = 1024 * 1024 * 1024;

    fn board() -> Vec<Card> {
        "9c 5d 2h Ks"
            .split_ascii_whitespace()
            .map(|card| card.parse().unwrap())
            .collect()
    }

    /// A turn tree whose only wager is the jam, so a called line runs the river
    /// out with no further decision. It is small and it still holds a decision
    /// node for each player, a chance node, a fold and a showdown: every node
    /// kind [`PostflopStrategy::node_values`] has to answer at.
    fn game() -> PostflopGame {
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
        PostflopGame::new(
            &board(),
            [
                Range::parse("AA, QQ, JTs").unwrap(),
                Range::parse("KK, 99, 76s").unwrap(),
            ],
            tree,
            PostflopOptions {
                memory_limit_bytes: LIMIT,
                precision: Precision::F64,
                threads: 1,
            },
        )
        .unwrap()
    }

    fn child(game: &PostflopGame, node: NodeId, action: Action) -> NodeId {
        let view = game.node(node).unwrap();
        let index = view
            .actions()
            .iter()
            .position(|candidate| *candidate == action)
            .unwrap();
        view.children()[index]
    }

    /// An average over a few iterations, so the policy under test is a real
    /// mixture rather than the uniform row.
    fn solved(game: &PostflopGame) -> PostflopStrategy {
        let mut solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
        for _ in 0..3 {
            solver.run_iteration().unwrap();
        }
        solver.average_strategy().unwrap()
    }

    fn combo(text: &str) -> Combo {
        let cards: Vec<Card> = text
            .split_ascii_whitespace()
            .map(|card| card.parse().unwrap())
            .collect();
        Combo::new(cards[0], cards[1]).unwrap()
    }

    #[test]
    fn a_decision_node_value_is_the_action_values_averaged_under_the_policy() {
        let game = game();
        let strategy = solved(&game);
        let jam = child(&game, game.root(), Action::AllIn(20));
        let mut compared = 0;
        for node in [game.root(), jam] {
            let view = game.node(node).unwrap();
            let PostflopNodeKind::Decision { player } = view.kind() else {
                unreachable!("both of these are decision nodes");
            };
            let player = player as usize;
            let n = view.actions().len();
            let decision = strategy.decision_values(node).unwrap();
            let values = strategy.node_values(node).unwrap();
            assert_eq!(decision.player(), player);
            for id in 0..STATES {
                let row = &decision.values()[id * n..(id + 1) * n];
                let Some(value) = values.values(player)[id] else {
                    // Every hand the narrower report answers for is answered here.
                    assert!(row.iter().all(Option::is_none), "combo {id} at node {node}");
                    continue;
                };
                if row.iter().any(Option::is_none) {
                    continue;
                }
                let policy = strategy
                    .row(node, Combo::from_id(id as u16).unwrap())
                    .expect("a live combo at the node it acts on");
                let averaged: f64 = policy
                    .iter()
                    .zip(row)
                    .map(|(probability, action)| probability * action.unwrap())
                    .sum();
                assert!(
                    (value - averaged).abs() < 1e-9,
                    "node {node} combo {id}: {value} is not the policy average {averaged}"
                );
                compared += 1;
            }
        }
        // The board takes the king of spades and the nine of clubs, leaving 16
        // out-of-position and 10 in-position combos, and both reports answer for
        // every one of them at the node where that player acts.
        assert_eq!(compared, 16 + 10);
    }

    #[test]
    fn a_chance_node_and_a_terminal_both_report_values() {
        let game = game();
        let strategy = solved(&game);
        let jam = child(&game, game.root(), Action::AllIn(20));
        let called = child(&game, jam, Action::Call);
        let folded = child(&game, jam, Action::Fold);
        let deal = game.node(called).unwrap();
        assert!(matches!(deal.kind(), PostflopNodeKind::Chance { .. }));
        let showdown = deal.children()[0];
        let runout = deal.possible_cards()[0];

        for (node, cards) in [(called, 4), (folded, 4), (showdown, 5)] {
            let values = strategy.node_values(node).unwrap();
            assert_eq!(values.board().len(), cards);
            for player in 0..2 {
                let answered = (0..STATES)
                    .filter(|id| values.values(player)[*id].is_some())
                    .count();
                assert!(
                    answered > 0,
                    "node {node} answers nothing for player {player}"
                );
            }
        }

        // The fold is not a guess: the in-position player folded to the jam, so
        // every live hand is worth exactly half the ten-chip pot to the jammer
        // and the same to the folder with the sign turned round.
        let folded_values = strategy.node_values(folded).unwrap();
        assert!(folded_values.runout().is_empty());
        for id in 0..STATES {
            if let Some(value) = folded_values.values(0)[id] {
                assert!((value - 5.0).abs() < 1e-9, "combo {id} won {value}");
            }
            if let Some(value) = folded_values.values(1)[id] {
                assert!((value + 5.0).abs() < 1e-9, "combo {id} folded for {value}");
            }
        }

        // A river showdown carries the card that was dealt, and no hand holding
        // that card has a value there.
        let river = strategy.node_values(showdown).unwrap();
        assert_eq!(river.runout(), [runout]);
        for player in 0..2 {
            for id in 0..STATES {
                if Combo::from_id(id as u16).unwrap().mask() & runout.mask() != 0 {
                    assert!(
                        river.values(player)[id].is_none(),
                        "combo {id} holds {runout}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_unknown_node_is_refused_and_an_unreached_hand_still_has_a_value() {
        let game = game();
        let uniform = PostflopStrategy::uniform(&game).unwrap();
        let unknown = game.num_nodes() as NodeId;
        let refused = uniform.node_values(unknown).unwrap_err();
        assert!(
            matches!(&refused, SolveError::InvalidGame(text) if text.contains("unknown postflop node")),
            "{refused:?}"
        );

        // A hand outside both ranges has nothing to report, at any node.
        let outside = usize::from(combo("Ts 9s").id());
        assert!(uniform.node_values(game.root()).unwrap().values(0)[outside].is_none());

        // Make one hand jam every time. The check branch is then a history that
        // hand never reaches, and its value there is exactly what a walker
        // stopping at a chance node still has to be able to read: the walker
        // arrives down branches the hand takes with probability zero, and
        // multiplies by that probability itself.
        let hand = combo("Ah Ad");
        let root = game.root();
        let actions = game.node(root).unwrap().actions().len();
        let mut values = uniform.values().to_vec();
        // The root row starts at the root node's own offset, and the hand takes
        // the compact slot its combo id maps to.
        let state = usize::from(game.inner.slot[0][usize::from(hand.id())]);
        for action in 0..actions {
            values[state * actions + action] = f64::from(u8::from(action == actions - 1));
        }
        let pure = PostflopStrategy::from_values(&game, values).unwrap();

        let checked = child(&game, root, Action::Check);
        let facing = child(&game, checked, Action::AllIn(20));
        let values = pure.node_values(facing).unwrap();
        let hand_id = usize::from(hand.id());
        assert_eq!(values.reach(0)[hand_id], 0.0);
        let value = values.values(0)[hand_id].expect("an unreached hand still has a value");
        assert!(value.is_finite());
        assert!(values.reach(1)[usize::from(combo("Kh Kd").id())] > 0.0);

        // The narrower report withholds that row, which is the whole reason
        // this accessor exists.
        let decision = pure.decision_values(facing).unwrap();
        let n = game.node(facing).unwrap().actions().len();
        assert!(
            decision.values()[hand_id * n..(hand_id + 1) * n]
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn a_node_report_reserves_the_row_the_estimate_charges_for_it() {
        let game = game();
        let memory = game.memory_usage();
        // Both players' values, reach and opposing mass, and nothing else: the
        // report is sized by the player count, not by the action count.
        assert_eq!(
            memory.node_bytes,
            2 * STATES * std::mem::size_of::<Option<f64>>() + 4 * STATES * 8 + 512
        );
        let strategy = PostflopStrategy::uniform(&game).unwrap();
        let held = game.reserved_bytes();
        let values = strategy.node_values(game.root()).unwrap();
        assert_eq!(game.reserved_bytes(), held + memory.node_bytes);
        drop(values);
        assert_eq!(game.reserved_bytes(), held);
    }

    #[test]
    fn a_node_query_fits_a_limit_sized_to_the_estimate_during_a_solve() {
        // The failure this guards against: a caller who sizes the limit to the
        // estimate, as the README's table invites, and is refused on the first
        // node query after a solve.
        let sized = game();
        let bound = sized.memory_usage().working_set_bound_bytes;
        let mut options = PostflopOptions {
            memory_limit_bytes: bound,
            precision: Precision::F64,
            threads: 1,
        };
        options.memory_limit_bytes = bound;
        let tree = PostflopTree::new(sized.tree().config().clone()).unwrap();
        let game = PostflopGame::new(
            &board(),
            [
                Range::parse("AA, QQ, JTs").unwrap(),
                Range::parse("KK, 99, 76s").unwrap(),
            ],
            tree,
            options,
        )
        .unwrap();
        let mut solver = PostflopSolver::new(game.clone(), Variant::Plus).unwrap();
        solver.run_iteration().unwrap();
        let average = solver.average_strategy().unwrap();
        let second = solver.average_strategy().unwrap();
        let decision = average.decision_values(game.root()).unwrap();
        let values = average.node_values(game.root()).unwrap();
        assert!(values.values(0).iter().any(Option::is_some));
        drop((decision, values, average, second, solver));
    }
}
