use super::memory::PostflopMemory;
use super::terminal::{Payoff, TerminalContext};
use crate::memory::Budget;
use crate::{
    NodeId, NodeKind, Precision, Real, SolveError,
    allocation::{collect, filled, reserved},
    game::{Node, TraversalLayout},
    terminal::{OutcomeUtilities, ShowdownTable, evaluate_fold},
};
use cards::{Card, CardSet, Combo, Range};
use std::{collections::HashMap, fmt, ops, sync::Arc};
use tree::{Action, Chips, PostflopNodeKind, PostflopTree, Street, Terminal};

/// Private states per player, one per unordered two-card combination.
const STATES: usize = 1326;
/// Cards held by the two players, which a runout can never repeat.
const PRIVATE_CARDS: usize = 4;
/// The largest memory limit a game will accept, matching the river's ceiling.
const MEMORY_CEILING: u128 = 16 * 1024 * 1024 * 1024;
/// Edges from the root the expansion will follow. The compact tree stops at
/// 128; this recursion is bounded again so a future tree cannot overflow the
/// stack silently.
const MAX_EXPANSION_DEPTH: usize = 256;

/// How a postflop game is built and run.
///
/// Every field is configuration, not a magic constant: the limit and the
/// storage width come from the solver's configuration file and the worker count
/// from `solve.threads`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostflopOptions {
    /// Ceiling on everything this game, its solver, its snapshots and its
    /// query workspaces may hold at once. Positive and at most 16 GiB.
    pub memory_limit_bytes: usize,
    /// Storage width for regrets and strategy sums. Only `f64` is implemented.
    pub precision: Precision,
    /// Requested traversal workers: zero asks for one per available core, one
    /// asks for serial execution, and a larger value asks for a pool that size.
    /// Until step 4 of `docs/phase-4/PLAN.md` wires the parallel traversal every
    /// value runs serially, so the estimate charges one worker's workspaces.
    pub threads: usize,
}

/// One board the expansion reaches, and everything derived from it.
struct BoardState {
    /// Prefix cards plus the runout dealt so far, in deal order.
    cards: Vec<Card>,
    dead: CardSet,
    street: Street,
    /// Index into `Inner::tables`, present exactly when the board is complete.
    table: Option<usize>,
    /// Cards still to come, in card order. Empty on a complete board.
    possible: Vec<Card>,
}

/// One expanded public node's provenance and payoff.
struct Expanded {
    /// Node in the compact tree this one was expanded from.
    compact: NodeId,
    /// Index into `Inner::boards`.
    board: u32,
    payoff: Payoff,
}

pub(super) struct Inner {
    pub prefix: Vec<Card>,
    pub ranges: [Range; 2],
    pub tree: PostflopTree,
    pub layout: Arc<TraversalLayout>,
    pub parents: Vec<Option<(NodeId, usize)>>,
    pub memory: PostflopMemory,
    pub budget: Arc<Budget>,
    pub options: PostflopOptions,
    pub workers: usize,
    boards: Vec<BoardState>,
    tables: Vec<ShowdownTable>,
    nodes: Vec<Expanded>,
    runout_ranges: Vec<ops::Range<NodeId>>,
}

impl Inner {
    pub(super) fn payoff(&self, node: NodeId) -> Option<TerminalContext<'_>> {
        let expanded = self.nodes.get(node as usize)?;
        let board = &self.boards[expanded.board as usize];
        Some(TerminalContext {
            payoff: &expanded.payoff,
            dead: board.dead,
            table: board.table.map(|index| &self.tables[index]),
        })
    }

    pub(super) fn compact(&self, node: NodeId) -> Option<&tree::PostflopNode> {
        self.tree.node(self.nodes.get(node as usize)?.compact)
    }

    fn board_of(&self, node: NodeId) -> Option<&BoardState> {
        Some(&self.boards[self.nodes.get(node as usize)?.board as usize])
    }
}

/// Immutable postflop inputs and their expanded public tree.
///
/// The compact [`PostflopTree`] keeps one chance node per street transition;
/// this binding expands each of those into one child per dealt card, so every
/// distinct public history has its own node and the existing traversals run
/// over chance nodes unchanged. Clones retain the same binding and budget.
#[derive(Clone)]
pub struct PostflopGame {
    pub(super) inner: Arc<Inner>,
}

impl fmt::Debug for PostflopGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostflopGame")
            .field("board", &self.inner.prefix)
            .field("start_street", &self.inner.tree.config().start_street)
            .field("compact_nodes", &self.inner.tree.nodes().len())
            .field("expanded_nodes", &self.inner.layout.nodes.len())
            .field("memory", &self.inner.memory)
            .finish()
    }
}

impl PostflopGame {
    /// Own checked inputs, refuse an oversized estimate, then expand the tree.
    ///
    /// The board must hold exactly the cards the tree's start street knows:
    /// three on the flop, four on the turn, five on the river. The estimate is
    /// computed and compared against `options.memory_limit_bytes` before any
    /// row is allocated, so a game that cannot fit never allocates one.
    pub fn new(
        board: &[Card],
        ranges: [Range; 2],
        tree: PostflopTree,
        options: PostflopOptions,
    ) -> Result<Self, SolveError> {
        let start = tree.config().start_street;
        let expected = match start {
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River => 5,
        };
        if board.len() != expected {
            return Err(SolveError::InvalidGame(format!(
                "a {start} tree needs a {expected}-card board, not {}",
                board.len()
            )));
        }
        if options.precision != Precision::F64 {
            return Err(SolveError::Config(format!(
                "precision \"{}\" is not implemented for postflop games. Use \"f64\".",
                options.precision.as_str()
            )));
        }
        if options.memory_limit_bytes == 0 || options.memory_limit_bytes as u128 > MEMORY_CEILING {
            return Err(SolveError::Config(
                "postflop memory limit must be positive and at most 16 GiB".into(),
            ));
        }
        let workers = options.threads.max(1);
        let prefix_dead =
            CardSet::new(board).map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let memory = PostflopMemory::estimate(&tree, board.len(), workers)?;
        if memory.working_set_bound_bytes > options.memory_limit_bytes {
            return Err(SolveError::MemoryLimit {
                required: memory.working_set_bound_bytes,
                limit: options.memory_limit_bytes,
            });
        }

        let weights = [
            scaled(&ranges[0], prefix_dead)?,
            scaled(&ranges[1], prefix_dead)?,
        ];
        // One pair check on the board prefix, not one per runout: a runout only
        // removes combos, so a product that survives here survives everywhere.
        let combos: Vec<_> = collect(Combo::all())?;
        for (a, &wa) in combos.iter().zip(&weights[0]) {
            if wa == 0.0 {
                continue;
            }
            for (b, &wb) in combos.iter().zip(&weights[1]) {
                if wb > 0.0 && a.mask() & b.mask() == 0 && wa * wb == 0.0 {
                    return Err(SolveError::InvalidGame(
                        "positive compatible pair weight underflows".into(),
                    ));
                }
            }
        }
        let mut opposing_mass = [0.0; STATES];
        let opposing: &[f64; STATES] = weights[1]
            .as_slice()
            .try_into()
            .expect("fixed combo vector");
        evaluate_fold(prefix_dead, opposing, 1.0, &mut opposing_mass)
            .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let normalizer: f64 = weights[0]
            .iter()
            .zip(opposing_mass)
            .map(|(a, b)| a * b)
            .sum();
        if !normalizer.is_finite() || normalizer <= 0.0 {
            return Err(SolveError::EmptyGame);
        }

        let mut ctx = Expansion::new(&tree, memory.expanded_nodes)?;
        let root_board = ctx.board(collect(board.iter().copied())?, start)?;
        ctx.expand(&tree, tree.root(), root_board, 0)?;
        let Expansion {
            nodes,
            meta,
            boards,
            tables,
            parents,
            mask_pool,
            runout_ranges,
            ..
        } = ctx;

        let layout = Arc::new(TraversalLayout {
            root: 0,
            states: [STATES; 2],
            weights,
            nodes,
            mask_pool,
            normalizer,
            pot: tree.config().starting_pot as f64,
        });
        Ok(Self {
            inner: Arc::new(Inner {
                prefix: collect(board.iter().copied())?,
                ranges,
                tree,
                layout,
                parents,
                memory,
                budget: Budget::new(options.memory_limit_bytes, memory.shared_bytes),
                options,
                workers,
                boards,
                tables,
                nodes: meta,
                runout_ranges,
            }),
        })
    }

    /// Board cards known at the root, in the supplied order.
    #[must_use]
    pub fn board(&self) -> &[Card] {
        &self.inner.prefix
    }
    /// Original inclusion ranges, before board removal and per-range scaling.
    #[must_use]
    pub fn ranges(&self) -> &[Range; 2] {
        &self.inner.ranges
    }
    /// The compact betting tree, one chance node per street transition.
    #[must_use]
    pub fn tree(&self) -> &PostflopTree {
        &self.inner.tree
    }
    /// Construction settings, exactly as supplied.
    #[must_use]
    pub fn options(&self) -> PostflopOptions {
        self.inner.options
    }
    /// Traversal workers the estimate was charged for.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.inner.workers
    }
    /// Conservative retained and temporary allocation estimates.
    #[must_use]
    pub fn memory_usage(&self) -> PostflopMemory {
        self.inner.memory
    }
    /// Bytes currently reserved across every object sharing this binding.
    #[must_use]
    pub fn reserved_bytes(&self) -> usize {
        self.inner.budget.used()
    }
    /// Root normalizer after each board-filtered range is divided by its maximum.
    #[must_use]
    pub fn compatible_weight(&self) -> f64 {
        self.inner.layout.normalizer
    }
    /// Board-filtered and scaled inclusion weights in canonical combo order.
    #[must_use]
    pub fn initial_weights(&self, player: usize) -> Option<&[f64]> {
        self.inner.layout.weights.get(player).map(Vec::as_slice)
    }
    /// Root of the expanded tree, always zero.
    #[must_use]
    pub fn root(&self) -> NodeId {
        0
    }
    /// Public nodes after expansion, including chance and terminal nodes.
    #[must_use]
    pub fn num_nodes(&self) -> usize {
        self.inner.layout.nodes.len()
    }
    /// Reads an expanded node, or `None` for an out-of-range ID.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<PostflopNodeView<'_>> {
        ((id as usize) < self.inner.nodes.len()).then_some(PostflopNodeView {
            game: &self.inner,
            id,
        })
    }
    /// One contiguous node range per dealt card, in expansion order.
    ///
    /// Every runout's subtree owns a distinct range, which is what lets step 4
    /// hand each parallel task a disjoint slice of the accumulators.
    #[must_use]
    pub fn runout_ranges(&self) -> &[ops::Range<NodeId>] {
        &self.inner.runout_ranges
    }
}

/// One expanded public history: what the compact tree said, plus its board.
#[derive(Clone, Copy)]
pub struct PostflopNodeView<'a> {
    game: &'a Inner,
    id: NodeId,
}

impl fmt::Debug for PostflopNodeView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostflopNodeView")
            .field("id", &self.id)
            .field("kind", &self.kind())
            .field("street", &self.street())
            .field("board", &self.board())
            .finish()
    }
}

impl PostflopNodeView<'_> {
    fn compact(&self) -> &tree::PostflopNode {
        self.game.compact(self.id).expect("checked node index")
    }

    /// Decision actor, chance transition, or terminal outcome.
    #[must_use]
    pub fn kind(&self) -> PostflopNodeKind {
        self.compact().kind()
    }
    /// Street this history belongs to. A chance node reports the street whose
    /// betting just ended, not the street it deals.
    #[must_use]
    pub fn street(&self) -> Street {
        self.compact().street()
    }
    /// Chips committed since the root, excluding refunded excess wagers.
    #[must_use]
    pub fn contributions(&self) -> [Chips; 2] {
        self.compact().contributions()
    }
    /// Legal actions; empty at chance and terminal nodes.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        self.compact().actions()
    }
    /// Expanded children: one per action, one per dealt card, or none.
    #[must_use]
    pub fn children(&self) -> &[NodeId] {
        &self.game.layout.nodes[self.id as usize].children
    }
    /// Every board card known at this history, in deal order.
    #[must_use]
    pub fn board(&self) -> &[Card] {
        &self
            .game
            .board_of(self.id)
            .expect("checked node index")
            .cards
    }
    /// Board cards dealt after the root, in deal order.
    #[must_use]
    pub fn runout(&self) -> &[Card] {
        &self.board()[self.game.prefix.len()..]
    }
    /// Cards a chance node can deal, in card order; empty elsewhere.
    #[must_use]
    pub fn possible_cards(&self) -> &[Card] {
        if matches!(self.kind(), PostflopNodeKind::Chance { .. }) {
            &self
                .game
                .board_of(self.id)
                .expect("checked node index")
                .possible
        } else {
            &[]
        }
    }
    /// Probability of one dealt card at a chance node, before private masks.
    #[must_use]
    pub fn chance_probability(&self) -> Option<Real> {
        self.game.layout.nodes[self.id as usize]
            .probabilities
            .first()
            .copied()
    }
    /// The compact-tree node this history was expanded from.
    #[must_use]
    pub fn compact_id(&self) -> NodeId {
        self.game.nodes[self.id as usize].compact
    }
}

/// Mutable state carried through the depth-first expansion.
struct Expansion {
    nodes: Vec<Node>,
    meta: Vec<Expanded>,
    boards: Vec<BoardState>,
    /// Child board index per (board, card), or `u32::MAX` when not yet dealt.
    board_children: Vec<Vec<u32>>,
    tables: Vec<ShowdownTable>,
    /// Complete boards already tabulated, keyed on the card set.
    tabulated: HashMap<u64, usize>,
    parents: Vec<Option<(NodeId, usize)>>,
    mask_pool: Vec<[Vec<Real>; 2]>,
    /// Mask-pool index per dealt card, keyed on the card ID.
    pooled: HashMap<u8, usize>,
    runout_ranges: Vec<ops::Range<NodeId>>,
    half_pot: f64,
    limit: usize,
}

impl Expansion {
    fn new(tree: &PostflopTree, limit: usize) -> Result<Self, SolveError> {
        Ok(Self {
            nodes: reserved(limit)?,
            meta: reserved(limit)?,
            boards: Vec::new(),
            board_children: Vec::new(),
            tables: Vec::new(),
            tabulated: HashMap::new(),
            parents: filled(limit, None)?,
            mask_pool: Vec::new(),
            pooled: HashMap::new(),
            runout_ranges: Vec::new(),
            half_pot: tree.config().starting_pot as f64 / 2.0,
            limit,
        })
    }

    /// Interns one board, building its showdown table when it is complete.
    fn board(&mut self, cards: Vec<Card>, street: Street) -> Result<usize, SolveError> {
        let dead = CardSet::new(&cards).map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let table = if cards.len() == 5 {
            let key = dead.bits();
            let index = match self.tabulated.get(&key) {
                Some(index) => *index,
                None => {
                    let board: [Card; 5] = cards.as_slice().try_into().expect("checked length");
                    let built = ShowdownTable::new(board)
                        .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
                    if built.storage_bytes() > 65_536 {
                        return Err(SolveError::Allocation(
                            "showdown table exceeds its preflight bound".into(),
                        ));
                    }
                    self.tables.push(built);
                    let index = self.tables.len() - 1;
                    self.tabulated.insert(key, index);
                    index
                }
            };
            Some(index)
        } else {
            None
        };
        let possible = if table.is_some() {
            Vec::new()
        } else {
            collect(Card::all())?
                .into_iter()
                .filter(|card| !dead.contains(*card))
                .collect()
        };
        self.boards.push(BoardState {
            cards,
            dead,
            street,
            table,
            possible,
        });
        self.board_children.push(vec![u32::MAX; 52]);
        Ok(self.boards.len() - 1)
    }

    /// The board reached by dealing `card` from `board`, interned once.
    fn deal(&mut self, board: usize, card: Card, street: Street) -> Result<usize, SolveError> {
        let existing = self.board_children[board][usize::from(card.id())];
        if existing != u32::MAX {
            return Ok(existing as usize);
        }
        let mut cards = self.boards[board].cards.clone();
        cards.push(card);
        let child = self.board(cards, street)?;
        self.board_children[board][usize::from(card.id())] = child as u32;
        Ok(child)
    }

    /// Both players' zero-or-one masks for one dealt card, interned once.
    fn masks(&mut self, card: Card) -> Result<usize, SolveError> {
        if let Some(index) = self.pooled.get(&card.id()) {
            return Ok(*index);
        }
        let entries: Vec<Real> = collect(Combo::all().map(|combo| {
            if combo.mask() & card.mask() == 0 {
                1.0
            } else {
                0.0
            }
        }))?;
        self.mask_pool.push([entries.clone(), entries]);
        let index = self.mask_pool.len() - 1;
        self.pooled.insert(card.id(), index);
        Ok(index)
    }

    fn expand(
        &mut self,
        tree: &PostflopTree,
        compact: NodeId,
        board: usize,
        depth: usize,
    ) -> Result<NodeId, SolveError> {
        if depth > MAX_EXPANSION_DEPTH {
            return Err(SolveError::InvalidGame(format!(
                "expansion exceeded the depth limit of {MAX_EXPANSION_DEPTH} edges"
            )));
        }
        if self.nodes.len() >= self.limit {
            return Err(SolveError::Allocation(format!(
                "expansion exceeded its own estimate of {} public nodes",
                self.limit
            )));
        }
        let node = tree
            .node(compact)
            .ok_or_else(|| SolveError::InvalidGame("compact tree child is out of range".into()))?;
        if node.street() != self.boards[board].street {
            return Err(SolveError::InvalidGame(format!(
                "node street {} does not match its board's {}",
                node.street(),
                self.boards[board].street
            )));
        }
        let id = self.nodes.len() as NodeId;
        let (kind, payoff) =
            match node.kind() {
                PostflopNodeKind::Decision { player } => (
                    NodeKind::Player {
                        player,
                        num_actions: node.actions().len().try_into().map_err(|_| {
                            SolveError::InvalidGame("postflop action count exceeds 255".into())
                        })?,
                    },
                    Payoff::Decision,
                ),
                PostflopNodeKind::Chance { .. } => (
                    NodeKind::Chance {
                        num_outcomes: self.boards[board].possible.len().try_into().map_err(
                            |_| SolveError::InvalidGame("outcome count overflow".into()),
                        )?,
                    },
                    Payoff::Chance,
                ),
                PostflopNodeKind::Terminal(terminal) => {
                    let contributions = node.contributions().map(|chips| chips as f64);
                    if contributions[0] != contributions[1] {
                        return Err(SolveError::InvalidGame(
                            "postflop terminal must refund uncalled contributions".into(),
                        ));
                    }
                    let amount = self.half_pot + contributions[0];
                    let payoff = match terminal {
                        Terminal::Fold { winner } => {
                            Payoff::Fold(if winner == 0 { amount } else { -amount })
                        }
                        Terminal::Showdown => {
                            if self.boards[board].table.is_none() {
                                return Err(SolveError::InvalidGame(
                                    "showdown reached before the board was complete".into(),
                                ));
                            }
                            let utilities = OutcomeUtilities::new(amount, 0.0, -amount)
                                .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
                            Payoff::Showdown([utilities, utilities])
                        }
                    };
                    (NodeKind::Terminal, payoff)
                }
            };
        self.nodes.push(Node {
            kind,
            children: Vec::new(),
            probabilities: Vec::new(),
            masks: Vec::new(),
        });
        self.meta.push(Expanded {
            compact,
            board: board.try_into().map_err(|_| {
                SolveError::InvalidGame("board index exceeds its 32-bit range".into())
            })?,
            payoff,
        });

        match node.kind() {
            PostflopNodeKind::Decision { .. } => {
                let mut children = reserved(node.children().len())?;
                for (action, child) in node.children().iter().enumerate() {
                    let expanded = self.expand(tree, *child, board, depth + 1)?;
                    self.parents[expanded as usize] = Some((id, action));
                    children.push(expanded);
                }
                self.nodes[id as usize].children = children;
            }
            PostflopNodeKind::Chance { next } => {
                let cards = self.boards[board].possible.clone();
                let unseen = cards.len();
                if unseen <= PRIVATE_CARDS {
                    return Err(SolveError::InvalidGame(format!(
                        "a deal with {unseen} unseen cards cannot exclude four private cards"
                    )));
                }
                // Every compatible pair blocks exactly four of the unseen cards,
                // so the surviving outcomes carry probability one between them.
                let probability = 1.0 / (unseen - PRIVATE_CARDS) as f64;
                let compact_child = *node.children().first().ok_or_else(|| {
                    SolveError::InvalidGame("chance node deals no child block".into())
                })?;
                let mut children = reserved(unseen)?;
                let mut probabilities = reserved(unseen)?;
                let mut masks = reserved(unseen)?;
                for (outcome, card) in cards.into_iter().enumerate() {
                    let child_board = self.deal(board, card, next)?;
                    let start = self.nodes.len() as NodeId;
                    let expanded = self.expand(tree, compact_child, child_board, depth + 1)?;
                    self.parents[expanded as usize] = Some((id, outcome));
                    self.runout_ranges.push(start..self.nodes.len() as NodeId);
                    children.push(expanded);
                    probabilities.push(probability);
                    masks.push(self.masks(card)?);
                }
                let expanded = &mut self.nodes[id as usize];
                expanded.children = children;
                expanded.probabilities = probabilities;
                expanded.masks = masks;
            }
            PostflopNodeKind::Terminal(_) => {}
        }
        Ok(id)
    }
}

/// Board-filtered inclusion weights, divided by the range's own maximum.
fn scaled(range: &Range, dead: CardSet) -> Result<Vec<f64>, SolveError> {
    let mut result = collect(range.weights().iter().copied())?;
    for combo in Combo::all() {
        if combo.mask() & dead.bits() != 0 {
            result[usize::from(combo.id())] = 0.0;
        }
    }
    let maximum = result.iter().copied().fold(0.0_f64, f64::max);
    if maximum == 0.0 {
        return Err(SolveError::EmptyGame);
    }
    for value in &mut result {
        *value /= maximum;
    }
    Ok(result)
}
