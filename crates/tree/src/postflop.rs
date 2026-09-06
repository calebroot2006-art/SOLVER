use crate::{
    Action, BetSize, BetSizeOptions, Chips, MAX_CHIPS, NodeId, Terminal, TreeError, reserve,
};
use std::fmt;

/// Deepest history, counted in edges from the root, that construction expands.
/// Three streets of 32 raises each plus two deals reach 107, so the river's
/// limit still bounds a full flop tree.
const MAX_DEPTH: usize = 128;

/// Postflop street. The discriminant indexes the per-street configuration and
/// the per-street counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Street {
    /// Three board cards are known and two more will be dealt.
    Flop = 0,
    /// Four board cards are known and one more will be dealt.
    Turn = 1,
    /// The board is complete; betting here ends the hand.
    River = 2,
}

impl Street {
    /// Street dealt after this one, or `None` on the river.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Flop => Some(Self::Turn),
            Self::Turn => Some(Self::River),
            Self::River => None,
        }
    }

    /// Index into `PostflopTreeConfig::sizes` and the per-street counters.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Street {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Flop => "flop",
            Self::Turn => "turn",
            Self::River => "river",
        })
    }
}

/// Explicit rules and resource budget for a heads-up postflop tree.
///
/// Every field except `start_street` and `sizes` means what the river tree's
/// field of the same name means. `effective_stack` is the total each player may
/// commit across all remaining streets, while `min_bet` and `max_raises` apply
/// once per street.
#[derive(Clone, Debug, PartialEq)]
pub struct PostflopTreeConfig {
    /// Pot before any action on `start_street`; must be positive.
    pub starting_pot: Chips,
    /// Chips each player can commit across all remaining streets.
    pub effective_stack: Chips,
    /// Smallest full opening bet and minimum full raise increment, per street.
    pub min_bet: Chips,
    /// Street the root belongs to; earlier streets' menus are never read.
    pub start_street: Street,
    /// Menus by street then by player: `sizes[street.index()][player]`.
    pub sizes: [[BetSizeOptions; 2]; 3],
    /// Maximum raises after the initial bet on one street, from zero through 32.
    pub max_raises: u8,
    /// Add an all-in when this pot fraction reaches the remaining wager target.
    pub add_all_in_threshold: f64,
    /// Replace a candidate with all-in when its remaining stack is this small.
    pub force_all_in_threshold: f64,
    /// Maximum public nodes, including chance and terminal nodes.
    pub max_nodes: usize,
}

impl PostflopTreeConfig {
    fn validate(&self) -> Result<(), TreeError> {
        if self.starting_pot == 0 || self.min_bet == 0 {
            return Err(TreeError::new(
                "starting pot and minimum bet must be positive",
            ));
        }
        if [self.starting_pot, self.effective_stack, self.min_bet]
            .iter()
            .any(|value| *value > MAX_CHIPS)
        {
            return Err(TreeError::new("chip settings must not exceed 1000000000"));
        }
        if self.max_raises > 32 || !(1..=1_000_000).contains(&self.max_nodes) {
            return Err(TreeError::new(
                "raise cap must be at most 32 and node budget in 1..=1000000",
            ));
        }
        if [self.add_all_in_threshold, self.force_all_in_threshold]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(TreeError::new(
                "all-in thresholds must be finite and nonnegative",
            ));
        }
        Ok(())
    }
}

/// Operation at a public postflop history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostflopNodeKind {
    /// The named player chooses among this node's actions.
    Decision {
        /// Zero-based acting player, either zero or one.
        player: u8,
    },
    /// Betting on this node's street is over and the next card is dealt. The
    /// node has no actions and exactly one child, the root of the dealt street.
    /// The runouts the deal stands for are expanded by the solver, not here.
    Chance {
        /// Street the single child belongs to.
        next: Street,
    },
    /// The hand is over.
    Terminal(Terminal),
}

/// One immutable history, with ordered actions and corresponding child IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostflopNode {
    kind: PostflopNodeKind,
    street: Street,
    contributions: [Chips; 2],
    actions: Vec<Action>,
    children: Vec<NodeId>,
}

impl PostflopNode {
    /// Decision actor, chance transition, or terminal outcome.
    #[must_use]
    pub fn kind(&self) -> PostflopNodeKind {
        self.kind
    }

    /// Street this history belongs to. A chance node reports the street whose
    /// betting just ended, not the street it deals.
    #[must_use]
    pub fn street(&self) -> Street {
        self.street
    }

    /// Chips committed since the root, across every street, excluding refunded
    /// excess wagers at a terminal. Never above the effective stack.
    #[must_use]
    pub fn contributions(&self) -> [Chips; 2] {
        self.contributions
    }

    /// Legal actions sorted by `Action` order; empty at chance and terminal nodes.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// One child per action at a decision node, in the same order; exactly one
    /// child at a chance node; empty at a terminal.
    #[must_use]
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

/// Checked heads-up postflop histories. Distinct histories never share a node.
///
/// This is the compact form of the game: one chance node stands for the whole
/// set of runouts of a street transition. Betting rules, size syntax, rounding,
/// thresholds, and the raise cap are the river rules applied once per street,
/// so `start_street: Street::River` reproduces `RiverTree` node for node.
#[derive(Clone, Debug, PartialEq)]
pub struct PostflopTree {
    config: PostflopTreeConfig,
    nodes: Vec<PostflopNode>,
    max_depth: usize,
    decision_nodes: [usize; 3],
    live_continuations: [usize; 3],
}

impl PostflopTree {
    /// Builds an entire tree, rejecting invalid settings and exhausted budgets.
    /// Construction has no side effects and never returns a partial tree.
    pub fn new(config: PostflopTreeConfig) -> Result<Self, TreeError> {
        config.validate()?;
        let mut tree = Self {
            config,
            nodes: Vec::new(),
            max_depth: 0,
            decision_nodes: [0; 3],
            live_continuations: [0; 3],
        };
        let root = State::opening(tree.config.start_street, tree.config.effective_stack);
        let mut edges = 0;
        tree.build(root, 0, &mut edges)?;
        Ok(tree)
    }

    /// Validated construction settings.
    #[must_use]
    pub fn config(&self) -> &PostflopTreeConfig {
        &self.config
    }

    /// Root node, always zero.
    #[must_use]
    pub fn root(&self) -> NodeId {
        0
    }

    /// Nodes in deterministic depth-first construction order.
    #[must_use]
    pub fn nodes(&self) -> &[PostflopNode] {
        &self.nodes
    }

    /// Reads a node, or returns `None` for an out-of-range ID.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&PostflopNode> {
        self.nodes.get(id as usize)
    }

    /// Greatest number of edges from the root to a node, counting deals.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Inline storage plus actual node, action, child, and menu buffer capacities.
    #[must_use]
    pub fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.nodes.capacity() * std::mem::size_of::<PostflopNode>()
            + self
                .config
                .sizes
                .iter()
                .flatten()
                .map(BetSizeOptions::storage_bytes)
                .sum::<usize>()
            + self
                .nodes
                .iter()
                .map(|node| {
                    node.actions.capacity() * std::mem::size_of::<Action>()
                        + node.children.capacity() * std::mem::size_of::<NodeId>()
                })
                .sum::<usize>()
    }

    /// Decision nodes on each street, indexed by `Street::index`.
    ///
    /// These are compact-tree counts. A solver that expands one chance node into
    /// its runouts multiplies the streets below that node by the runout count.
    #[must_use]
    pub fn decision_nodes_per_street(&self) -> [usize; 3] {
        self.decision_nodes
    }

    /// Histories that finish each street with both players contesting the pot
    /// and chips still behind, indexed by `Street::index`.
    ///
    /// Before the river each one is a chance node, so this is the branching
    /// factor into the next street's betting. On the river each one is a
    /// showdown that no called all-in produced. Lines where a call put both
    /// players all in are excluded: they run the board out with no decisions.
    #[must_use]
    pub fn live_continuations_per_street(&self) -> [usize; 3] {
        self.live_continuations
    }

    fn build(
        &mut self,
        state: State,
        depth: usize,
        edges: &mut usize,
    ) -> Result<NodeId, TreeError> {
        if depth > MAX_DEPTH {
            return Err(TreeError::new(format!(
                "postflop tree exceeds depth limit {MAX_DEPTH}"
            )));
        }
        if self.nodes.len() >= self.config.max_nodes {
            return Err(TreeError::new("postflop tree exceeds max_nodes"));
        }
        if self.nodes.len() == self.nodes.capacity() {
            let capacity = self
                .nodes
                .capacity()
                .saturating_mul(2)
                .clamp(1, self.config.max_nodes);
            let additional = capacity - self.nodes.len();
            reserve(&mut self.nodes, additional)?;
        }
        let id = self.nodes.len() as NodeId;
        self.max_depth = self.max_depth.max(depth);
        let street = state.street;
        let behind = state.contributions[0] < self.config.effective_stack;
        self.nodes.push(PostflopNode {
            kind: state.stage.kind(state.player),
            street,
            contributions: state.contributions,
            actions: Vec::new(),
            children: Vec::new(),
        });
        match state.stage {
            Stage::Terminal(terminal) => {
                if behind && street == Street::River && terminal == Terminal::Showdown {
                    self.live_continuations[street.index()] += 1;
                }
                Ok(id)
            }
            Stage::Chance { next } => {
                if behind {
                    self.live_continuations[street.index()] += 1;
                }
                self.reserve_edges(edges, 1)?;
                let mut children = Vec::new();
                reserve(&mut children, 1)?;
                let dealt =
                    State::opening_after(state.contributions, next, self.config.effective_stack);
                children.push(self.build(dealt, depth + 1, edges)?);
                self.nodes[id as usize].children = children;
                Ok(id)
            }
            Stage::Decision => {
                self.decision_nodes[street.index()] += 1;
                let actions = self.actions(state)?;
                self.reserve_edges(edges, actions.len())?;
                let mut children = Vec::new();
                reserve(&mut children, actions.len())?;
                for action in &actions {
                    children.push(self.build(state.after(*action), depth + 1, edges)?);
                }
                self.nodes[id as usize].actions = actions;
                self.nodes[id as usize].children = children;
                Ok(id)
            }
        }
    }

    /// Counts every promised child before its edge buffer is allocated, so
    /// pending siblings keep their slots while a deeper subtree is built.
    fn reserve_edges(&self, edges: &mut usize, count: usize) -> Result<(), TreeError> {
        *edges = edges
            .checked_add(count)
            .ok_or_else(|| TreeError::new("edge count overflow"))?;
        if *edges >= self.config.max_nodes {
            return Err(TreeError::new("postflop tree exceeds max_nodes"));
        }
        Ok(())
    }

    fn actions(&self, state: State) -> Result<Vec<Action>, TreeError> {
        let cfg = &self.config;
        let player = usize::from(state.player);
        let highest = state.contributions[1 - player];
        let facing = highest > state.contributions[player];
        let options = &cfg.sizes[state.street.index()][player];
        let menu = if facing {
            options.raises()
        } else {
            options.bets()
        };
        let capped = facing
            && (state.raises >= cfg.max_raises
                || state.contributions.contains(&cfg.effective_stack));
        let mut actions = Vec::new();
        reserve(&mut actions, if capped { 2 } else { menu.len() + 3 })?;
        if facing {
            actions.extend([Action::Fold, Action::Call]);
        } else {
            actions.push(Action::Check);
        }
        if capped {
            return Ok(actions);
        }
        let call = highest - state.contributions[player];
        // Minimums are street-relative: an unopened bet is `min_bet` above the
        // level both players carried into this street.
        let minimum = if facing {
            highest
                .checked_add(call.max(cfg.min_bet))
                .ok_or_else(|| TreeError::new("minimum raise overflow"))?
        } else {
            state
                .base
                .checked_add(cfg.min_bet)
                .ok_or_else(|| TreeError::new("minimum bet overflow"))?
        }
        .min(cfg.effective_stack);
        let pot = pot_after(cfg.starting_pot, highest)?;
        for size in menu {
            let target = match *size {
                BetSize::Pot(ratio) => highest as f64 + rounded_product(pot, ratio)?,
                // A raise-to multiplier scales the wager made on this street,
                // not the chips carried in from the earlier ones.
                BetSize::PreviousBet(ratio) => {
                    state.base as f64 + rounded_product(highest - state.base, ratio)?
                }
                BetSize::Additive(increment) => highest
                    .checked_add(increment)
                    .ok_or_else(|| TreeError::new("additive wager overflow"))?
                    as f64,
                BetSize::AllIn => cfg.effective_stack as f64,
            };
            if !target.is_finite() {
                return Err(TreeError::new("wager target overflow"));
            }
            // Clamp while still floating: a huge finite option may exceed u64.
            let target = target.clamp(minimum as f64, cfg.effective_stack as f64) as Chips;
            let remaining = cfg.effective_stack - target;
            let force = rounded_product(
                pot_after(cfg.starting_pot, target)?,
                cfg.force_all_in_threshold,
            )?;
            let target = if remaining as f64 <= force {
                cfg.effective_stack
            } else {
                target
            };
            add_action(&mut actions, target, highest, cfg.effective_stack, facing);
        }
        let added = highest as f64 + rounded_product(pot, cfg.add_all_in_threshold)?;
        if !added.is_finite() {
            return Err(TreeError::new("all-in threshold target overflow"));
        }
        if cfg.effective_stack as f64 <= added {
            add_action(
                &mut actions,
                cfg.effective_stack,
                highest,
                cfg.effective_stack,
                facing,
            );
        }
        actions.sort_unstable();
        actions.dedup();
        Ok(actions)
    }
}

/// What a node does, decided before its children exist.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Decision,
    Chance { next: Street },
    Terminal(Terminal),
}

impl Stage {
    fn kind(self, player: u8) -> PostflopNodeKind {
        match self {
            Self::Decision => PostflopNodeKind::Decision { player },
            Self::Chance { next } => PostflopNodeKind::Chance { next },
            Self::Terminal(terminal) => PostflopNodeKind::Terminal(terminal),
        }
    }
}

#[derive(Clone, Copy)]
struct State {
    contributions: [Chips; 2],
    /// Contribution level both players carried into `street`.
    base: Chips,
    street: Street,
    player: u8,
    checked: bool,
    raises: u8,
    stage: Stage,
}

impl State {
    /// Root of `street`, with nothing committed yet.
    fn opening(street: Street, stack: Chips) -> Self {
        Self::opening_after([0; 2], street, stack)
    }

    /// Root of `street` after both players committed `contributions`, which are
    /// equal because a street ends only on a call or a second check. With no
    /// chips behind there is nothing to decide, so `settled` deals the rest of
    /// the board out to a showdown.
    fn opening_after(contributions: [Chips; 2], street: Street, stack: Chips) -> Self {
        let stage = if contributions[0] == stack {
            Self::settled(street)
        } else {
            Stage::Decision
        };
        Self {
            contributions,
            base: contributions[0],
            street,
            player: 0,
            checked: false,
            raises: 0,
            stage,
        }
    }

    /// Betting on `street` is over with both players still in the hand.
    fn settled(street: Street) -> Stage {
        match street.next() {
            Some(next) => Stage::Chance { next },
            None => Stage::Terminal(Terminal::Showdown),
        }
    }

    fn after(mut self, action: Action) -> Self {
        let player = usize::from(self.player);
        let highest = self.contributions[1 - player];
        let ends_street = match action {
            Action::Fold => {
                self.contributions[1 - player] = self.contributions[player];
                self.stage = Stage::Terminal(Terminal::Fold {
                    winner: 1 - self.player,
                });
                self.player = 1 - self.player;
                return self;
            }
            Action::Call => {
                self.contributions[player] = highest;
                true
            }
            Action::Check => {
                let second = self.checked;
                self.checked = true;
                second
            }
            Action::Bet(target) | Action::Raise(target) | Action::AllIn(target) => {
                if highest > self.contributions[player] {
                    self.raises += 1;
                }
                self.contributions[player] = target;
                self.checked = false;
                false
            }
        };
        if ends_street {
            self.stage = Self::settled(self.street);
        }
        self.player = 1 - self.player;
        self
    }
}

// river.rs is frozen as the phase 3 regression baseline, so these three helpers
// are copied here instead of being shared out of it. The river-equivalence test
// in tests/postflop.rs fails the moment the two copies disagree.

fn add_action(
    actions: &mut Vec<Action>,
    target: Chips,
    highest: Chips,
    stack: Chips,
    facing: bool,
) {
    if target > highest {
        actions.push(if target == stack {
            Action::AllIn(target)
        } else if facing {
            Action::Raise(target)
        } else {
            Action::Bet(target)
        });
    }
}

fn pot_after(starting_pot: Chips, contribution: Chips) -> Result<Chips, TreeError> {
    contribution
        .checked_mul(2)
        .and_then(|chips| starting_pot.checked_add(chips))
        .ok_or_else(|| TreeError::new("pot arithmetic overflow"))
}

fn rounded_product(chips: Chips, ratio: f64) -> Result<f64, TreeError> {
    let value = (chips as f64 * ratio).round();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(TreeError::new("bet size or threshold arithmetic overflow"))
    }
}
