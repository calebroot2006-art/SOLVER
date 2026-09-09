//! Probability and information-set contract for a two-player public tree.
use crate::{
    SolveError,
    allocation::{filled, reserved},
    error::normalized_sum,
};
pub use payoff::Real;
use std::{collections::HashMap, ops::Deref, sync::Arc};

/// Index into immutable public-node storage.
pub type NodeId = u32;

/// Operation at a public node; action menus are identical for all own states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A decision by one player.
    Player {
        /// Zero-based player, either zero or one.
        player: u8,
        /// Actions in `Game::child` order.
        num_actions: u8,
    },
    /// Public physical outcomes, including outcomes blocked by private cards.
    Chance {
        /// Physical outcomes in `Game::child` order.
        num_outcomes: u16,
    },
    /// A terminal utility, including folds and ties.
    Terminal,
}

/// Immutable public histories for a two-player zero-sum chip game.
///
/// An information set is (public node, own private state). Distinct public
/// histories must use distinct nodes. Cycles, shared children, and unreachable
/// nodes are rejected. All methods must give stable answers throughout a solve.
///
/// Private deals have product weight w0[h0] * w1[h1], conditioned on compatibility.
/// EV and best response divide by the surviving root mass. Chance is counted
/// once: for each compatible pair still legal at a chance node,
/// sum(probability * mask0 * mask1) equals one. Leduc enumerates six board cards
/// with probability 1/4 each; masks remove the two held cards.
///
/// Terminal opponent reach includes opponent initial weight, opponent action
/// probabilities, chance probabilities, and OPPONENT masks. Own masks zero the
/// corresponding output entries separately. Own reach for strategy averaging
/// contains only own action probabilities, never chance or opponent reach.
/// Actions may depend only on public history and the acting player's own state.
pub trait Game {
    /// Number of public nodes, including terminals.
    fn num_nodes(&self) -> usize;
    /// Root public node.
    fn root(&self) -> NodeId;
    /// Operation at a valid node.
    fn kind(&self, node: NodeId) -> NodeKind;
    /// Child for a valid action or physical outcome.
    fn child(&self, node: NodeId, index: usize) -> NodeId;
    /// Length of this player's fixed private-state vector.
    fn num_private_states(&self, player: usize) -> usize;
    /// Unnormalized finite nonnegative private weights.
    fn initial_weights(&self, player: usize) -> &[Real];
    /// Whether the two states can coexist in a deal.
    fn compatible(&self, p0_state: usize, p1_state: usize) -> bool;
    /// Physical outcome probability before private masks.
    fn chance_prob(&self, node: NodeId, outcome: usize) -> Real;
    /// Zero-or-one entries in the player's private-state order.
    fn chance_mask(&self, node: NodeId, outcome: usize, player: usize) -> &[Real];
    /// Writes sum(opp_reach[h'] * compatible(h,h') * utility(player,h,h')).
    /// Every output must be written. This operation must be linear in reach;
    /// each compatible deal's two utilities must sum to zero.
    fn terminal_values(&self, node: NodeId, player: usize, opp_reach: &[Real], out: &mut [Real]);
    /// Positive fixed root pot used as the percentage denominator.
    fn starting_pot(&self) -> Real;
    /// Debug label; it does not define information-set identity.
    fn info_label(&self, node: NodeId, player: usize, state: usize) -> String;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
    pub probabilities: Vec<Real>,
    /// One index into [`TraversalLayout::mask_pool`] per outcome, in the same
    /// order as `children`. Player and terminal nodes hold none.
    pub masks: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TraversalLayout {
    pub root: NodeId,
    pub states: [usize; 2],
    pub weights: [Vec<Real>; 2],
    pub nodes: Vec<Node>,
    /// Each distinct pair of per-player chance masks, stored once. Outcomes
    /// that remove the same card produce the same entries, so a turn or flop
    /// tree keeps one pair per card rather than one per (chance node, outcome).
    /// Entries are matched on their exact bit patterns: a mask spelling zero as
    /// `-0.0` stays separate from one spelling it `0.0`, so every value a
    /// traversal reads is the value the game supplied, unchanged by pooling.
    pub mask_pool: Vec<[Vec<Real>; 2]>,
    pub normalizer: Real,
    pub pot: Real,
}

// Only callback games carry this audit evidence. Owned river games construct
// traversal metadata from their checked concrete tree and have no callback API.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub traversal: Arc<TraversalLayout>,
    compatible: Vec<bool>,
    // Opponent-state-major columns. Bits retain deterministic NaN sentinels.
    terminal_kernels: Vec<[Vec<u64>; 2]>,
}

impl Deref for Layout {
    type Target = TraversalLayout;

    fn deref(&self) -> &Self::Target {
        &self.traversal
    }
}

impl Layout {
    pub fn new(game: &dyn Game) -> Result<Self, SolveError> {
        let invalid = |message: &str| SolveError::InvalidGame(message.into());
        let count = game.num_nodes();
        if count == 0 || count > NodeId::MAX as usize || game.root() as usize >= count {
            return Err(invalid(
                "node count and root must describe a nonempty indexed tree",
            ));
        }
        let states = [game.num_private_states(0), game.num_private_states(1)];
        if states.contains(&0) {
            return Err(SolveError::EmptyGame);
        }
        states[0]
            .checked_mul(states[1])
            .ok_or_else(|| invalid("private-pair allocation overflow"))?;
        let weights = [
            game.initial_weights(0).to_vec(),
            game.initial_weights(1).to_vec(),
        ];
        for player in 0..2 {
            if weights[player].len() != states[player]
                || weights[player].iter().any(|v| !v.is_finite() || *v < 0.0)
            {
                return Err(invalid(
                    "initial_weights must match private-state counts and be finite and nonnegative",
                ));
            }
        }
        let mut compatible = Vec::with_capacity(states[0] * states[1]);
        let mut normalizer = 0.0;
        for (h0, w0) in weights[0].iter().enumerate() {
            for (h1, w1) in weights[1].iter().enumerate() {
                let legal = game.compatible(h0, h1);
                compatible.push(legal);
                if legal {
                    normalizer += w0 * w1;
                }
            }
        }
        if !normalizer.is_finite() || normalizer <= 0.0 {
            return Err(SolveError::EmptyGame);
        }
        let pot = game.starting_pot();
        if !pot.is_finite() || pot <= 0.0 {
            return Err(invalid("starting_pot must be finite and positive"));
        }
        let mut nodes = Vec::with_capacity(count);
        let mut terminal_kernels = Vec::with_capacity(count);
        let mut mask_pool: Vec<[Vec<Real>; 2]> = Vec::new();
        // Keyed on both players' mask bits so equal outcomes share one entry.
        let mut pooled: HashMap<Vec<u64>, usize> = HashMap::new();
        for id in 0..count {
            let kind = game.kind(id as NodeId);
            let n = match kind {
                NodeKind::Player {
                    player,
                    num_actions,
                } => {
                    if player > 1 || num_actions == 0 {
                        return Err(invalid(
                            "player nodes require player 0 or 1 and nonzero actions",
                        ));
                    }
                    states[player as usize]
                        .checked_mul(num_actions as usize)
                        .ok_or_else(|| invalid("strategy allocation overflow"))?;
                    num_actions as usize
                }
                NodeKind::Chance { num_outcomes } => {
                    if num_outcomes == 0 {
                        return Err(invalid("chance node has no outcomes"));
                    }
                    num_outcomes as usize
                }
                NodeKind::Terminal => 0,
            };
            let children: Vec<_> = (0..n).map(|i| game.child(id as NodeId, i)).collect();
            if children.iter().any(|child| *child as usize >= count) {
                return Err(invalid("child index outside node storage"));
            }
            let mut probabilities = Vec::new();
            let mut masks = Vec::new();
            if matches!(kind, NodeKind::Chance { .. }) {
                for outcome in 0..n {
                    let p = game.chance_prob(id as NodeId, outcome);
                    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                        return Err(invalid("chance_prob must be finite and in [0,1]"));
                    }
                    let pair = [
                        game.chance_mask(id as NodeId, outcome, 0).to_vec(),
                        game.chance_mask(id as NodeId, outcome, 1).to_vec(),
                    ];
                    for player in 0..2 {
                        if pair[player].len() != states[player]
                            || pair[player].iter().any(|v| *v != 0.0 && *v != 1.0)
                        {
                            return Err(invalid(
                                "chance_mask must contain a zero-or-one entry per private state",
                            ));
                        }
                    }
                    let key: Vec<u64> = pair[0]
                        .iter()
                        .chain(pair[1].iter())
                        .map(|value| value.to_bits())
                        .collect();
                    let next = mask_pool.len();
                    let index = *pooled.entry(key).or_insert(next);
                    if index == next {
                        mask_pool.push(pair);
                    }
                    probabilities.push(p);
                    masks.push(index);
                }
                if !probabilities.iter().any(|p| *p > 0.0) {
                    return Err(invalid("chance node needs a positive probability"));
                }
            }
            let terminal_kernel = if kind == NodeKind::Terminal {
                capture_kernel(game, id as NodeId, states)
            } else {
                [Vec::new(), Vec::new()]
            };
            terminal_kernels.push(terminal_kernel);
            nodes.push(Node {
                kind,
                children,
                probabilities,
                masks,
            });
        }
        let layout = Self {
            traversal: Arc::new(TraversalLayout {
                root: game.root(),
                states,
                weights,
                nodes,
                mask_pool,
                normalizer,
                pot,
            }),
            compatible,
            terminal_kernels,
        };
        layout.validate_paths()?;
        Ok(layout)
    }

    /// Walks every contract over every declared pair. A callback game states
    /// its own private-state counts and this path already holds a compatibility
    /// bit and a full terminal kernel per pair, so the zero-sum pass's pair
    /// matrix is the smallest of the three and needs no separate budget. The
    /// owned games, whose states are the 1326 hold'em combos, scope the walk by
    /// weight instead and charge the matrix in their estimate.
    fn validate_paths(&self) -> Result<(), SolveError> {
        let states = self.states;
        let compatible = |h0: usize, h1: usize| self.compatible[h0 * states[1] + h1];
        let mut columns = KernelColumns {
            kernels: &self.terminal_kernels,
            states,
        };
        let source: Option<&mut dyn TerminalColumns> = Some(&mut columns);
        validate_traversal(&self.traversal, &compatible, PairScope::All, source)?;
        Ok(())
    }

    pub fn check_game(&self, game: &dyn Game) -> Result<(), SolveError> {
        let changed =
            || SolveError::InvalidGame("game differs from the strategy/solver binding".into());
        if game.root() != self.root
            || game.num_nodes() != self.nodes.len()
            || game.starting_pot() != self.pot
        {
            return Err(changed());
        }
        for player in 0..2 {
            if game.num_private_states(player) != self.states[player]
                || game.initial_weights(player) != self.weights[player]
            {
                return Err(changed());
            }
        }
        for h0 in 0..self.states[0] {
            for h1 in 0..self.states[1] {
                if game.compatible(h0, h1) != self.compatible[h0 * self.states[1] + h1] {
                    return Err(changed());
                }
            }
        }
        for (id, node) in self.nodes.iter().enumerate() {
            if game.kind(id as NodeId) != node.kind {
                return Err(changed());
            }
            for (action, child) in node.children.iter().enumerate() {
                if game.child(id as NodeId, action) != *child {
                    return Err(changed());
                }
            }
            for (outcome, p) in node.probabilities.iter().enumerate() {
                if game.chance_prob(id as NodeId, outcome) != *p {
                    return Err(changed());
                }
                for (player, entries) in self.masks(node, outcome).iter().enumerate() {
                    if game.chance_mask(id as NodeId, outcome, player) != entries.as_slice() {
                        return Err(changed());
                    }
                }
            }
            if node.kind == NodeKind::Terminal
                && capture_kernel(game, id as NodeId, self.states) != self.terminal_kernels[id]
            {
                return Err(SolveError::InvalidGame(format!(
                    "terminal payoff changed at node {id}; construct a new solver/strategy"
                )));
            }
        }
        Ok(())
    }
}

/// Which private-state pairs a path validation walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PairScope {
    /// Every declared pair, whatever weight its states carry. Callback games
    /// use this: their state counts are small and the weights are the caller's.
    All,
    /// Only pairs whose two states both carry positive initial weight. A zero
    /// weight contributes nothing to any reach, value or mass, and a hold'em
    /// game declares 1326 states per player of which a range uses a fraction.
    PositiveWeight,
    /// No pairs at all. The chance-mass and zero-sum checks are quadratic in
    /// the live states, so a tree above the caller's pair budget asks for this
    /// scope: every linear structural contract is still walked, and the report
    /// says zero pairs rather than claiming a check that did not run.
    NoPairs,
}

/// One terminal's utilities, read one opponent state at a time.
///
/// Terminal values are linear in opponent reach, so a one-hot opponent vector
/// returns the whole column for that opponent state: the zero-sum check costs
/// one call per state rather than one per pair.
pub(crate) trait TerminalColumns {
    /// Writes the utility to `player` in each of their states, given the
    /// opponent holds `opponent`.
    fn column(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: usize,
        out: &mut [Real],
    ) -> Result<(), SolveError>;
}

/// Reads utility columns off a callback game's captured terminal kernels.
struct KernelColumns<'a> {
    kernels: &'a [[Vec<u64>; 2]],
    states: [usize; 2],
}

impl TerminalColumns for KernelColumns<'_> {
    fn column(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: usize,
        out: &mut [Real],
    ) -> Result<(), SolveError> {
        let own = self.states[player];
        let start = opponent * own;
        let bits = self.kernels[node as usize][player]
            .get(start..start + own)
            .ok_or_else(|| {
                SolveError::InvalidGame(format!("terminal {node} kernel is missing a column"))
            })?;
        for (slot, value) in out.iter_mut().zip(bits) {
            *slot = Real::from_bits(*value);
        }
        Ok(())
    }
}

/// What a completed path validation walked, so a caller can report its cover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathChecks {
    /// Nodes reached from the root, which must be every node in storage.
    pub nodes: usize,
    /// Chance nodes reached. Their outcome count, probabilities and mask shapes
    /// are checked on every walk; their mass only when the scope names pairs.
    pub chance_nodes: usize,
    /// Terminals reached. Their utilities were checked only with a column source.
    pub terminals: usize,
    /// Private-state pairs the two quadratic checks ran over. Zero under
    /// [`PairScope::NoPairs`], because neither of them ran.
    pub pairs: usize,
}

/// Edges from the root a validated tree may hold. The traversals are recursive,
/// so the depth is checked before anything uses the call stack.
const MAX_VALIDATED_DEPTH: usize = 256;

/// Checks the whole-tree contracts a local traversal cannot see: the structural
/// ones listed below, one unit of chance mass per compatible pair still legal
/// after ancestor masks, and zero-sum terminal utilities.
///
/// `compatible` answers for a (player-zero state, player-one state) pair.
/// Terminal utilities are checked only when `terminals` supplies them, because
/// reading them costs one evaluation per live state per terminal; the caller
/// decides whether that is affordable and reports what ran. Supplying a source
/// under a scope that resolves to no pairs is rejected rather than silently
/// walked, because the zero-sum check would then read no column at all and the
/// `Ok` would claim a check that never ran.
///
/// The structural half is linear in the nodes and runs on every tree, whatever
/// its size: reachability exactly once, no cycles or shared children, the depth
/// limit, declared child counts against action and outcome counts, chance
/// probabilities inside [0,1], mask shapes, and terminals without children.
/// Only the two per-pair checks are quadratic in the live states, and
/// [`PairScope::NoPairs`] turns just those off. Nothing outside the scoped
/// loops reads the per-node live flags, so with no scoped pairs the walk
/// carries empty flag vectors and stays linear in the nodes.
///
/// The zero-sum pass holds one utility per scoped pair between its two passes.
/// That buffer cannot be streamed away: the first pass reads a column of player
/// zero's utilities for one opponent state and the second a column of player
/// one's, so pairing them without keeping one of the two would cost one
/// terminal evaluation per pair instead of one per state. Callers charge it
/// instead; `streets::PostflopMemory` charges it at its pair budget.
pub(crate) fn validate_traversal(
    layout: &TraversalLayout,
    compatible: &dyn Fn(usize, usize) -> bool,
    scope: PairScope,
    mut terminals: Option<&mut dyn TerminalColumns>,
) -> Result<PathChecks, SolveError> {
    let invalid = SolveError::InvalidGame;
    let states = layout.states;
    let scoped: [Vec<usize>; 2] = std::array::from_fn(|player| match scope {
        PairScope::All => (0..states[player]).collect(),
        PairScope::PositiveWeight => (0..states[player])
            .filter(|state| layout.weights[player][*state] > 0.0)
            .collect(),
        PairScope::NoPairs => Vec::new(),
    });
    let width = scoped[1].len();
    let mut checks = PathChecks {
        // Exactly the pairs the two quadratic checks below walk, which is zero
        // when the scope names none: never a count they did not reach.
        pairs: scoped[0]
            .len()
            .checked_mul(width)
            .ok_or_else(|| invalid("scoped private-pair count overflows".into()))?,
        ..PathChecks::default()
    };
    // The zero-sum check holds one utility per scoped pair between its two
    // passes, plus one column wide enough for either player.
    if terminals.is_some() && checks.pairs == 0 {
        return Err(invalid(
            "a terminal column source needs a scope with pairs: this one resolves to none, \
             so the zero-sum check would read nothing"
                .into(),
        ));
    }
    let mut matrix = if terminals.is_some() {
        filled(checks.pairs, 0.0)?
    } else {
        Vec::new()
    };
    let mut column = filled(states[0].max(states[1]), 0.0)?;
    let mut visited = filled(layout.nodes.len(), false)?;
    let mut stack = reserved(1)?;
    // Live flags are read only inside the scoped pair loops, and every edge
    // clones them. With no scoped pairs the walk carries empty vectors, so the
    // mask update below iterates nothing and the pass stays linear.
    let root_live: [Vec<bool>; 2] = if checks.pairs > 0 {
        [filled(states[0], true)?, filled(states[1], true)?]
    } else {
        [Vec::new(), Vec::new()]
    };
    stack.push((layout.root, root_live, 0_usize));

    while let Some((id, live, depth)) = stack.pop() {
        if visited[id as usize] {
            return Err(invalid(
                "public nodes must form a tree without cycles or shared children".into(),
            ));
        }
        if depth > MAX_VALIDATED_DEPTH {
            return Err(invalid(format!(
                "tree exceeds phase 1 depth limit of {MAX_VALIDATED_DEPTH}"
            )));
        }
        visited[id as usize] = true;
        checks.nodes += 1;
        let node = &layout.nodes[id as usize];
        match node.kind {
            NodeKind::Player {
                player,
                num_actions,
            } => {
                if player > 1 || node.children.len() != num_actions as usize {
                    return Err(invalid(format!(
                        "player node {id} has {} children for {num_actions} actions",
                        node.children.len()
                    )));
                }
            }
            NodeKind::Chance { num_outcomes } => {
                checks.chance_nodes += 1;
                let outcomes = num_outcomes as usize;
                if node.children.len() != outcomes
                    || node.probabilities.len() != outcomes
                    || node.masks.len() != outcomes
                {
                    return Err(invalid(format!(
                        "chance node {id} declares {outcomes} outcomes but holds {} children, {} probabilities and {} masks",
                        node.children.len(),
                        node.probabilities.len(),
                        node.masks.len()
                    )));
                }
                for (outcome, index) in node.masks.iter().enumerate() {
                    let probability = node.probabilities[outcome];
                    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
                        return Err(invalid(format!(
                            "chance probability {probability} at node {id} outcome {outcome} is not in [0,1]"
                        )));
                    }
                    let pair = layout.mask_pool.get(*index).ok_or_else(|| {
                        invalid(format!(
                            "chance node {id} outcome {outcome} names mask entry {index}, outside the pool"
                        ))
                    })?;
                    for (player, entries) in pair.iter().enumerate() {
                        if entries.len() != states[player]
                            || entries.iter().any(|entry| *entry != 0.0 && *entry != 1.0)
                        {
                            return Err(invalid(format!(
                                "chance mask at node {id} outcome {outcome} needs a zero-or-one entry per private state"
                            )));
                        }
                    }
                }
                for &h0 in &scoped[0] {
                    if !live[0][h0] {
                        continue;
                    }
                    for &h1 in &scoped[1] {
                        if !live[1][h1] || !compatible(h0, h1) {
                            continue;
                        }
                        let mass: Real = node
                            .probabilities
                            .iter()
                            .zip(&node.masks)
                            .map(|(p, index)| {
                                let mask = &layout.mask_pool[*index];
                                p * mask[0][h0] * mask[1][h1]
                            })
                            .sum();
                        if (mass - 1.0).abs() > 1e-12 {
                            return Err(invalid(format!(
                                "chance mass at node {id} for ({h0},{h1}) is {mass}, expected one"
                            )));
                        }
                    }
                }
            }
            NodeKind::Terminal => {
                checks.terminals += 1;
                if !node.children.is_empty() {
                    return Err(invalid(format!(
                        "terminal {id} holds {} children",
                        node.children.len()
                    )));
                }
                if let Some(source) = &mut terminals {
                    for (j, &h1) in scoped[1].iter().enumerate() {
                        if !live[1][h1] {
                            continue;
                        }
                        source.column(id, 0, h1, &mut column[..states[0]])?;
                        for (i, &h0) in scoped[0].iter().enumerate() {
                            matrix[i * width + j] = column[h0];
                        }
                    }
                    for (i, &h0) in scoped[0].iter().enumerate() {
                        if !live[0][h0] {
                            continue;
                        }
                        source.column(id, 1, h0, &mut column[..states[1]])?;
                        for (j, &h1) in scoped[1].iter().enumerate() {
                            if !live[1][h1] || !compatible(h0, h1) {
                                continue;
                            }
                            let u0 = matrix[i * width + j];
                            let u1 = column[h1];
                            // Non-finite values fail in the first evaluation or
                            // update, carrying that operation's iteration and
                            // player context.
                            if u0.is_finite()
                                && u1.is_finite()
                                && normalized_sum(u0, u1).abs() > 1e-10
                            {
                                return Err(invalid(format!(
                                    "terminal {id} has non-zero-sum utilities for ({h0},{h1})"
                                )));
                            }
                        }
                    }
                }
            }
        }
        for (outcome, child) in node.children.iter().enumerate() {
            if *child as usize >= layout.nodes.len() {
                return Err(invalid(format!(
                    "node {id} names child {child}, outside node storage"
                )));
            }
            let mut next_live = live.clone();
            if matches!(node.kind, NodeKind::Chance { .. }) {
                let mask = layout.masks(node, outcome);
                for (player, entries) in next_live.iter_mut().enumerate() {
                    for (h, entry) in entries.iter_mut().enumerate() {
                        *entry &= mask[player][h] != 0.0;
                    }
                }
            }
            stack.push((*child, next_live, depth + 1));
        }
    }
    if visited.contains(&false) {
        return Err(invalid("node storage contains unreachable nodes".into()));
    }
    Ok(checks)
}

impl TraversalLayout {
    /// Both players' chance masks for one outcome, read through the pool.
    /// Panics only on an index this crate did not put there; every index comes
    /// from a checked construction that pushed the entry it names.
    pub fn masks(&self, node: &Node, outcome: usize) -> &[Vec<Real>; 2] {
        &self.mask_pool[node.masks[outcome]]
    }

    pub fn row_len(&self, node: usize) -> usize {
        match self.nodes[node].kind {
            NodeKind::Player {
                player,
                num_actions,
            } => self.states[player as usize] * num_actions as usize,
            _ => 0,
        }
    }
}

fn capture_kernel(game: &dyn Game, node: NodeId, states: [usize; 2]) -> [Vec<u64>; 2] {
    std::array::from_fn(|player| {
        let mut kernel = Vec::with_capacity(states[0] * states[1]);
        let mut opponent = vec![0.0; states[1 - player]];
        let mut out = vec![Real::NAN; states[player]];
        for state in 0..opponent.len() {
            opponent[state] = 1.0;
            out.fill(Real::NAN);
            game.terminal_values(node, player, &opponent, &mut out);
            kernel.extend(out.iter().map(|value| value.to_bits()));
            opponent[state] = 0.0;
        }
        kernel
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two sibling chance nodes deal the same three cards. Masks depend on the
    /// dealt card alone, which is what a turn or flop expansion produces.
    struct TwinChance {
        masks: Vec<Vec<Real>>,
        weights: Vec<Real>,
    }

    impl TwinChance {
        const STATES: usize = 3;

        fn new() -> Self {
            Self {
                masks: (0..Self::STATES)
                    .map(|card| {
                        (0..Self::STATES)
                            .map(|hand| if hand == card { 0.0 } else { 1.0 })
                            .collect()
                    })
                    .collect(),
                weights: vec![1.0; Self::STATES],
            }
        }
    }

    impl Game for TwinChance {
        fn num_nodes(&self) -> usize {
            9
        }
        fn root(&self) -> NodeId {
            0
        }
        fn kind(&self, node: NodeId) -> NodeKind {
            match node {
                0 => NodeKind::Player {
                    player: 0,
                    num_actions: 2,
                },
                1 | 2 => NodeKind::Chance { num_outcomes: 3 },
                _ => NodeKind::Terminal,
            }
        }
        fn child(&self, node: NodeId, index: usize) -> NodeId {
            match node {
                0 => 1 + index as NodeId,
                1 => 3 + index as NodeId,
                2 => 6 + index as NodeId,
                _ => unreachable!("terminals have no children"),
            }
        }
        fn num_private_states(&self, _player: usize) -> usize {
            Self::STATES
        }
        fn initial_weights(&self, _player: usize) -> &[Real] {
            &self.weights
        }
        fn compatible(&self, p0_state: usize, p1_state: usize) -> bool {
            p0_state != p1_state
        }
        // One card of the three survives each compatible pair, so the masked
        // sum is one even though each unmasked outcome carries probability one.
        fn chance_prob(&self, _node: NodeId, _outcome: usize) -> Real {
            1.0
        }
        fn chance_mask(&self, _node: NodeId, outcome: usize, _player: usize) -> &[Real] {
            &self.masks[outcome]
        }
        fn terminal_values(
            &self,
            _node: NodeId,
            _player: usize,
            _opp_reach: &[Real],
            out: &mut [Real],
        ) {
            out.fill(0.0);
        }
        fn starting_pot(&self) -> Real {
            2.0
        }
        fn info_label(&self, node: NodeId, player: usize, state: usize) -> String {
            format!("{node}:{player}:{state}")
        }
    }

    /// Three private states, one chance node dealing all three, and one
    /// terminal per outcome: the smallest tree with a mask that removes a state.
    fn dealt_layout() -> TraversalLayout {
        let masks: Vec<[Vec<Real>; 2]> = (0..3)
            .map(|card| {
                let entries: Vec<Real> = (0..3)
                    .map(|state| if state == card { 0.0 } else { 1.0 })
                    .collect();
                [entries.clone(), entries]
            })
            .collect();
        let mut nodes = vec![Node {
            kind: NodeKind::Chance { num_outcomes: 3 },
            children: vec![1, 2, 3],
            probabilities: vec![1.0; 3],
            masks: vec![0, 1, 2],
        }];
        for _ in 0..3 {
            nodes.push(Node {
                kind: NodeKind::Terminal,
                children: Vec::new(),
                probabilities: Vec::new(),
                masks: Vec::new(),
            });
        }
        TraversalLayout {
            root: 0,
            states: [3, 3],
            // Player zero never holds state two and player one never holds
            // state zero, so the positive-weight scope is a strict subset.
            weights: [vec![1.0, 1.0, 0.0], vec![0.0, 1.0, 1.0]],
            nodes,
            mask_pool: masks,
            normalizer: 3.0,
            pot: 2.0,
        }
    }

    /// Zero-sum utilities, unless `broken` flips one player's sign convention.
    struct Antisymmetric {
        broken: bool,
    }

    impl TerminalColumns for Antisymmetric {
        fn column(
            &mut self,
            node: NodeId,
            player: usize,
            _opponent: usize,
            out: &mut [Real],
        ) -> Result<(), SolveError> {
            let sign = if player == 0 { 1.0 } else { -1.0 };
            // Player zero is paid three at node two while player one still
            // loses one, so that terminal alone is not zero sum.
            let magnitude = if self.broken && node == 2 && player == 0 {
                3.0
            } else {
                1.0
            };
            out.fill(sign * magnitude);
            Ok(())
        }
    }

    #[test]
    fn a_traversal_layout_is_validated_without_a_callback_game() {
        let layout = dealt_layout();
        let compatible = |h0: usize, h1: usize| h0 != h1;
        let mut columns = Antisymmetric { broken: false };
        let checks = validate_traversal(
            &layout,
            &compatible,
            PairScope::PositiveWeight,
            Some(&mut columns),
        )
        .unwrap();
        // The fixture is one chance node over three cards with one terminal per
        // card: 1 + 3 = 4 nodes, 1 chance node, 3 terminals. Player zero holds
        // states 0 and 1 and player one states 1 and 2, so the positive-weight
        // scope is 2 * 2 = 4 pairs, of which (1,1) is filtered as incompatible
        // inside the walk.
        assert_eq!(checks.nodes, 4);
        assert_eq!(checks.chance_nodes, 1);
        assert_eq!(checks.terminals, 3);
        assert_eq!(checks.pairs, 4);

        // The whole-state scope walks every declared pair instead: 3 * 3 = 9.
        let wide = validate_traversal(&layout, &compatible, PairScope::All, None).unwrap();
        assert_eq!(wide.nodes, 4);
        assert_eq!(wide.chance_nodes, 1);
        assert_eq!(wide.terminals, 3);
        assert_eq!(wide.pairs, 9);
    }

    #[test]
    fn the_no_pair_scope_still_walks_every_structural_contract_and_reports_no_pairs() {
        let compatible = |h0: usize, h1: usize| h0 != h1;

        // The same four nodes, one chance node and three terminals, with no
        // pairs at all: the quadratic half is off, the linear half is not.
        let layout = dealt_layout();
        let checks = validate_traversal(&layout, &compatible, PairScope::NoPairs, None).unwrap();
        assert_eq!(checks.nodes, 4);
        assert_eq!(checks.chance_nodes, 1);
        assert_eq!(checks.terminals, 3);
        assert_eq!(checks.pairs, 0);

        // A broken chance mass is exactly what this scope stops checking, so it
        // passes here and still fails under a scope that names pairs.
        let mut layout = dealt_layout();
        layout.nodes[0].probabilities[0] = 0.5;
        assert_eq!(
            validate_traversal(&layout, &compatible, PairScope::NoPairs, None)
                .unwrap()
                .pairs,
            0
        );
        assert!(
            validate_traversal(&layout, &compatible, PairScope::PositiveWeight, None).is_err(),
            "the pair scope must still catch the mass this scope skips"
        );

        // The structural contracts are the ones that keep running: a shared
        // child, an out-of-range probability and an unreachable node all fail.
        let mut layout = dealt_layout();
        layout.nodes[0].children[2] = 2;
        let error = validate_traversal(&layout, &compatible, PairScope::NoPairs, None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("without cycles or shared children"),
            "{error}"
        );

        let mut layout = dealt_layout();
        layout.nodes[0].probabilities[0] = 2.0;
        let error = validate_traversal(&layout, &compatible, PairScope::NoPairs, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("is not in [0,1]"), "{error}");

        let mut layout = dealt_layout();
        layout.nodes.push(Node {
            kind: NodeKind::Terminal,
            children: Vec::new(),
            probabilities: Vec::new(),
            masks: Vec::new(),
        });
        let error = validate_traversal(&layout, &compatible, PairScope::NoPairs, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unreachable nodes"), "{error}");
    }

    #[test]
    fn a_column_source_without_pairs_to_check_is_refused_rather_than_reported_as_run() {
        let layout = dealt_layout();
        let compatible = |h0: usize, h1: usize| h0 != h1;

        // The source would never be read under this scope, so an `Ok` here
        // would report a zero-sum check that walked nothing.
        let mut columns = Antisymmetric { broken: false };
        let error =
            validate_traversal(&layout, &compatible, PairScope::NoPairs, Some(&mut columns))
                .unwrap_err()
                .to_string();
        assert!(error.contains("needs a scope with pairs"), "{error}");

        // A scope that does name pairs takes the same source.
        let mut columns = Antisymmetric { broken: false };
        assert_eq!(
            validate_traversal(
                &layout,
                &compatible,
                PairScope::PositiveWeight,
                Some(&mut columns)
            )
            .unwrap()
            .pairs,
            4
        );
    }

    #[test]
    fn the_split_validation_still_names_a_broken_mass_terminal_or_reachability() {
        let compatible = |h0: usize, h1: usize| h0 != h1;

        // States one and two both survive the deal of state zero, so halving
        // that outcome's probability leaves them a mass of one half.
        let mut layout = dealt_layout();
        layout.nodes[0].probabilities[0] = 0.5;
        let error = validate_traversal(&layout, &compatible, PairScope::PositiveWeight, None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("chance mass at node 0 for (1,2) is 0.5"),
            "{error}"
        );

        let mut layout = dealt_layout();
        layout.nodes[0].children[2] = 2;
        let error = validate_traversal(&layout, &compatible, PairScope::All, None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("without cycles or shared children"),
            "{error}"
        );

        let mut layout = dealt_layout();
        layout.nodes.push(Node {
            kind: NodeKind::Terminal,
            children: Vec::new(),
            probabilities: Vec::new(),
            masks: Vec::new(),
        });
        let error = validate_traversal(&layout, &compatible, PairScope::All, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unreachable nodes"), "{error}");

        let layout = dealt_layout();
        let mut columns = Antisymmetric { broken: true };
        let error = validate_traversal(
            &layout,
            &compatible,
            PairScope::PositiveWeight,
            Some(&mut columns),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("terminal 2 has non-zero-sum"), "{error}");
    }

    #[test]
    fn chance_nodes_dealing_the_same_card_share_one_mask_pool_entry() {
        let game = TwinChance::new();
        let layout = Layout::new(&game).unwrap();
        let first = &layout.nodes[1];
        let second = &layout.nodes[2];
        assert!(matches!(first.kind, NodeKind::Chance { .. }));
        assert!(matches!(second.kind, NodeKind::Chance { .. }));

        // Six (node, outcome) pairs, three distinct cards, one entry per card.
        assert_eq!(first.masks.len() + second.masks.len(), 6);
        assert_eq!(layout.mask_pool.len(), TwinChance::STATES);
        assert_eq!(first.masks, second.masks);
        assert_eq!(first.masks, vec![0, 1, 2]);

        // Sharing an entry must not change the bits either walk reads.
        for outcome in 0..TwinChance::STATES {
            for node in [first, second] {
                for (player, entries) in layout.masks(node, outcome).iter().enumerate() {
                    assert_eq!(entries.as_slice(), game.chance_mask(1, outcome, player));
                }
            }
        }
    }
}
