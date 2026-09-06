use crate::{BetSize, BetSizeOptions, Chips, MAX_CHIPS, NodeId, TreeError, reserve};
use std::fmt;

/// Explicit rules and resource budget for a heads-up river tree.
#[derive(Clone, Debug, PartialEq)]
pub struct RiverTreeConfig {
    /// Existing pot before any river action; must be positive.
    pub starting_pot: Chips,
    /// Chips each player can commit on this river; zero means immediate showdown.
    pub effective_stack: Chips,
    /// Smallest full opening bet and minimum full raise increment.
    pub min_bet: Chips,
    /// Separate menus for player zero (out of position) and player one.
    pub sizes: [BetSizeOptions; 2],
    /// Maximum raises after the initial bet, from zero through 32.
    pub max_raises: u8,
    /// Add an all-in when this pot fraction reaches the remaining wager target.
    pub add_all_in_threshold: f64,
    /// Replace a candidate with all-in when its remaining stack is this small.
    pub force_all_in_threshold: f64,
    /// Maximum public nodes, including terminals, from one through one million.
    pub max_nodes: usize,
}

impl RiverTreeConfig {
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

/// Legal action; wager amounts are total contributions on this river.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    /// Surrender the pot to the opponent.
    Fold,
    /// Pass when no wager is outstanding.
    Check,
    /// Match the opponent's wager and reach showdown.
    Call,
    /// Open betting to this total river contribution.
    Bet(Chips),
    /// Raise to this total river contribution.
    Raise(Chips),
    /// Commit the full effective stack.
    AllIn(Chips),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fold => f.write_str("fold"),
            Self::Check => f.write_str("check"),
            Self::Call => f.write_str("call"),
            Self::Bet(value) => write!(f, "bet:{value}"),
            Self::Raise(value) => write!(f, "raise:{value}"),
            Self::AllIn(value) => write!(f, "allin:{value}"),
        }
    }
}

/// How a river history ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terminal {
    /// One player folded; the named player wins without comparing cards.
    Fold {
        /// Zero-based winner, either zero or one.
        winner: u8,
    },
    /// Both players remain and their hands determine the pot allocation.
    Showdown,
}

/// Operation at a public river history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiverNodeKind {
    /// The named player chooses among this node's actions.
    Decision {
        /// Zero-based acting player, either zero or one.
        player: u8,
    },
    /// Betting has ended.
    Terminal(Terminal),
}

/// One immutable history, with ordered actions and corresponding child IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverNode {
    kind: RiverNodeKind,
    contributions: [Chips; 2],
    actions: Vec<Action>,
    children: Vec<NodeId>,
}

impl RiverNode {
    /// Decision actor or terminal outcome.
    #[must_use]
    pub fn kind(&self) -> RiverNodeKind {
        self.kind
    }

    /// Chips committed on this river; terminals exclude refunded excess wagers.
    #[must_use]
    pub fn contributions(&self) -> [Chips; 2] {
        self.contributions
    }

    /// Legal actions sorted by `Action` order; empty at terminals.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// One valid child per action, in the same order; empty at terminals.
    #[must_use]
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

/// Checked heads-up river histories. Distinct histories never share a node.
#[derive(Clone, Debug, PartialEq)]
pub struct RiverTree {
    config: RiverTreeConfig,
    nodes: Vec<RiverNode>,
    max_depth: usize,
}

impl RiverTree {
    /// Builds an entire tree, rejecting invalid settings and exhausted budgets.
    /// Construction has no side effects and never returns a partial tree.
    pub fn new(config: RiverTreeConfig) -> Result<Self, TreeError> {
        config.validate()?;
        let mut tree = Self {
            config,
            nodes: Vec::new(),
            max_depth: 0,
        };
        let terminal = (tree.config.effective_stack == 0).then_some(Terminal::Showdown);
        let state = State {
            contributions: [0; 2],
            player: 0,
            checked: false,
            raises: 0,
            terminal,
        };
        let mut edges = 0;
        tree.build(state, 0, &mut edges)?;
        Ok(tree)
    }

    /// Validated construction settings.
    #[must_use]
    pub fn config(&self) -> &RiverTreeConfig {
        &self.config
    }

    /// Root node, always zero.
    #[must_use]
    pub fn root(&self) -> NodeId {
        0
    }

    /// Nodes in deterministic depth-first construction order.
    #[must_use]
    pub fn nodes(&self) -> &[RiverNode] {
        &self.nodes
    }

    /// Reads a node, or returns `None` for an out-of-range ID.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&RiverNode> {
        self.nodes.get(id as usize)
    }

    /// Greatest number of edges from the root to a node.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Inline storage plus actual node, action, child, and menu buffer capacities.
    #[must_use]
    pub fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.nodes.capacity() * std::mem::size_of::<RiverNode>()
            + self
                .config
                .sizes
                .iter()
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

    fn build(
        &mut self,
        state: State,
        depth: usize,
        edges: &mut usize,
    ) -> Result<NodeId, TreeError> {
        if depth > 128 {
            return Err(TreeError::new("river tree exceeds depth limit 128"));
        }
        if self.nodes.len() >= self.config.max_nodes {
            return Err(TreeError::new("river tree exceeds max_nodes"));
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
        self.nodes.push(RiverNode {
            kind: state.terminal.map_or(
                RiverNodeKind::Decision {
                    player: state.player,
                },
                RiverNodeKind::Terminal,
            ),
            contributions: state.contributions,
            actions: Vec::new(),
            children: Vec::new(),
        });
        if state.terminal.is_some() {
            return Ok(id);
        }
        let actions = self.actions(state)?;
        // Count every promised child before allocating its edge buffer. Pending
        // siblings retain their slots while a deeper subtree is built.
        *edges = edges
            .checked_add(actions.len())
            .ok_or_else(|| TreeError::new("edge count overflow"))?;
        if *edges >= self.config.max_nodes {
            return Err(TreeError::new("river tree exceeds max_nodes"));
        }
        let mut children = Vec::new();
        reserve(&mut children, actions.len())?;
        for action in &actions {
            children.push(self.build(state.after(*action), depth + 1, edges)?);
        }
        self.nodes[id as usize].actions = actions;
        self.nodes[id as usize].children = children;
        Ok(id)
    }

    fn actions(&self, state: State) -> Result<Vec<Action>, TreeError> {
        let cfg = &self.config;
        let player = usize::from(state.player);
        let highest = state.contributions[1 - player];
        let facing = highest > state.contributions[player];
        let menu = if facing {
            cfg.sizes[player].raises()
        } else {
            cfg.sizes[player].bets()
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
        let minimum = if facing {
            highest
                .checked_add(call.max(cfg.min_bet))
                .ok_or_else(|| TreeError::new("minimum raise overflow"))?
        } else {
            cfg.min_bet
        }
        .min(cfg.effective_stack);
        let pot = pot_after(cfg.starting_pot, highest)?;
        for size in menu {
            let target = match *size {
                BetSize::Pot(ratio) => highest as f64 + rounded_product(pot, ratio)?,
                BetSize::PreviousBet(ratio) => rounded_product(highest, ratio)?,
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

#[derive(Clone, Copy)]
struct State {
    contributions: [Chips; 2],
    player: u8,
    checked: bool,
    raises: u8,
    terminal: Option<Terminal>,
}

impl State {
    fn after(mut self, action: Action) -> Self {
        let player = usize::from(self.player);
        let highest = self.contributions[1 - player];
        match action {
            Action::Fold => {
                self.contributions[1 - player] = self.contributions[player];
                self.terminal = Some(Terminal::Fold {
                    winner: 1 - self.player,
                });
            }
            Action::Call => {
                self.contributions[player] = highest;
                self.terminal = Some(Terminal::Showdown);
            }
            Action::Check => {
                if self.checked {
                    self.terminal = Some(Terminal::Showdown);
                }
                self.checked = true;
            }
            Action::Bet(target) | Action::Raise(target) | Action::AllIn(target) => {
                if highest > self.contributions[player] {
                    self.raises += 1;
                }
                self.contributions[player] = target;
                self.checked = false;
            }
        }
        self.player = 1 - self.player;
        self
    }
}

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
