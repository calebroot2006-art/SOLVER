//! Probability and information-set contract for a two-player public tree.
use crate::{SolveError, error::normalized_sum};
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

    fn validate_paths(&self) -> Result<(), SolveError> {
        let mut visited = vec![false; self.nodes.len()];
        let mut stack = vec![(
            self.root,
            [vec![1.0; self.states[0]], vec![1.0; self.states[1]]],
            0usize,
        )];
        while let Some((id, live, depth)) = stack.pop() {
            if visited[id as usize] {
                return Err(SolveError::InvalidGame(
                    "public nodes must form a tree without cycles or shared children".into(),
                ));
            }
            // Traversal is recursive; validate the depth before using the call stack.
            if depth > 256 {
                return Err(SolveError::InvalidGame(
                    "tree exceeds phase 1 depth limit of 256".into(),
                ));
            }
            visited[id as usize] = true;
            let node = &self.nodes[id as usize];
            if node.kind == NodeKind::Terminal {
                for h0 in 0..self.states[0] {
                    for h1 in 0..self.states[1] {
                        if live[0][h0] == 0.0
                            || live[1][h1] == 0.0
                            || !self.compatible[h0 * self.states[1] + h1]
                        {
                            continue;
                        }
                        let u0 = Real::from_bits(
                            self.terminal_kernels[id as usize][0][h1 * self.states[0] + h0],
                        );
                        let u1 = Real::from_bits(
                            self.terminal_kernels[id as usize][1][h0 * self.states[1] + h1],
                        );
                        // Non-finite values fail in the first evaluation/update,
                        // carrying that operation's iteration and player context.
                        if u0.is_finite() && u1.is_finite() && normalized_sum(u0, u1).abs() > 1e-10
                        {
                            return Err(SolveError::InvalidGame(format!(
                                "terminal {id} has non-zero-sum utilities for ({h0},{h1})"
                            )));
                        }
                    }
                }
            }
            if matches!(node.kind, NodeKind::Chance { .. }) {
                for h0 in 0..self.states[0] {
                    for h1 in 0..self.states[1] {
                        if live[0][h0] == 0.0
                            || live[1][h1] == 0.0
                            || !self.compatible[h0 * self.states[1] + h1]
                        {
                            continue;
                        }
                        let mass: Real = node
                            .probabilities
                            .iter()
                            .zip(&node.masks)
                            .map(|(p, index)| {
                                let mask = &self.mask_pool[*index];
                                p * mask[0][h0] * mask[1][h1]
                            })
                            .sum();
                        if (mass - 1.0).abs() > 1e-12 {
                            return Err(SolveError::InvalidGame(format!(
                                "chance mass at node {id} for ({h0},{h1}) is {mass}, expected one"
                            )));
                        }
                    }
                }
            }
            for (outcome, child) in node.children.iter().enumerate() {
                let mut next_live = live.clone();
                if matches!(node.kind, NodeKind::Chance { .. }) {
                    let mask = self.masks(node, outcome);
                    for (player, entries) in next_live.iter_mut().enumerate() {
                        for (h, entry) in entries.iter_mut().enumerate() {
                            *entry *= mask[player][h];
                        }
                    }
                }
                stack.push((*child, next_live, depth + 1));
            }
        }
        if visited.contains(&false) {
            return Err(SolveError::InvalidGame(
                "node storage contains unreachable nodes".into(),
            ));
        }
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
