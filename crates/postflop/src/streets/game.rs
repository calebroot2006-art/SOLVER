use super::memory::PostflopMemory;
use super::terminal::{
    Payoff, PostflopColumns, PostflopTerminal, TerminalContext, TerminalWorkspace,
};
use super::{
    PRIVATE_CARDS, STATES, VALIDATION_COLUMN_LIMIT, VALIDATION_NODE_LIMIT, VALIDATION_PAIR_LIMIT,
    resolve_workers,
};
use crate::memory::Budget;
use crate::{
    NodeId, NodeKind, Precision, Real, SolveError, SolverConfig,
    allocation::{collect, filled, reserved},
    config::{MEMORY_LIMIT_CEILING_BYTES, MEMORY_LIMIT_CEILING_MIB},
    game::{NodeBuild, PairScope, TerminalColumns, TraversalLayout, validate_traversal},
    terminal::{OutcomeUtilities, ShowdownTable, evaluate_fold},
};
use cards::{Card, CardSet, Combo, Range};
use std::{collections::HashMap, fmt, ops, sync::Arc};
use tree::{Action, Chips, PostflopNodeKind, PostflopTree, Street, Terminal};

/// How a postflop game is built and run.
///
/// Every field is configuration, not a magic constant. [`Self::from_config`]
/// reads all three from a parsed `config/solver.toml`: `memory_limit_mib`,
/// `precision` and `solve.threads`.
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
    /// value runs serially on the first workspace, but the estimate charges and
    /// the solver allocates one workspace per resolved worker either way.
    pub threads: usize,
}

impl PostflopOptions {
    /// Everything a game needs from the solver's configuration file.
    ///
    /// The byte limit comes from `memory_limit_mib`, which defaults to
    /// decision 4's 12 GiB and is refused above the 16 GiB ceiling.
    pub fn from_config(config: &SolverConfig) -> Result<Self, SolveError> {
        Ok(Self {
            memory_limit_bytes: config.memory_limit_bytes()?,
            precision: config.precision,
            threads: config.solve.threads,
        })
    }
}

/// What the construction-time path validation covered.
///
/// The walk checks the contracts a traversal cannot see locally. The structural
/// ones are linear in the tree and run on every game, however large: every
/// expanded node reachable exactly once, no cycles or shared children, declared
/// child counts against action and outcome counts, chance probabilities inside
/// [0,1], mask shapes, and terminals without children. Two are quadratic in the
/// live combos and are size gated: one unit of chance mass per compatible pair
/// still legal after ancestor masks, and zero-sum terminal utilities.
///
/// The two gated checks have separate budgets, so they stop independently: the
/// mass check needs the live pairs and the expanded nodes inside their limits,
/// and the zero-sum check needs that and its own column limit on top. A tree
/// with 500 live combos per player whose terminals times live states exceed the
/// column limit therefore reports its pairs and no zero-sum terminals. Read
/// each field before trusting its check: a zero means that check did not run,
/// never that it passed. The other three counts are always the whole tree, so a
/// game above either gate still reports what was covered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PostflopValidation {
    /// Expanded nodes the structural walk reached, which is every node.
    pub nodes: usize,
    /// Live private-state pairs the mass check ran over, or zero when the tree
    /// was above the pair or node limit. The zero-sum check, when it ran, ran
    /// over these same pairs, but it has a further budget of its own: a nonzero
    /// count here does not mean it ran, so read
    /// [`Self::zero_sum_terminals`] for that.
    pub pairs: usize,
    /// Chance nodes whose outcome count, probabilities and mask shapes were
    /// checked. Structural, so this is every chance node in the tree.
    pub chance_nodes: usize,
    /// Terminals whose node shape was checked. Structural, so this is every
    /// terminal in the tree.
    pub terminals: usize,
    /// Terminals whose utilities were checked pairwise for zero sum, or zero
    /// when the tree was above a budget for that check. It has its own column
    /// limit on top of the pair and node limits, so this is zero whenever
    /// [`Self::pairs`] is, and can be zero when [`Self::pairs`] is not.
    pub zero_sum_terminals: usize,
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

/// A node with no parent: only the root has one.
const NO_PARENT: NodeId = NodeId::MAX;

/// Every expanded node's provenance, as struct-of-arrays.
///
/// Each field used to be one member of a per-node record holding an inline
/// `Payoff`, which is 56 bytes of enum on every node in the tree whether or not
/// it is a terminal. The gate flop tree has 1.79 million of them. Now the
/// payoff is an index into a handful of interned records, and the parent link
/// is two `u32`s rather than an `Option<(NodeId, usize)>`.
#[derive(Default)]
struct Topology {
    /// Node in the compact tree each one was expanded from.
    compact: Vec<NodeId>,
    /// Index into `Inner::boards`.
    board: Vec<u32>,
    /// One past the last node expanded below this one. Expansion is depth
    /// first, so `id..end` is exactly this node and its descendants.
    end: Vec<NodeId>,
    /// Index into `Inner::payoffs`.
    payoff: Vec<u32>,
    /// Parent node, `NO_PARENT` at the root, and which of its edges leads here.
    parent: Vec<NodeId>,
    parent_edge: Vec<u32>,
}

pub(super) struct Inner {
    pub prefix: Vec<Card>,
    pub ranges: [Range; 2],
    pub tree: PostflopTree,
    pub layout: Arc<TraversalLayout>,
    pub memory: PostflopMemory,
    pub budget: Arc<Budget>,
    pub workers: usize,
    /// Board-filtered, scaled weights over all 1326 combo IDs, for the public
    /// API and the reports. The layout's own weights are the compacted ones.
    pub weights: [Vec<f64>; 2],
    /// Compact index to combo ID, per player: `layout.states[p]` entries, in
    /// increasing combo order. This is the projection everything the walk holds
    /// is indexed by.
    pub live: [Vec<u16>; 2],
    /// Combo ID to compact index, or [`Self::BLOCKED`] for a combo this game
    /// never deals: no weight in the range, or blocked by the board prefix.
    pub slot: [Vec<u16>; 2],
    validation: PostflopValidation,
    boards: Vec<BoardState>,
    tables: Vec<ShowdownTable>,
    topology: Topology,
    payoffs: Vec<Payoff>,
}

impl Inner {
    /// Marks a combo ID with no compact slot in this game.
    pub(super) const BLOCKED: u16 = u16::MAX;

    pub(super) fn num_nodes(&self) -> usize {
        self.topology.compact.len()
    }

    pub(super) fn payoff(&self, node: NodeId) -> Option<TerminalContext<'_>> {
        let index = *self.topology.payoff.get(node as usize)?;
        let board = &self.boards[self.topology.board[node as usize] as usize];
        Some(TerminalContext {
            payoff: &self.payoffs[index as usize],
            dead: board.dead,
            table: board.table.map(|index| &self.tables[index]),
        })
    }

    pub(super) fn compact(&self, node: NodeId) -> Option<&tree::PostflopNode> {
        self.tree.node(*self.topology.compact.get(node as usize)?)
    }

    /// Parent node and the edge index that leads here, or `None` at the root.
    pub(super) fn parent(&self, node: NodeId) -> Option<(NodeId, usize)> {
        let parent = *self.topology.parent.get(node as usize)?;
        (parent != NO_PARENT).then(|| (parent, self.topology.parent_edge[node as usize] as usize))
    }

    fn board_of(&self, node: NodeId) -> Option<&BoardState> {
        Some(&self.boards[*self.topology.board.get(node as usize)? as usize])
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
            .field("expanded_nodes", &self.inner.num_nodes())
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
        if options.memory_limit_bytes == 0
            || options.memory_limit_bytes as u128 > MEMORY_LIMIT_CEILING_BYTES
        {
            return Err(SolveError::Config(format!(
                "postflop memory limit must be positive and at most {MEMORY_LIMIT_CEILING_MIB} MiB"
            )));
        }
        let workers = resolve_workers(options.threads);
        let prefix_dead =
            CardSet::new(board).map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let weights = [
            scaled(&ranges[0], prefix_dead)?,
            scaled(&ranges[1], prefix_dead)?,
        ];

        // In-range compaction. A combo with no weight, or one the board prefix
        // blocks, contributes nothing to any reach, value or accumulator: its
        // live mask is zero at every terminal, so its regrets and strategy sums
        // stay at zero for the whole solve and every value read off it is zero.
        // The walk therefore carries only the combos that are actually dealt,
        // and the terminal boundary scatters back to all 1326 for the showdown
        // tables and the fold evaluator, which are written against combo IDs.
        // A later runout can still block a live combo; that stays a mask.
        //
        // This runs before the estimate, and it is two passes over 1326 range
        // weights: the estimate charges the buffers the compacted walk will
        // hold, so a game is still refused before it allocates any of them.
        let mut live: [Vec<u16>; 2] = [Vec::new(), Vec::new()];
        let mut slot: [Vec<u16>; 2] = [
            filled(STATES, Inner::BLOCKED)?,
            filled(STATES, Inner::BLOCKED)?,
        ];
        let mut compacted: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        for player in 0..2 {
            for (id, weight) in weights[player].iter().enumerate() {
                if *weight > 0.0 {
                    let index = u16::try_from(live[player].len())
                        .map_err(|_| SolveError::InvalidGame("live combo overflow".into()))?;
                    slot[player][id] = index;
                    live[player].push(id as u16);
                    compacted[player].push(*weight);
                }
            }
            if live[player].is_empty() {
                return Err(SolveError::EmptyGame);
            }
        }
        let states = [live[0].len(), live[1].len()];

        let memory = PostflopMemory::estimate(&tree, board.len(), workers, states)?;
        if memory.working_set_bound_bytes > options.memory_limit_bytes {
            return Err(SolveError::MemoryLimit {
                required: memory.working_set_bound_bytes,
                limit: options.memory_limit_bytes,
            });
        }

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

        let mut ctx = Expansion::new(&tree, memory.expanded_nodes, &live)?;
        let root_board = ctx.board(collect(board.iter().copied())?, start)?;
        ctx.expand(&tree, tree.root(), root_board)?;
        let Expansion {
            nodes,
            topology,
            boards,
            tables,
            payoffs,
            mask_pool,
            ..
        } = ctx;

        let layout = Arc::new(TraversalLayout::new(
            0,
            states,
            compacted,
            nodes,
            mask_pool,
            normalizer,
            tree.config().starting_pot as f64,
        )?);
        let mut inner = Inner {
            prefix: collect(board.iter().copied())?,
            ranges,
            tree,
            layout,
            memory,
            budget: Budget::new(options.memory_limit_bytes, memory.shared_bytes),
            workers,
            weights,
            live,
            slot,
            validation: PostflopValidation::default(),
            boards,
            tables,
            topology,
            payoffs,
        };
        inner.validation = validate_expansion(&inner)?;
        Ok(Self {
            inner: Arc::new(inner),
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
    /// What the construction-time path validation covered. The structural walk
    /// always ran; [`PostflopValidation`] says whether the two quadratic checks
    /// were inside their budgets.
    #[must_use]
    pub fn validation(&self) -> PostflopValidation {
        self.inner.validation
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
        self.inner.weights.get(player).map(Vec::as_slice)
    }
    /// The combos this game carries for `player`, in combo-ID order.
    ///
    /// In-range compaction drops every combo with no weight and every combo the
    /// board prefix blocks, so a policy row has one entry per combo listed here
    /// rather than one per 1326. Row `state` of a node's row belongs to
    /// `live_combos(player)[state]`.
    #[must_use]
    pub fn live_combos(&self, player: usize) -> Option<&[u16]> {
        self.inner.live.get(player).map(Vec::as_slice)
    }
    /// Where `combo` sits in `player`'s rows, or `None` when this game never
    /// deals it.
    #[must_use]
    pub fn state_of(&self, player: usize, combo: Combo) -> Option<usize> {
        let slot = *self.inner.slot.get(player)?.get(usize::from(combo.id()))?;
        (slot != Inner::BLOCKED).then(|| usize::from(slot))
    }
    /// Root of the expanded tree, always zero.
    #[must_use]
    pub fn root(&self) -> NodeId {
        0
    }
    /// Public nodes after expansion, including chance and terminal nodes.
    #[must_use]
    pub fn num_nodes(&self) -> usize {
        self.inner.num_nodes()
    }
    /// Reads an expanded node, or `None` for an out-of-range ID.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<PostflopNodeView<'_>> {
        ((id as usize) < self.inner.num_nodes()).then_some(PostflopNodeView {
            game: &self.inner,
            id,
        })
    }
    /// Half-open node range holding `node` and everything expanded below it.
    ///
    /// Expansion is depth first, so a node's descendants are contiguous: the
    /// range always starts at `node` itself and is never empty.
    #[must_use]
    pub fn subtree(&self, node: NodeId) -> Option<ops::Range<NodeId>> {
        let end = *self.inner.topology.end.get(node as usize)?;
        Some(node..end)
    }
    /// The nodes one outcome of a chance node owns, in outcome order.
    ///
    /// This is the disjoint-runout contract a parallel traversal splits
    /// accumulators along, and it holds at every chance level: the ranges of one
    /// chance node's outcomes are non-empty, pairwise disjoint, increasing, and
    /// together they partition that chance node's own subtree less its root. A
    /// flop tree's turn deal and each of its river deals therefore each
    /// partition their own parent's range, rather than sharing one flat list.
    /// `None` for an unknown node, an outcome the node does not have, or a node
    /// that is not a chance node.
    #[must_use]
    pub fn outcome_range(&self, chance: NodeId, outcome: usize) -> Option<ops::Range<NodeId>> {
        let view = self.node(chance)?;
        if !matches!(view.kind(), PostflopNodeKind::Chance { .. }) {
            return None;
        }
        self.subtree(*view.children().get(outcome)?)
    }
}

/// Walks the expanded tree's whole-tree contracts.
///
/// The structural contracts cost one pass over the nodes, so they run on every
/// tree: a game that skipped them could hand the traversals a shared child or
/// an unreachable node and never find out. The chance-mass check is one pass
/// over every live pair at every chance node and the zero-sum check is one
/// terminal evaluation per live state per terminal, so those two are gated by
/// their own budgets and report zero when they do not run. The small fixtures
/// in `tests/streets.rs` sit inside every budget, which is where the two gated
/// checks earn their keep.
fn validate_expansion(inner: &Inner) -> Result<PostflopValidation, SolveError> {
    // Compaction has already dropped every zero-weight combo, so the live count
    // is the layout's own state count.
    let live = inner.layout.states;
    let pairs = live[0].saturating_mul(live[1]);
    // A range with no live combo cannot reach here, but the walk refuses a
    // column source it would never read, so the zero case picks the no-pair
    // scope rather than asking for a check with nothing to check.
    let pairwise =
        pairs > 0 && pairs <= VALIDATION_PAIR_LIMIT && inner.num_nodes() <= VALIDATION_NODE_LIMIT;
    let terminals = inner
        .layout
        .kinds
        .iter()
        .filter(|kind| **kind == NodeKind::Terminal)
        .count();
    let zero_sum =
        pairwise && terminals.saturating_mul(live[0] + live[1]) <= VALIDATION_COLUMN_LIMIT;

    // Two combos coexist in a deal exactly when they share no card. The walk
    // asks about compact indices, so each side is looked up through its own
    // live table first.
    let masks: [Vec<u64>; 2] = std::array::from_fn(|player| {
        inner.live[player]
            .iter()
            .map(|id| {
                Combo::from_id(*id)
                    .expect("live combos come from Combo::all")
                    .mask()
            })
            .collect()
    });
    let compatible = |h0: usize, h1: usize| masks[0][h0] & masks[1][h1] == 0;

    let mut workspace = TerminalWorkspace::default();
    let mut columns = PostflopColumns {
        terminal: PostflopTerminal {
            game: inner,
            workspace: &mut workspace,
        },
        opponent: filled(live[1].max(live[0]), 0.0)?,
    };
    let source: Option<&mut dyn TerminalColumns> = if zero_sum { Some(&mut columns) } else { None };
    // Every state the layout carries has positive weight after compaction, so
    // the two scopes name the same pairs; this one says so at the call site.
    let scope = if pairwise {
        PairScope::PositiveWeight
    } else {
        PairScope::NoPairs
    };
    let checks = validate_traversal(&inner.layout, &compatible, scope, source)?;
    Ok(PostflopValidation {
        nodes: checks.nodes,
        // `PairScope::NoPairs` reports no pairs, so this is what was walked.
        pairs: checks.pairs,
        chance_nodes: checks.chance_nodes,
        terminals: checks.terminals,
        zero_sum_terminals: if zero_sum { checks.terminals } else { 0 },
    })
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
        self.game.layout.children(self.id)
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
        matches!(self.kind(), PostflopNodeKind::Chance { .. })
            .then(|| self.game.layout.probabilities(self.id).first().copied())
            .flatten()
    }
    /// The compact-tree node this history was expanded from.
    #[must_use]
    pub fn compact_id(&self) -> NodeId {
        self.game.topology.compact[self.id as usize]
    }
}

/// Mutable state carried through the depth-first expansion.
struct Expansion<'a> {
    nodes: Vec<NodeBuild>,
    topology: Topology,
    boards: Vec<BoardState>,
    /// Child board index per (board, card), or `u32::MAX` when not yet dealt.
    board_children: Vec<Vec<u32>>,
    tables: Vec<ShowdownTable>,
    /// Complete boards already tabulated, keyed on the card set.
    tabulated: HashMap<u64, usize>,
    /// Distinct payoffs, one entry per (kind, amount) the tree pays.
    payoffs: Vec<Payoff>,
    /// Payoff index keyed on the kind tag and the amount's bits.
    interned: HashMap<(u8, u64), u32>,
    mask_pool: Vec<[Vec<Real>; 2]>,
    /// Mask-pool index per dealt card, keyed on the card ID.
    pooled: HashMap<u8, u32>,
    /// Compact index to combo ID per player, so a dealt card's mask is built
    /// over the combos this game actually carries.
    live: &'a [Vec<u16>; 2],
    half_pot: f64,
    limit: usize,
}

/// Payoff kinds, so two payoffs of the same amount but different kinds intern
/// separately.
const FOLD: u8 = 0;
const SHOWDOWN: u8 = 1;
const DECISION: u8 = 2;
const CHANCE: u8 = 3;

impl<'a> Expansion<'a> {
    fn new(tree: &PostflopTree, limit: usize, live: &'a [Vec<u16>; 2]) -> Result<Self, SolveError> {
        Ok(Self {
            nodes: reserved(limit)?,
            topology: Topology {
                compact: reserved(limit)?,
                board: reserved(limit)?,
                end: reserved(limit)?,
                payoff: reserved(limit)?,
                parent: reserved(limit)?,
                parent_edge: reserved(limit)?,
            },
            boards: Vec::new(),
            board_children: Vec::new(),
            tables: Vec::new(),
            tabulated: HashMap::new(),
            payoffs: Vec::new(),
            interned: HashMap::new(),
            mask_pool: Vec::new(),
            pooled: HashMap::new(),
            live,
            half_pot: tree.config().starting_pot as f64 / 2.0,
            limit,
        })
    }

    /// Interns one payoff on its kind and its amount, returning its index.
    fn intern(&mut self, tag: u8, amount: f64, payoff: Payoff) -> Result<u32, SolveError> {
        let key = (tag, amount.to_bits());
        if let Some(index) = self.interned.get(&key) {
            return Ok(*index);
        }
        let index = u32::try_from(self.payoffs.len())
            .map_err(|_| SolveError::InvalidGame("payoff table overflow".into()))?;
        self.payoffs.push(payoff);
        self.interned.insert(key, index);
        Ok(index)
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
    ///
    /// The entries run over each player's live combos, so the two halves are
    /// different lengths whenever the ranges are.
    fn masks(&mut self, card: Card) -> Result<u32, SolveError> {
        if let Some(index) = self.pooled.get(&card.id()) {
            return Ok(*index);
        }
        let pair: [Vec<Real>; 2] = [self.card_mask(card, 0)?, self.card_mask(card, 1)?];
        let index = u32::try_from(self.mask_pool.len())
            .map_err(|_| SolveError::InvalidGame("chance mask pool overflow".into()))?;
        self.mask_pool.push(pair);
        self.pooled.insert(card.id(), index);
        Ok(index)
    }

    fn card_mask(&self, card: Card, player: usize) -> Result<Vec<Real>, SolveError> {
        collect(self.live[player].iter().map(|id| {
            let combo = Combo::from_id(*id).expect("live combos come from Combo::all");
            if combo.mask() & card.mask() == 0 {
                1.0
            } else {
                0.0
            }
        }))
    }

    /// Expands one compact node onto one board, depth first.
    ///
    /// The recursion follows one compact edge per level and `PostflopTree`
    /// refuses a tree deeper than its own 128-edge `MAX_DEPTH`, so the depth
    /// here needs no second limit of its own; the node budget below bounds the
    /// total work either way.
    fn expand(
        &mut self,
        tree: &PostflopTree,
        compact: NodeId,
        board: usize,
    ) -> Result<NodeId, SolveError> {
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
                    self.intern(DECISION, 0.0, Payoff::Decision)?,
                ),
                PostflopNodeKind::Chance { .. } => (
                    NodeKind::Chance {
                        num_outcomes: self.boards[board].possible.len().try_into().map_err(
                            |_| SolveError::InvalidGame("outcome count overflow".into()),
                        )?,
                    },
                    self.intern(CHANCE, 0.0, Payoff::Chance)?,
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
                            let value = if winner == 0 { amount } else { -amount };
                            self.intern(FOLD, value, Payoff::Fold(value))?
                        }
                        Terminal::Showdown => {
                            if self.boards[board].table.is_none() {
                                return Err(SolveError::InvalidGame(
                                    "showdown reached before the board was complete".into(),
                                ));
                            }
                            // One record, not a pair: a showdown pays the winner
                            // `amount` and the loser `-amount` whichever player is
                            // asking, so both players read the same utilities.
                            let utilities = OutcomeUtilities::new(amount, 0.0, -amount)
                                .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
                            self.intern(SHOWDOWN, amount, Payoff::Showdown(utilities))?
                        }
                    };
                    (NodeKind::Terminal, payoff)
                }
            };
        self.nodes.push(NodeBuild {
            kind,
            children: Vec::new(),
            probabilities: Vec::new(),
            masks: Vec::new(),
        });
        self.topology.compact.push(compact);
        self.topology.board.push(
            board.try_into().map_err(|_| {
                SolveError::InvalidGame("board index exceeds its 32-bit range".into())
            })?,
        );
        // Filled in once the subtree below this node is complete.
        self.topology.end.push(id + 1);
        self.topology.payoff.push(payoff);
        self.topology.parent.push(NO_PARENT);
        self.topology.parent_edge.push(0);

        match node.kind() {
            PostflopNodeKind::Decision { .. } => {
                let mut children = reserved(node.children().len())?;
                for (action, child) in node.children().iter().enumerate() {
                    let expanded = self.expand(tree, *child, board)?;
                    self.topology.parent[expanded as usize] = id;
                    self.topology.parent_edge[expanded as usize] = action as u32;
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
                    let expanded = self.expand(tree, compact_child, child_board)?;
                    self.topology.parent[expanded as usize] = id;
                    self.topology.parent_edge[expanded as usize] = outcome as u32;
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
        // Depth first: every node pushed since this one belongs below it, so
        // `id..end` is this subtree and nothing else.
        self.topology.end[id as usize] = self.nodes.len() as NodeId;
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

#[cfg(test)]
mod tests {
    use super::{PostflopOptions, resolve_workers};
    use crate::{Precision, SolverConfig};

    const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/solver.toml");

    #[test]
    fn options_come_from_the_configuration_file_rather_than_a_compiled_default() {
        let config = SolverConfig::load(FILE).unwrap_or_else(|error| panic!("{FILE}: {error}"));
        let options = PostflopOptions::from_config(&config).unwrap();
        assert_eq!(options.memory_limit_bytes, 12 * 1024 * 1024 * 1024);
        assert_eq!(options.precision, Precision::F64);
        assert_eq!(options.threads, config.solve.threads);
        // The shipped file asks for one worker per core; the game resolves that
        // once and the estimate charges for the number it resolved to.
        assert_eq!(options.threads, 0);
        assert!(resolve_workers(options.threads) >= 1);
    }
}
