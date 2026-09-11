//! Working-set accounting for a street-aware postflop game.
//!
//! The estimate is computed from the compact [`PostflopTree`] before a single
//! solver row is allocated, so a game that cannot fit refuses to start instead
//! of failing part way through construction. It reads the built tree's own
//! per-street counters and multiplies each street by the number of board states
//! that street has; it never multiplies one street block's node count by
//! another's, because a deep raise target can clamp to the stack and merge into
//! the all-in, which makes the blocks different sizes.
//!
//! It charges the temporary buffers construction itself holds, not only the
//! ones the solve keeps, so a game that refuses has been measured against
//! everything it would have allocated rather than only against what survives
//! construction.
//!
//! [`PostflopMemory::rows_under`] reports the same bound one buffer at a time:
//! what each row stores, how many bytes it takes, how long it lives, and
//! whether another row is the same allocation seen under another lifetime. The
//! rows the bound counts sum to [`PostflopMemory::working_set_bound_bytes`]
//! exactly, and [`MemoryReservation`] names, for every `Budget` reservation this
//! module's callers make, which rows that reservation draws from.
//!
//! [`PostflopMemory::plan`] is the layout the code implements after step 6 of
//! `docs/phase-4/PLAN.md`: two stored arrays, the current policy derived from
//! the regrets at visit time, no snapshot retained by the solve itself, one
//! held by a caller browsing a result, and one entry per live combo rather than
//! per 1326. Any other [`StoragePlan`] is arithmetic over the same entry
//! counts, not a measurement: `f32` (step 7) and `i16` (step 10) are not
//! implemented, and [`StoragePlan::before_compaction`] is what the crate stored
//! before step 6, kept so the table can print what moved.
//!
//! These are allocations this crate makes under its own API. They are not
//! process resident set size.

use super::terminal::{Payoff, TerminalWorkspace};
use super::{DECK, PRIVATE_CARDS, STATES, VALIDATION_PAIR_LIMIT};
use crate::{
    Cfr, NodeKind, Precision, SolveError, Strategy,
    game::{NodeBuild, TraversalLayout},
    memory::Lease,
};
use std::{mem::size_of, sync::Mutex};
use tree::{PostflopNodeKind, PostflopTree, Street};

/// Upper bound on the chance-mask pool: one entry per card that can be dealt.
const MASK_POOL_ENTRIES: usize = DECK;
/// Bound on one checked `ShowdownTable`, the same bound the river game asserts.
const SHOWDOWN_TABLE_BYTES: usize = 65_536;
/// Bytes charged per interned key in a construction-time `HashMap`. The maps
/// hold a `u64` or a `u8` against an index. A doubling can briefly hold
/// both the old and new tables: 64 bytes per entry plus the fixed tail below
/// covers that overlap, bucket rounding and control bytes. Payoff keys are
/// wider and are charged separately at 96 bytes per compact node.
const MAP_ENTRY_BYTES: usize = 64;

/// Row names. One `const` per row so a reservation, a test and the printed
/// table cannot drift apart on spelling.
pub mod rows {
    /// The compact betting tree the game was built from.
    pub const TREE: &str = "compact betting tree";
    /// Expanded topology: node records, edges, per-node links, runout ranges.
    pub const TOPOLOGY: &str = "expanded topology and offsets";
    /// One probability and one mask-pool index per chance outcome.
    pub const CHANCE: &str = "chance probabilities and mask indices";
    /// Cards, card set, street and remaining deck for every board state.
    pub const BOARDS: &str = "board metadata";
    /// One interned showdown table per complete board.
    pub const SHOWDOWN_TABLES: &str = "showdown tables";
    /// Both players' blocker masks for every dealt card.
    pub const MASK_POOL: &str = "chance mask pool";
    /// Range weights, evaluator rank groups and the layout header.
    pub const RANGES: &str = "ranges and evaluator tables";
    /// Cumulative regrets, one entry per state-action.
    pub const REGRETS: &str = "regrets";
    /// Reach-weighted cumulative strategy, one entry per state-action.
    pub const STRATEGY_SUMS: &str = "strategy sums";
    /// The current policy, stored before step 6 and derived at visit time now.
    pub const CURRENT_POLICY: &str = "current policy";
    /// Iteration counters, layout handle and the estimate's slack.
    pub const CFR_BOOKKEEPING: &str = "CFR bookkeeping";
    /// Retained average strategies.
    pub const SNAPSHOTS: &str = "average-strategy snapshots";
    /// One `f32` scale per stored array per decision node, under `i16` only.
    pub const SCALES: &str = "per-node compression scales";
    /// Recursive value vectors, one set per traversal worker.
    pub const TRAVERSAL: &str = "traversal value buffers";
    /// One terminal evaluation workspace per traversal worker.
    pub const SCRATCH: &str = "terminal showdown scratch";
    /// The workspace a strategy query overlapping an iteration holds.
    pub const QUERY_WORKSPACE: &str = "query workspace";
    /// One returned action-value report.
    pub const DECISION_REPORT: &str = "decision-value report";
    /// One returned per-node value report, for both players.
    pub const NODE_REPORT: &str = "node-value report";
    /// Everything construction holds and frees before a solver exists.
    pub const CONSTRUCTION: &str = "construction transients";
    /// The full unmerged best-response walk, which owns no buffers of its own.
    pub const VERIFICATION: &str = "best-response verification walk";
}

/// How long one row's buffers live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLifetime {
    /// Allocated and freed inside `PostflopGame::new`.
    Construction,
    /// Held from construction, or from the solver's creation, until it drops.
    WholeSolve,
    /// Taken at the start of an iteration and released when it ends.
    PerIteration,
    /// Held by one strategy query and released when its report drops.
    PerQuery,
    /// Held by one best-response measurement.
    PerVerification,
    /// Taken on request and held until the caller drops the object it is in,
    /// which no phase of a solve bounds.
    HeldByCaller,
}

impl MemoryLifetime {
    /// The spelling used in the printed table and in the README.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Construction => "construction only",
            Self::WholeSolve => "whole solve",
            Self::PerIteration => "per iteration",
            Self::PerQuery => "per query",
            Self::PerVerification => "per verification",
            Self::HeldByCaller => "held while the caller keeps it",
        }
    }
}

/// Whether the bound adds a row, and what else is the same allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryOverlap {
    /// Added to the bound, and alive at the same time as every other counted
    /// row.
    Counted,
    /// Added to the bound although it is freed before the rows it is added to
    /// ever exist. The bound is a peak bound, not one instant.
    CountedFreedEarly,
    /// Not added: these bytes are the rows named here, under another lifetime.
    /// The names are [`rows`] constants, so a renamed row cannot leave a stale
    /// sentence behind.
    Aliases(&'static [&'static str]),
}

impl MemoryOverlap {
    /// True when [`PostflopMemory::bound_under`] adds this row.
    #[must_use]
    pub fn is_counted(self) -> bool {
        matches!(self, Self::Counted | Self::CountedFreedEarly)
    }

    /// The spelling used in the printed table and in the README.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Counted => "no",
            Self::CountedFreedEarly => "freed before the solve",
            Self::Aliases(_) => "yes, not counted again",
        }
    }
}

/// One buffer in the working set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryRow {
    /// One of the names in [`rows`].
    pub name: &'static str,
    /// What the bytes hold, in the width they hold it.
    pub representation: &'static str,
    /// Bytes this row takes under the [`StoragePlan`] it was built for.
    pub bytes: usize,
    /// State-action entries inside `bytes`, summed over this row's arrays.
    pub entries: usize,
    /// Independently stored entry arrays inside this row, which is how many
    /// per-node scales an `i16` layout needs for it. Those scales are charged
    /// together in the [`rows::SCALES`] row, not here.
    pub arrays: usize,
    /// How long the row's buffers live.
    pub lifetime: MemoryLifetime,
    /// Whether the bound adds the row, and what else is the same allocation.
    pub overlap: MemoryOverlap,
    /// What a reader of the table needs to know that the fields do not say.
    pub note: &'static str,
}

/// A storage layout to price, so the later steps' arithmetic is written once.
///
/// [`PostflopMemory::plan`] is what the code does now, and its bound is exactly
/// [`PostflopMemory::working_set_bound_bytes`]. Every other plan is arithmetic
/// over the same entry counts: `f32` and `i16` are not implemented yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoragePlan {
    /// Width of one stored state-action entry.
    pub precision: Precision,
    /// Private states charged per player: the combos with positive weight the
    /// board prefix leaves, or 1326 for a plan priced without ranges.
    pub states: [usize; 2],
    /// Average strategies retained at once. The implemented plan charges one
    /// browsing snapshot; a running solve retains none of its own.
    pub snapshots: usize,
    /// Whether the current policy is stored beside the regrets, as it was
    /// before step 6, or derived at visit time as it is now.
    pub store_current_policy: bool,
}

impl StoragePlan {
    /// What the crate stored before step 6: three arrays over all 1326 combos,
    /// with two retained averages charged at once.
    #[must_use]
    pub fn before_compaction() -> Self {
        Self {
            precision: Precision::F64,
            states: [STATES; 2],
            snapshots: 2,
            store_current_policy: true,
        }
    }

    /// The same layout at another entry width.
    #[must_use]
    pub fn at(self, precision: Precision) -> Self {
        Self { precision, ..self }
    }

    /// The same layout over another pair of live-combo counts.
    #[must_use]
    pub fn over(self, states: [usize; 2]) -> Self {
        Self { states, ..self }
    }
}

/// Rows the best-response verification walk borrows instead of allocating.
/// A parallel measurement takes the first two; a serial
/// `PostflopStrategy::exploitability` takes the query workspace instead.
pub const VERIFICATION_ALIASES: &[&str] = &[rows::TRAVERSAL, rows::SCRATCH, rows::QUERY_WORKSPACE];

/// Bytes one stored state-action entry takes.
#[must_use]
pub fn bytes_per_entry(precision: Precision) -> usize {
    match precision {
        Precision::F64 => 8,
        Precision::F32 => 4,
        Precision::I16 => 2,
    }
}

/// A `Budget` reservation this crate's postflop callers make, and the rows it
/// draws from. Every fixed-size reservation site is named here. Imports and
/// diagnostic current-policy rows have variable capacity reservations. Flat
/// input can exceed the snapshot row; a row import also leases consumed buffers
/// until flattening finishes. A CurrentPolicyRow reserves its header and payload
/// while held, fitting within the decision-report allowance when used in place
/// of that report. Concurrent retained diagnostics each take their own lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryReservation {
    /// `streets/game.rs`: `Budget::new` opens the budget already holding the
    /// shared game.
    Shared,
    /// `streets/solver.rs`: `PostflopSolver::new` reserves the CFR arrays and
    /// one terminal workspace per worker for the life of the solver.
    Solver,
    /// `streets/solver.rs`: `reserve_workspace` reserves the traversal buffers
    /// for one iteration or one measurement, and releases them at its end.
    Iteration,
    /// `streets/solver.rs` and `streets/strategy.rs`: one retained average, from
    /// `average_strategy`, `uniform` or `from_rows`.
    Snapshot,
    /// `streets/strategy.rs`: `reserve_workspace` reserves one serial traversal
    /// buffer set and one scratch for a query or a best-response walk.
    Query,
    /// `streets/strategy.rs`: `decision_values` reserves its returned report.
    DecisionReport,
    /// `streets/strategy.rs`: `node_values` reserves its returned report, which
    /// holds one value, reach and opposing-mass vector per player rather than
    /// one value vector per action.
    NodeReport,
}

impl MemoryReservation {
    /// Every reservation site, for a test that wants to cover all of them.
    pub const ALL: [Self; 7] = [
        Self::Shared,
        Self::Solver,
        Self::Iteration,
        Self::Snapshot,
        Self::Query,
        Self::DecisionReport,
        Self::NodeReport,
    ];

    /// Rows this reservation is made of.
    #[must_use]
    pub fn row_names(self) -> &'static [&'static str] {
        match self {
            Self::Shared => &[
                rows::TREE,
                rows::TOPOLOGY,
                rows::CHANCE,
                rows::BOARDS,
                rows::SHOWDOWN_TABLES,
                rows::MASK_POOL,
                rows::RANGES,
            ],
            Self::Solver => &[
                rows::REGRETS,
                rows::STRATEGY_SUMS,
                rows::CFR_BOOKKEEPING,
                rows::SCALES,
                rows::SCRATCH,
            ],
            Self::Iteration => &[rows::TRAVERSAL],
            Self::Snapshot => &[rows::SNAPSHOTS],
            Self::Query => &[rows::QUERY_WORKSPACE],
            Self::DecisionReport => &[rows::DECISION_REPORT],
            Self::NodeReport => &[rows::NODE_REPORT],
        }
    }

    /// How many of these the bound charges at once.
    ///
    /// One of each. A solve retains no average of its own after step 6: the
    /// best-response walk normalises the strategy sums as it reads them, so the
    /// snapshot in the default bound is the one a caller browsing a result
    /// holds. The byte budget does not enforce a snapshot count: spare capacity
    /// can admit another. The 5d driver registry must enforce its object limit.
    /// Every additional snapshot, query or diagnostic takes its own reservation;
    /// insufficient remaining bytes return a named refusal.
    #[must_use]
    pub fn charged(self) -> usize {
        let _ = self;
        1
    }

    /// Bytes one such reservation takes, under the implemented layout.
    ///
    /// This is one reservation, not the charge: the bound holds
    /// [`Self::charged`] of them. Summing every site at its charged count, and
    /// adding the construction transients that no reservation covers because
    /// they are freed before a solver exists, gives the whole bound.
    #[must_use]
    pub fn bytes(self, memory: &PostflopMemory) -> usize {
        match self {
            Self::Shared => memory.shared_bytes,
            Self::Solver => memory.solver_bytes + memory.workers * memory.scratch_bytes,
            Self::Iteration => memory.workers * memory.traversal_bytes,
            Self::Snapshot => memory.snapshot_bytes,
            Self::Query => memory.traversal_bytes + memory.scratch_bytes,
            Self::DecisionReport => memory.decision_bytes,
            Self::NodeReport => memory.node_bytes,
        }
    }
}

/// Byte counts the row breakdown needs that the aggregate fields have already
/// added together. Every one of them is a term of the sums below, so the rows
/// and the aggregates cannot disagree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Parts {
    tree_bytes: usize,
    topology_bytes: usize,
    chance_bytes: usize,
    board_bytes: usize,
    showdown_bytes: usize,
    mask_pool_bytes: usize,
    range_bytes: usize,
    /// `Cfr` itself and the estimate's slack, outside the stored arrays.
    cfr_overhead_bytes: usize,
    /// `Strategy` itself and the estimate's slack, per snapshot.
    snapshot_overhead_bytes: usize,
    /// Levels a recursive walk can be nested to, and value vectors it can hold
    /// per level. The traversal row is these two times one vector's bytes, so a
    /// plan over other state counts prices the same buffers.
    traversal_levels: usize,
    traversal_vectors: usize,
}

/// Conservative allocations for one postflop game and its checked operations.
///
/// The five byte fields below `shared_bytes` mean exactly what the river's
/// fields of the same name mean. The three counts above them describe the
/// expanded tree the compact tree turns into, and are reported so a capture can
/// state what it solved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostflopMemory {
    /// Distinct boards the expansion reaches, counting the root board.
    pub board_states: usize,
    /// Complete five-card boards, one showdown table each before deduplication.
    pub showdown_tables: usize,
    /// Public nodes after every chance node is expanded into its runouts.
    pub expanded_nodes: usize,
    /// Decision nodes among them, which is how many per-node compression scales
    /// one stored array would need.
    pub expanded_decision_nodes: usize,
    /// Action slots over the expanded decision nodes of each player, indexed by
    /// the acting player. One stored array holds `slots[0] * states[0] +
    /// slots[1] * states[1]` entries.
    pub action_slots: [usize; 2],
    /// Private states the game actually carries, one per live combo after
    /// in-range compaction. `[1326; 2]` when a caller priced a tree without
    /// naming ranges.
    pub states: [usize; 2],
    /// Traversal workers this estimate charged for, from `resolve_workers`.
    pub workers: usize,
    /// Retained tree, ranges, traversal metadata, showdown tables and mask pool.
    pub shared_bytes: usize,
    /// CFR regrets, strategy sums and their metadata; policy is derived.
    pub solver_bytes: usize,
    /// One retained average or imported strategy.
    pub snapshot_bytes: usize,
    /// Maximum temporary recursive traversal buffers, per worker.
    pub traversal_bytes: usize,
    /// One checked terminal evaluation workspace, per worker.
    pub scratch_bytes: usize,
    /// One returned decision-value report and its combo reach vectors.
    pub decision_bytes: usize,
    /// One returned node-value report: both players' values, reach and
    /// compatible opposing mass.
    pub node_bytes: usize,
    /// Temporary buffers construction holds and frees before the solve: the
    /// temporary topology, interning maps, per-board deal table and validation's
    /// visited flags, pending-node stack, showdown scratch and the pair matrix
    /// its zero-sum pass cannot stream away.
    pub construction_bytes: usize,
    /// Shared game, one solver, one snapshot, per-worker workspaces, both reports
    /// and the construction transients. The sum is a bound on the peak, not a
    /// snapshot of one instant: construction has freed its transients before a
    /// solver exists, so no run holds every term at once.
    ///
    /// The `workers + 1` workspace terms cover one strategy query overlapping a
    /// running iteration, and one only. A second query overlapping the first is
    /// not in this bound: it reserves its own report and workspaces from the
    /// same budget, which returns [`SolveError::MemoryLimit`] as soon as the
    /// configured limit is reached instead of allocating past it. Two
    /// concurrent queries therefore need a configured limit above this bound by
    /// another `decision_bytes`, `traversal_bytes` and `scratch_bytes`.
    pub working_set_bound_bytes: usize,
    /// Terms of the sums above, kept so the row breakdown restates the bound
    /// instead of recomputing it.
    parts: Parts,
}

fn overflow() -> SolveError {
    SolveError::Allocation("postflop memory estimate overflow".into())
}

fn sum(values: &[usize]) -> Result<usize, SolveError> {
    values
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(*value))
        .ok_or_else(overflow)
}

fn product(a: usize, b: usize) -> Result<usize, SolveError> {
    a.checked_mul(b).ok_or_else(overflow)
}

/// Board states at each street, and the fan-out of a chance node on it.
///
/// A chance node deals every card that is not already on the board. Its
/// probability divides by that count less the four private cards, so a board
/// that leaves four or fewer unseen cards has no legal deal and is rejected
/// here rather than producing an infinite probability later.
fn board_states(start: Street, board_len: usize) -> Result<([usize; 3], [usize; 3]), SolveError> {
    let mut states = [0_usize; 3];
    let mut outcomes = [0_usize; 3];
    let mut count = 1_usize;
    let mut cards = board_len;
    let mut street = Some(start);
    while let Some(current) = street {
        states[current.index()] = count;
        if current.next().is_some() {
            let fan = DECK
                .checked_sub(cards)
                .filter(|unseen| *unseen > PRIVATE_CARDS)
                .ok_or_else(|| {
                    SolveError::InvalidGame(format!(
                        "a {cards}-card board leaves too few cards to deal the {} street",
                        current.next().expect("checked above")
                    ))
                })?;
            outcomes[current.index()] = fan;
            count = product(count, fan)?;
            cards += 1;
        }
        street = current.next();
    }
    Ok((states, outcomes))
}

/// Per-street totals read from the compact tree in one pass.
#[derive(Clone, Copy, Debug, Default)]
struct CompactTotals {
    /// Nodes of every kind on each street.
    nodes: [usize; 3],
    /// Decision nodes on each street.
    decisions: [usize; 3],
    /// Sum of action counts over the street's decision nodes, by acting player.
    action_slots: [[usize; 3]; 2],
    /// Chance nodes on each street.
    chance: [usize; 3],
    /// Largest action menu anywhere in the tree.
    max_actions: usize,
}

fn compact_totals(tree: &PostflopTree) -> Result<CompactTotals, SolveError> {
    let mut totals = CompactTotals::default();
    for node in tree.nodes() {
        let street = node.street().index();
        totals.nodes[street] = totals.nodes[street].checked_add(1).ok_or_else(overflow)?;
        match node.kind() {
            PostflopNodeKind::Decision { player } => {
                let actions = node.actions().len();
                let side = usize::from(player);
                if side >= 2 {
                    return Err(SolveError::InvalidGame(format!(
                        "a {} decision node acts for player {player}, and this game has two",
                        node.street()
                    )));
                }
                totals.decisions[street] = totals.decisions[street]
                    .checked_add(1)
                    .ok_or_else(overflow)?;
                totals.action_slots[side][street] = totals.action_slots[side][street]
                    .checked_add(actions)
                    .ok_or_else(overflow)?;
                totals.max_actions = totals.max_actions.max(actions);
            }
            PostflopNodeKind::Chance { .. } => {
                totals.chance[street] =
                    totals.chance[street].checked_add(1).ok_or_else(overflow)?;
            }
            PostflopNodeKind::Terminal(_) => {}
        }
    }
    Ok(totals)
}

/// Bound on the construction-time path validation's own buffers.
///
/// The walk keeps one visited flag per expanded node and an explicit stack of
/// pending nodes, each entry carrying a node ID, a depth and both players'
/// live-state flags. Every node on the current path leaves at most its siblings
/// pending, and the widest fan-out is a chance node's 52 outcomes or the widest
/// action menu, whichever is larger, so the stack holds at most
/// `(depth + 1) * max(52, max_actions)` entries. It starts at capacity one and
/// doubles, so the allocation behind it is charged at twice its peak length.
/// That doubling is charged on the whole entry, including the flag buffers,
/// while the spare capacity a doubling leaves holds no buffers at all, so the
/// stack term is a bound above what the walk can hold rather than a count.
///
/// The pair half holds one utility per scoped pair between its two passes,
/// which the pair budget caps, and beside it one reusable full-width column,
/// one one-hot opponent reach, the compatibility mask table, and the column
/// source's terminal workspace, including both boxed payloads. Compatibility
/// masks and scoped indices each hold one entry per private state per player.
/// These buffers are charged at full width. They are built before the walk
/// decides whether the pair checks fit, so they are charged either way.
fn validation_bytes(
    expanded_nodes: usize,
    max_depth: usize,
    max_actions: usize,
) -> Result<usize, SolveError> {
    let entry = sum(&[
        size_of::<(crate::NodeId, usize, [Vec<bool>; 2])>(),
        product(2 * STATES, size_of::<bool>())?,
    ])?;
    let stack = product(
        product(sum(&[max_depth, 1])?, DECK.max(max_actions))?,
        entry,
    )?;
    sum(&[
        product(expanded_nodes, size_of::<bool>())?,
        product(2, stack)?,
        product(VALIDATION_PAIR_LIMIT, size_of::<f64>())?,
        product(2 * STATES, size_of::<f64>())?,
        product(2 * STATES, size_of::<u64>())?,
        // Filtered collection of scoped indices can retain spare capacity.
        product(4 * STATES, size_of::<usize>())?,
        terminal_workspace_bytes()?,
        1024,
    ])
}

/// Both players' blocker masks for one dealt card, over their live combos,
/// plus the two vector headers, for every card the tree can deal.
fn mask_pool_for(states: [usize; 2]) -> Result<usize, SolveError> {
    sum(&[
        product(MASK_POOL_ENTRIES, product(sum(&states)?, size_of::<f64>())?)?,
        // Vec::push grows the outer pool to the next power of two.
        product(
            MASK_POOL_ENTRIES.next_power_of_two(),
            size_of::<[Vec<f64>; 2]>(),
        )?,
    ])
}

/// One terminal workspace, including the buffers owned through its two boxes.
/// The worker stores it in a Mutex inside a Vec; a serial query stores it on
/// its stack. Charging the larger wrapper also covers the query's lease.
fn terminal_workspace_bytes() -> Result<usize, SolveError> {
    sum(&[
        size_of::<Mutex<TerminalWorkspace>>(),
        product(2 * STATES, size_of::<f64>())?,
        size_of::<Vec<Mutex<TerminalWorkspace>>>(),
        size_of::<Lease>(),
    ])
}

/// One worker's recursive value buffers: `levels` nested levels each holding
/// `vectors` vectors as wide as the larger player's live-combo count.
fn traversal_for(levels: usize, vectors: usize, states: [usize; 2]) -> Result<usize, SolveError> {
    product(
        levels,
        product(
            vectors,
            sum(&[product(states[0].max(states[1]), size_of::<f64>())?, 128])?,
        )?,
    )
}

impl PostflopMemory {
    /// Bounds every buffer the game, one solver, one snapshot and one report
    /// can hold, for a tree expanded over `board_len` known board cards, run by
    /// `workers` traversal workers, over `live` combos per player.
    pub(super) fn estimate(
        tree: &PostflopTree,
        board_len: usize,
        workers: usize,
        live: [usize; 2],
    ) -> Result<Self, SolveError> {
        for count in live {
            if count == 0 || count > STATES {
                return Err(SolveError::InvalidGame(format!(
                    "a postflop game carries between 1 and {STATES} live combos, not {count}"
                )));
            }
        }
        let start = tree.config().start_street;
        let (states, outcomes) = board_states(start, board_len)?;
        let totals = compact_totals(tree)?;

        let mut expanded_nodes = 0;
        let mut expanded_decisions = 0;
        let mut slots = [0_usize; 2];
        let mut chance_outcomes = 0;
        let mut board_state_total = 0;
        for street in [Street::Flop, Street::Turn, Street::River] {
            let index = street.index();
            expanded_nodes = sum(&[expanded_nodes, product(totals.nodes[index], states[index])?])?;
            expanded_decisions = sum(&[
                expanded_decisions,
                product(totals.decisions[index], states[index])?,
            ])?;
            for (side, slot) in slots.iter_mut().enumerate() {
                *slot = sum(&[
                    *slot,
                    product(totals.action_slots[side][index], states[index])?,
                ])?;
            }
            chance_outcomes = sum(&[
                chance_outcomes,
                product(
                    product(totals.chance[index], states[index])?,
                    outcomes[index],
                )?,
            ])?;
            board_state_total = sum(&[board_state_total, states[index]])?;
        }
        let action_slots = sum(&slots)?;
        if expanded_nodes > crate::NodeId::MAX as usize {
            return Err(SolveError::InvalidGame(format!(
                "expanding this tree needs {expanded_nodes} public nodes, above the {} the node index holds",
                crate::NodeId::MAX
            )));
        }
        // One showdown table per complete board, interned on the board's card
        // set. A flop tree reaches the river through an ordered pair of dealt
        // cards, and the two orders name the same five-card board, so it builds
        // half as many tables as it has river board states. A turn tree deals
        // one card and its river states are already distinct.
        let orderings = match start {
            Street::Flop => 2,
            Street::Turn | Street::River => 1,
        };
        let showdown_tables = states[Street::River.index()] / orderings;
        let edges = sum(&[action_slots, chance_outcomes])?;

        let entries = sum(&[product(slots[0], live[0])?, product(slots[1], live[1])?])?;
        let rows = product(entries, size_of::<f64>())?;
        let snapshot_overhead = sum(&[size_of::<Strategy>(), 256])?;
        let snapshot_bytes = sum(&[snapshot_overhead, rows])?;
        let cfr_overhead = sum(&[size_of::<Cfr>(), 1024])?;
        // Two arrays, not three: the current policy is regret matching over the
        // regrets, derived where a walk reads it.
        let solver_bytes = sum(&[cfr_overhead, product(rows, 2)?])?;
        let mask_pool_bytes = mask_pool_for(live)?;
        // Rank groups use at most 2048 entries of two usize values, plus 1081
        // ranked combos, matching the river estimate's fixed evaluator terms.
        // The four full-width `f64` vectors are the two input ranges and the
        // two board-filtered ones the public API and the reports read; beside
        // them the compaction tables hold one `u16` per combo per player each
        // way, which the last term covers with room to spare.
        let range_bytes = sum(&[
            312_320,
            4096,
            size_of::<TraversalLayout>(),
            product(4 * STATES, size_of::<f64>())?,
            product(4 * STATES, size_of::<u16>())?,
        ])?;
        // Struct-of-arrays topology: the kind, the edge offset and the row
        // offset from `TraversalLayout`, and the compact id, board, subtree
        // end, payoff index and parent link from the expansion. No per-node
        // `Vec` header and no inline payoff record survive step 6.
        let per_node = sum(&[
            size_of::<NodeKind>(),
            size_of::<u32>(),
            size_of::<u64>(),
            5 * size_of::<u32>(),
            size_of::<crate::NodeId>(),
        ])?;
        let topology_bytes = sum(&[
            product(expanded_nodes, per_node)?,
            product(edges, size_of::<crate::NodeId>())?,
            // Every distinct payoff originates in a compact node. The Vec
            // grows by doubling, with a minimum allocation of four records.
            product(product(tree.nodes().len().max(4), 2)?, size_of::<Payoff>())?,
            1024,
        ])?;
        // Chance probabilities and mask-pool indices run parallel to the edge
        // array, so an action edge carries an unread pair. That is 12 bytes per
        // action edge against the 24-byte `Vec` header per node the flat layout
        // dropped.
        let chance_bytes = product(edges, size_of::<f64>() + size_of::<u32>())?;
        // Board metadata: five cards, a card set, a street and the deck of
        // cards still to come, with room for the vector headers.
        let board_bytes = product(board_state_total, DECK + 128)?;
        let tree_bytes = tree.storage_bytes();
        let showdown_bytes = product(showdown_tables, SHOWDOWN_TABLE_BYTES)?;
        let shared_bytes = sum(&[
            tree_bytes,
            showdown_bytes,
            mask_pool_bytes,
            range_bytes,
            topology_bytes,
            chance_bytes,
            board_bytes,
        ])?;
        // A serial walk holds one value vector per action at a decision node and
        // one at a time at a chance node. Above one worker a chance node instead
        // collects every outcome's vector before it reduces them in outcome
        // order, so the widest level is the widest deal rather than the widest
        // bet menu. One worker keeps the serial width, and every number a serial
        // solve has already recorded with it.
        //
        // Nested gathers are covered by the same term. A flop-start solve can
        // hold one outer turn-deal gather of up to 49 vectors while up to
        // `workers` river-deal gathers of 48 are in flight, so at most
        // 49 + 48 * workers vectors exist at once. The bound charges
        // (workers + 1) * (max_depth + 2) * (widest + 8) vectors, and with
        // widest = 49 even the shallowest tree charges 114 * (workers + 1),
        // which is above 49 + 48 * workers for every worker count and every
        // depth the tree allows.
        //
        // Since step 6 a decision node also derives its policy row into a
        // pooled buffer of `states * actions` entries, which is one more vector
        // per action on top of the per-action values it already held, so the
        // width doubles.
        let widest = if workers > 1 {
            totals
                .max_actions
                .max(outcomes.into_iter().max().unwrap_or(0))
        } else {
            totals.max_actions
        };
        let traversal_levels = tree.max_depth() + 2;
        let traversal_vectors = sum(&[product(2, widest)?, 8])?;
        let traversal_bytes = traversal_for(traversal_levels, traversal_vectors, live)?;
        let decision_bytes = sum(&[
            product(
                product(STATES, totals.max_actions)?,
                size_of::<Option<f64>>(),
            )?,
            2 * STATES * size_of::<f64>(),
            512,
        ])?;
        // Both players' per-combo values, reach and compatible opposing mass,
        // counted rather than assumed to be a multiple of the decision report:
        // this one is sized by the player count, that one by the action count,
        // and which is larger depends on the widest menu in the tree.
        let node_bytes = sum(&[
            product(product(2, STATES)?, size_of::<Option<f64>>())?,
            4 * STATES * size_of::<f64>(),
            512,
        ])?;
        // One `ShowdownScratch` plus the two full-width vectors the compacted
        // walk scatters into and gathers out of at the terminal boundary. Those
        // two stay 1326 wide whatever the ranges are: `ShowdownTable` and
        // `evaluate_fold` are written against combo IDs.
        let scratch_bytes = terminal_workspace_bytes()?;
        // Construction transients, freed before the solver exists but held at
        // the same time as everything in `shared_bytes`, so the refusal has to
        // cover them. The NodeBuild array and its separately allocated edge,
        // probability and mask buffers remain alive while TraversalLayout
        // allocates its flat copies. Child buffers held by recursive calls are
        // included in those same edge counts even before the call installs them.
        // Per board: one 52-entry child table of card indices and
        // its vector header (including the outer Vec's spare capacity).
        // Per complete board: one entry in the map that interns showdown tables
        // on the card set. Per dealt card: one entry in
        // the map that interns mask-pool indices.
        let construction_bytes = sum(&[
            product(expanded_nodes, size_of::<NodeBuild>())?,
            product(edges, size_of::<crate::NodeId>())?,
            product(chance_outcomes, size_of::<f64>() + size_of::<u32>())?,
            // One copied remaining-deck vector per active expansion level.
            product(tree.max_depth() + 1, DECK + size_of::<Vec<cards::Card>>())?,
            product(board_state_total, product(DECK, size_of::<u32>())?)?,
            product(product(board_state_total.max(4), 2)?, size_of::<Vec<u32>>())?,
            // Payoff-key maps also grow during expansion; keys contain a tag,
            // an f64 bit pattern and an index, so they need wider buckets.
            product(tree.nodes().len(), 96)?,
            512,
            product(showdown_tables, MAP_ENTRY_BYTES)?,
            product(MASK_POOL_ENTRIES, MAP_ENTRY_BYTES)?,
            validation_bytes(expanded_nodes, tree.max_depth(), totals.max_actions)?,
        ])?;
        let working_set_bound_bytes = sum(&[
            shared_bytes,
            solver_bytes,
            // One retained average, for a caller browsing a finished result. A
            // running solve keeps none: its measurement normalises the strategy
            // sums as it reads them.
            snapshot_bytes,
            // A strategy query can run while an iteration holds its own
            // workspaces: one traversal buffer and one scratch per worker for
            // the iteration, plus one of each for the query.
            product(scratch_bytes, sum(&[workers, 1])?)?,
            product(traversal_bytes, sum(&[workers, 1])?)?,
            decision_bytes,
            node_bytes,
            construction_bytes,
        ])?;
        Ok(Self {
            board_states: board_state_total,
            showdown_tables,
            expanded_nodes,
            expanded_decision_nodes: expanded_decisions,
            action_slots: slots,
            states: live,
            workers,
            shared_bytes,
            solver_bytes,
            snapshot_bytes,
            traversal_bytes,
            scratch_bytes,
            decision_bytes,
            node_bytes,
            construction_bytes,
            working_set_bound_bytes,
            parts: Parts {
                tree_bytes,
                topology_bytes,
                chance_bytes,
                board_bytes,
                showdown_bytes,
                mask_pool_bytes,
                range_bytes,
                cfr_overhead_bytes: cfr_overhead,
                snapshot_overhead_bytes: snapshot_overhead,
                traversal_levels,
                traversal_vectors,
            },
        })
    }

    /// The layout this crate implements: two stored arrays, the current policy
    /// derived at visit time, one snapshot for a caller browsing a result, and
    /// one entry per live combo. [`Self::working_set_bound_bytes`] is exactly
    /// [`Self::bound_under`] of this plan.
    #[must_use]
    pub fn plan(&self) -> StoragePlan {
        StoragePlan {
            precision: Precision::F64,
            states: self.states,
            snapshots: 1,
            store_current_policy: false,
        }
    }

    /// The same estimate for a tree no game has been built from.
    ///
    /// `PostflopGame::new` refuses a tree whose bound is above the configured
    /// limit, which is exactly the case a memory table has to report on, so the
    /// arithmetic is reachable without a game. `board_len` is the known board
    /// the tree would start from: three cards on the flop, four on the turn,
    /// five on the river. No ranges are named, so it prices all 1326 combos,
    /// which is the upper bound over every pair of ranges; a table asks for the
    /// live counts through [`Self::bound_under`].
    pub fn for_tree(
        tree: &PostflopTree,
        board_len: usize,
        workers: usize,
    ) -> Result<Self, SolveError> {
        Self::for_tree_over(tree, board_len, workers, [STATES; 2])
    }

    /// The same, over a named pair of live-combo counts.
    pub fn for_tree_over(
        tree: &PostflopTree,
        board_len: usize,
        workers: usize,
        live: [usize; 2],
    ) -> Result<Self, SolveError> {
        let start = tree.config().start_street;
        let expected = match start {
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River => 5,
        };
        if board_len != expected {
            return Err(SolveError::InvalidGame(format!(
                "a {start} tree needs a {expected}-card board, not {board_len}"
            )));
        }
        if workers == 0 {
            return Err(SolveError::Config(
                "a memory estimate needs at least one traversal worker".into(),
            ));
        }
        Self::estimate(tree, board_len, workers, live)
    }

    /// State-action entries in one stored array under `plan`.
    ///
    /// Each player's action slots are charged that player's private states, so
    /// a compacted plan prices the two ranges separately rather than assuming
    /// they are the same size.
    pub fn entries_under(&self, plan: &StoragePlan) -> Result<usize, SolveError> {
        for states in plan.states {
            if states == 0 || states > STATES {
                return Err(SolveError::InvalidGame(format!(
                    "a storage plan charges between 1 and {STATES} private states, not {states}"
                )));
            }
        }
        sum(&[
            product(self.action_slots[0], plan.states[0])?,
            product(self.action_slots[1], plan.states[1])?,
        ])
    }

    /// One row per buffer, under the layout the code implements today. The
    /// counted rows sum to [`Self::working_set_bound_bytes`].
    pub fn rows(&self) -> Result<Vec<MemoryRow>, SolveError> {
        self.rows_under(&self.plan())
    }

    /// One row per buffer under `plan`.
    ///
    /// Every row whose size depends on how many private states a walk carries
    /// follows the plan: the stored arrays, the snapshots, the compression
    /// scales, the chance mask pool and the traversal buffers. The rest do not,
    /// and they are not oversights. The terminal boundary stays 1326 wide
    /// because `ShowdownTable` and `evaluate_fold` are written against combo
    /// IDs, so the walk scatters into a full-width vector and gathers back out
    /// of one; the two reports stay 1326 wide because a consumer of a solved
    /// spot asks in combo IDs; and the topology does not depend on the ranges
    /// at all.
    pub fn rows_under(&self, plan: &StoragePlan) -> Result<Vec<MemoryRow>, SolveError> {
        let width = bytes_per_entry(plan.precision);
        let entries = self.entries_under(plan)?;
        let array = product(entries, width)?;
        let stored_arrays = if plan.store_current_policy { 3 } else { 2 };
        let snapshots = sum(&[
            product(array, plan.snapshots)?,
            product(self.parts.snapshot_overhead_bytes, plan.snapshots)?,
        ])?;
        let scale_arrays = stored_arrays + plan.snapshots;
        let scales = if plan.precision == Precision::I16 {
            product(
                product(self.expanded_decision_nodes, scale_arrays)?,
                size_of::<f32>(),
            )?
        } else {
            0
        };
        let mask_pool_bytes = mask_pool_for(plan.states)?;
        let traversal_bytes = traversal_for(
            self.parts.traversal_levels,
            self.parts.traversal_vectors,
            plan.states,
        )?;
        /// What every stored entry array holds, whatever its width.
        const STORED_ARRAY: &str = "one entry per state-action, in one flat buffer";
        let mut table = vec![
            MemoryRow {
                name: rows::TREE,
                representation: "compact PostflopNode records, actions and children",
                bytes: self.parts.tree_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "one street block per street, not one per board",
            },
            MemoryRow {
                name: rows::TOPOLOGY,
                representation: "kinds, edge and row offsets, u32 child edges, per-node board, payoff, subtree end and parent links",
                bytes: self.parts.topology_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "struct-of-arrays with u64 row offsets: no per-node Vec header and no inline payoff record",
            },
            MemoryRow {
                name: rows::CHANCE,
                representation: "one f64 probability and one u32 mask index per edge",
                bytes: self.parts.chance_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "",
            },
            MemoryRow {
                name: rows::BOARDS,
                representation: "five cards, a card set, a street and the remaining deck per board state",
                bytes: self.parts.board_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "",
            },
            MemoryRow {
                name: rows::SHOWDOWN_TABLES,
                representation: "one interned ShowdownTable per complete board, bounded at 64 KiB",
                bytes: self.parts.showdown_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "one per completed board, interned on its card set: a flop tree's two deal orders share one",
            },
            MemoryRow {
                name: rows::MASK_POOL,
                representation: "both players' f64 blocker masks over their live combos, per dealt card",
                bytes: mask_pool_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "interned on the dealt card, so 52 entries bound every deal in the tree",
            },
            MemoryRow {
                name: rows::RANGES,
                representation: "two f64 range vectors, evaluator rank groups, layout header",
                bytes: self.parts.range_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "",
            },
            MemoryRow {
                name: rows::REGRETS,
                representation: STORED_ARRAY,
                bytes: array,
                entries,
                arrays: 1,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "",
            },
            MemoryRow {
                name: rows::STRATEGY_SUMS,
                representation: STORED_ARRAY,
                bytes: array,
                entries,
                arrays: 1,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "the best-response walk normalises these per node as it reads them, so a measurement retains nothing",
            },
        ];
        if plan.store_current_policy {
            table.push(MemoryRow {
                name: rows::CURRENT_POLICY,
                representation: STORED_ARRAY,
                bytes: array,
                entries,
                arrays: 1,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "stored before step 6; the walks now derive it from the regrets at visit time",
            });
        }
        table.push(MemoryRow {
            name: rows::CFR_BOOKKEEPING,
            representation: "Cfr itself, its layout handle and the estimate's slack",
            bytes: self.parts.cfr_overhead_bytes,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::WholeSolve,
            overlap: MemoryOverlap::Counted,
            note: "",
        });
        table.push(MemoryRow {
            name: rows::SNAPSHOTS,
            representation: "retained average strategies, the same width and shape as one stored array",
            bytes: snapshots,
            entries: product(entries, plan.snapshots)?,
            arrays: plan.snapshots,
            lifetime: MemoryLifetime::HeldByCaller,
            overlap: MemoryOverlap::Counted,
            note: "one alive by default, for a caller browsing a result; a running solve retains none",
        });
        table.push(MemoryRow {
            name: rows::SCALES,
            representation: "one f32 scale per stored array per decision node",
            bytes: scales,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::WholeSolve,
            overlap: MemoryOverlap::Counted,
            note: "zero unless the entries are i16",
        });
        table.push(MemoryRow {
            name: rows::TRAVERSAL,
            representation: "f64 value vectors over the live combos, one set per worker",
            bytes: product(traversal_bytes, self.workers)?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerIteration,
            overlap: MemoryOverlap::Counted,
            note: "above one worker a chance node gathers every outcome, so the width is the widest deal",
        });
        table.push(MemoryRow {
            name: rows::SCRATCH,
            representation: "one locked TerminalWorkspace, both boxed scatter/gather payloads and wrappers per worker",
            bytes: product(self.scratch_bytes, self.workers)?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::WholeSolve,
            overlap: MemoryOverlap::Counted,
            note: "the two 1326-wide vectors stay full width: the showdown tables are written against combo IDs",
        });
        table.push(MemoryRow {
            name: rows::QUERY_WORKSPACE,
            representation: "one serial traversal buffer set and one scratch",
            bytes: sum(&[traversal_bytes, self.scratch_bytes])?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerQuery,
            overlap: MemoryOverlap::Counted,
            note: "one query overlapping a running iteration; a second concurrent query needs the limit raised",
        });
        table.push(MemoryRow {
            name: rows::DECISION_REPORT,
            representation: "1326 x actions Option<f64> plus two f64 reach vectors",
            bytes: self.decision_bytes,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerQuery,
            overlap: MemoryOverlap::Counted,
            note: "",
        });
        table.push(MemoryRow {
            name: rows::NODE_REPORT,
            representation: "two 1326-entry Option<f64> vectors and four f64 vectors",
            bytes: self.node_bytes,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerQuery,
            overlap: MemoryOverlap::Counted,
            note: "counted beside the decision report: nothing stops a caller holding one of each",
        });
        table.push(MemoryRow {
            name: rows::CONSTRUCTION,
            representation: "NodeBuild array and buffers, interning maps, deal tables and path-validation workspace",
            bytes: self.construction_bytes,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::Construction,
            overlap: MemoryOverlap::CountedFreedEarly,
            note: "held beside the shared game, freed before a solver exists",
        });
        table.push(MemoryRow {
            name: rows::VERIFICATION,
            representation: "the full unmerged best-response walk over every runout, in f64",
            bytes: sum(&[
                product(traversal_bytes, self.workers)?,
                product(self.scratch_bytes, self.workers)?,
            ])?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerVerification,
            overlap: MemoryOverlap::Aliases(VERIFICATION_ALIASES),
            note: "the bytes are SolveSession::measurement: the worker traversal buffers and the solver's scratch; it reads the sums without retaining an average. PostflopStrategy::exploitability walks the same tree serially and takes the query workspace instead. Neither allocates anything else",
        });
        Ok(table)
    }

    /// Sum of the rows the bound counts under `plan`.
    ///
    /// Under [`PostflopMemory::plan`] this is [`Self::working_set_bound_bytes`],
    /// which the tests pin on both fixtures.
    pub fn bound_under(&self, plan: &StoragePlan) -> Result<usize, SolveError> {
        let mut total = 0_usize;
        for row in self.rows_under(plan)? {
            if row.overlap.is_counted() {
                total = total.checked_add(row.bytes).ok_or_else(overflow)?;
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_board_deals_forty_eight_rivers_and_a_flop_board_deals_both_streets() {
        let (states, outcomes) = board_states(Street::Turn, 4).unwrap();
        assert_eq!(states, [0, 1, 48]);
        assert_eq!(outcomes, [0, 48, 0]);

        let (states, outcomes) = board_states(Street::Flop, 3).unwrap();
        assert_eq!(states, [1, 49, 49 * 48]);
        assert_eq!(outcomes, [49, 48, 0]);

        let (states, outcomes) = board_states(Street::River, 5).unwrap();
        assert_eq!(states, [0, 0, 1]);
        assert_eq!(outcomes, [0, 0, 0]);
    }

    #[test]
    fn the_validation_bound_charges_every_buffer_that_walk_holds() {
        // One pending-node entry: a NodeId, a depth, two vector headers and
        // both players' 1326 live flags.
        let entry =
            size_of::<(crate::NodeId, usize, [Vec<bool>; 2])>() + 2 * STATES * size_of::<bool>();
        // A depth-5 tree whose widest menu is 3 actions: a chance node's 52
        // outcomes are the widest fan-out, so the stack peaks at (5 + 1) * 52
        // entries and its Vec doubles to twice that. Plus one visited flag per
        // node, the capped pair matrix, the opponent/output vectors, both
        // mask vectors, scoped indices, the terminal workspace and 1 KiB slack.
        assert_eq!(
            validation_bytes(1000, 5, 3).unwrap(),
            1000 + 2 * (6 * 52 * entry)
                + VALIDATION_PAIR_LIMIT * size_of::<f64>()
                + 2 * STATES * size_of::<f64>()
                + 2 * STATES * size_of::<u64>()
                + 4 * STATES * size_of::<usize>()
                + terminal_workspace_bytes().unwrap()
                + 1024
        );

        // A menu wider than the deck widens the stack instead of the deck: the
        // only difference is 64 - 52 more entries per level, doubled.
        assert_eq!(
            validation_bytes(1000, 5, 64).unwrap() - validation_bytes(1000, 5, 3).unwrap(),
            2 * 6 * (64 - 52) * entry
        );
    }

    #[test]
    fn a_board_that_cannot_be_dealt_is_named_rather_than_divided_by_zero() {
        let error = board_states(Street::Turn, 48).unwrap_err().to_string();
        assert!(error.contains("too few cards"), "{error}");
    }

    #[test]
    fn a_storage_plan_prices_each_range_separately_and_refuses_an_impossible_one() {
        let memory = PostflopMemory {
            board_states: 1,
            showdown_tables: 1,
            expanded_nodes: 4,
            expanded_decision_nodes: 2,
            action_slots: [10, 4],
            states: [STATES; 2],
            workers: 1,
            shared_bytes: 0,
            solver_bytes: 0,
            snapshot_bytes: 0,
            traversal_bytes: 0,
            scratch_bytes: 0,
            decision_bytes: 0,
            node_bytes: 0,
            construction_bytes: 0,
            working_set_bound_bytes: 0,
            parts: Parts::default(),
        };
        let plan = StoragePlan {
            precision: Precision::F32,
            states: [100, 50],
            snapshots: 0,
            store_current_policy: false,
        };
        assert_eq!(memory.entries_under(&plan).unwrap(), 10 * 100 + 4 * 50);
        assert_eq!(
            memory
                .entries_under(&StoragePlan::before_compaction())
                .unwrap(),
            14 * STATES
        );

        let error = memory
            .entries_under(&StoragePlan {
                states: [STATES + 1, STATES],
                ..plan
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("private states"), "{error}");
    }
}
