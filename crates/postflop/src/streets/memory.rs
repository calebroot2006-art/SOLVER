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
//! module's callers make, which rows that reservation draws from. A
//! [`StoragePlan`] other than [`StoragePlan::today`] is arithmetic over the same
//! entry counts, not a measurement: `f32` (step 7), `i16` (step 10) and in-range
//! compaction (step 6) of `docs/phase-4/PLAN.md` are not implemented.
//!
//! These are allocations this crate makes under its own API. They are not
//! process resident set size.

use super::{DECK, PRIVATE_CARDS, STATES, VALIDATION_PAIR_LIMIT};
use crate::{
    Cfr, Precision, SolveError, Strategy,
    game::{Node, TraversalLayout},
    terminal::ShowdownScratch,
};
use std::mem::size_of;
use tree::{PostflopNodeKind, PostflopTree, Street};

/// Upper bound on the chance-mask pool: one entry per card that can be dealt.
const MASK_POOL_ENTRIES: usize = DECK;
/// Bound on one checked `ShowdownTable`, the same bound the river game asserts.
const SHOWDOWN_TABLE_BYTES: usize = 65_536;
/// Bytes charged per interned key in a construction-time `HashMap`. The maps
/// hold a `u64` or a `u8` against a `usize`; 32 bytes covers the entry, the
/// control byte and the table's spare capacity at its 87.5% load factor.
const MAP_ENTRY_BYTES: usize = 32;

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
    /// The current policy, stored today and derived at visit time after step 6.
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
    Aliases(&'static str),
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
/// [`StoragePlan::today`] is what the code does now, and its bound is exactly
/// [`PostflopMemory::working_set_bound_bytes`]. Every other plan is arithmetic
/// over the same entry counts: no `f32`, `i16` or compacted game exists yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoragePlan {
    /// Width of one stored state-action entry.
    pub precision: Precision,
    /// Private states charged per player: 1326 today, and the combos with
    /// positive weight left by the board prefix after step 6's compaction.
    pub states: [usize; 2],
    /// Average strategies retained at once. Two today; the design target during
    /// a solve is zero, with at most one compact snapshot for browsing.
    pub snapshots: usize,
    /// Whether the current policy is stored beside the regrets, as it is today,
    /// or derived at visit time as step 6 intends.
    pub store_current_policy: bool,
}

impl StoragePlan {
    /// What the implemented code stores today.
    #[must_use]
    pub fn today() -> Self {
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
}

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
/// draws from. Every reservation site is named here, so a row that no
/// reservation reaches, or a reservation that no row explains, is a test
/// failure rather than a discrepancy someone notices later.
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
}

impl MemoryReservation {
    /// Every reservation site, for a test that wants to cover all of them.
    pub const ALL: [Self; 6] = [
        Self::Shared,
        Self::Solver,
        Self::Iteration,
        Self::Snapshot,
        Self::Query,
        Self::DecisionReport,
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
                rows::CURRENT_POLICY,
                rows::CFR_BOOKKEEPING,
                rows::SCALES,
                rows::SCRATCH,
            ],
            Self::Iteration => &[rows::TRAVERSAL],
            Self::Snapshot => &[rows::SNAPSHOTS],
            Self::Query => &[rows::QUERY_WORKSPACE],
            Self::DecisionReport => &[rows::DECISION_REPORT],
        }
    }

    /// How many of these the bound charges at once.
    ///
    /// Two snapshots, because a caller can hold a second average while the
    /// first is still alive, and one of everything else. The design target
    /// after step 6 is zero snapshots retained during a solve and at most one
    /// compact snapshot for browsing; until then the bound charges two.
    #[must_use]
    pub fn charged(self) -> usize {
        match self {
            Self::Snapshot => 2,
            _ => 1,
        }
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
    /// One `Vec` header per expanded node, in one array.
    headers_bytes: usize,
    /// `Cfr` itself and the estimate's slack, outside the three arrays.
    cfr_overhead_bytes: usize,
    /// `Strategy` itself and the estimate's slack, per snapshot.
    snapshot_overhead_bytes: usize,
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
    /// Traversal workers this estimate charged for, from `resolve_workers`.
    pub workers: usize,
    /// Retained tree, ranges, traversal metadata, showdown tables and mask pool.
    pub shared_bytes: usize,
    /// CFR current policy, regrets, averaging buffers and their metadata.
    pub solver_bytes: usize,
    /// One retained average or imported strategy.
    pub snapshot_bytes: usize,
    /// Maximum temporary recursive traversal buffers, per worker.
    pub traversal_bytes: usize,
    /// One checked terminal evaluation workspace, per worker.
    pub scratch_bytes: usize,
    /// One returned decision-value report and its combo reach vectors.
    pub decision_bytes: usize,
    /// Temporary buffers construction holds and frees before the solve: the
    /// interning maps, the per-board deal table, and the path validation's
    /// visited flags, pending-node stack, showdown scratch and the pair matrix
    /// its zero-sum pass cannot stream away.
    pub construction_bytes: usize,
    /// Shared game, one solver, two snapshots, per-worker workspaces, one report
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
                let side = player as usize;
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
/// source's own showdown scratch. Charging three full-width `f64` vectors is a
/// bound above those, not a count of them. They are built before the walk
/// decides whether the pair checks fit, so they are charged either way.
fn validation_bytes(
    expanded_nodes: usize,
    max_depth: usize,
    max_actions: usize,
) -> Result<usize, SolveError> {
    let entry = sum(&[
        size_of::<crate::NodeId>(),
        size_of::<usize>(),
        product(2, size_of::<Vec<bool>>())?,
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
        product(3 * STATES, size_of::<f64>())?,
        size_of::<ShowdownScratch>(),
        1024,
    ])
}

impl PostflopMemory {
    /// Bounds every buffer the game, one solver, two snapshots and one report
    /// can hold, for a tree expanded over `board_len` known board cards and run
    /// by `workers` traversal workers.
    pub(super) fn estimate(
        tree: &PostflopTree,
        board_len: usize,
        workers: usize,
    ) -> Result<Self, SolveError> {
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
        let showdown_tables = states[Street::River.index()];
        let edges = sum(&[action_slots, chance_outcomes])?;

        let rows = product(product(action_slots, STATES)?, size_of::<f64>())?;
        let headers = product(expanded_nodes, size_of::<Vec<f64>>())?;
        let snapshot_overhead = size_of::<Strategy>() + 256;
        let snapshot_bytes = sum(&[snapshot_overhead, rows, headers])?;
        let cfr_overhead = size_of::<Cfr>() + 1024;
        let solver_bytes = sum(&[cfr_overhead, product(rows, 3)?, product(headers, 3)?])?;
        // Both players' masks for one dealt card, plus the two vector headers.
        let mask_pool_bytes = product(
            MASK_POOL_ENTRIES,
            sum(&[
                product(2 * STATES, size_of::<f64>())?,
                2 * size_of::<Vec<f64>>(),
            ])?,
        )?;
        // Rank groups use at most 2048 entries of two usize values, plus 1081
        // ranked combos, matching the river estimate's fixed evaluator terms.
        let range_bytes = sum(&[
            312_320,
            4096,
            size_of::<TraversalLayout>(),
            product(4 * STATES, size_of::<f64>())?,
        ])?;
        let topology_bytes = sum(&[
            product(expanded_nodes, size_of::<Node>() + 96)?,
            product(edges, size_of::<u32>())?,
            // Per-node board index, payoff and parent link, and one runout
            // range per dealt card.
            product(expanded_nodes, 96)?,
            product(chance_outcomes, 2 * size_of::<crate::NodeId>())?,
        ])?;
        // Chance probabilities and mask-pool indices, one pair per outcome.
        let chance_bytes = product(chance_outcomes, size_of::<f64>() + size_of::<usize>())?;
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
        let widest = if workers > 1 {
            totals
                .max_actions
                .max(outcomes.into_iter().max().unwrap_or(0))
        } else {
            totals.max_actions
        };
        let traversal_bytes = product(
            tree.max_depth() + 2,
            product(widest + 8, STATES * size_of::<f64>() + 128)?,
        )?;
        let decision_bytes = sum(&[
            product(
                product(STATES, totals.max_actions)?,
                size_of::<Option<f64>>(),
            )?,
            2 * STATES * size_of::<f64>(),
            512,
        ])?;
        let scratch_bytes = size_of::<ShowdownScratch>() + 128;
        // Construction transients, freed before the solver exists but held at
        // the same time as everything in `shared_bytes`, so the refusal has to
        // cover them. Per board: one 52-entry child table of card indices and
        // its vector header. Per complete board: one entry in the map that
        // interns showdown tables on the card set. Per dealt card: one entry in
        // the map that interns mask-pool indices.
        let construction_bytes = sum(&[
            product(
                board_state_total,
                sum(&[product(DECK, size_of::<u32>())?, size_of::<Vec<u32>>()])?,
            )?,
            product(showdown_tables, MAP_ENTRY_BYTES)?,
            product(MASK_POOL_ENTRIES, MAP_ENTRY_BYTES)?,
            validation_bytes(expanded_nodes, tree.max_depth(), totals.max_actions)?,
        ])?;
        let working_set_bound_bytes = sum(&[
            shared_bytes,
            solver_bytes,
            product(snapshot_bytes, 2)?,
            // A strategy query can run while an iteration holds its own
            // workspaces: one traversal buffer and one scratch per worker for
            // the iteration, plus one of each for the query.
            product(scratch_bytes, sum(&[workers, 1])?)?,
            product(traversal_bytes, sum(&[workers, 1])?)?,
            decision_bytes,
            construction_bytes,
        ])?;
        Ok(Self {
            board_states: board_state_total,
            showdown_tables,
            expanded_nodes,
            expanded_decision_nodes: expanded_decisions,
            action_slots: slots,
            workers,
            shared_bytes,
            solver_bytes,
            snapshot_bytes,
            traversal_bytes,
            scratch_bytes,
            decision_bytes,
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
                headers_bytes: headers,
                cfr_overhead_bytes: cfr_overhead,
                snapshot_overhead_bytes: snapshot_overhead,
            },
        })
    }

    /// The same estimate for a tree no game has been built from.
    ///
    /// `PostflopGame::new` refuses a tree whose bound is above the configured
    /// limit, which is exactly the case a memory table has to report on, so the
    /// arithmetic is reachable without a game. `board_len` is the known board
    /// the tree would start from: three cards on the flop, four on the turn,
    /// five on the river.
    pub fn for_tree(
        tree: &PostflopTree,
        board_len: usize,
        workers: usize,
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
        Self::estimate(tree, board_len, workers)
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
        self.rows_under(&StoragePlan::today())
    }

    /// One row per buffer under `plan`.
    ///
    /// Only the stored entry arrays respond to the plan. The traversal buffers,
    /// the decision report and the terminal boundary stay full-width `f64`:
    /// step 6 scatters and gathers at that boundary so `ShowdownTable` still
    /// sees all 1326 states, and the reference solver keeps `f64` summation
    /// temporaries for the same reason.
    pub fn rows_under(&self, plan: &StoragePlan) -> Result<Vec<MemoryRow>, SolveError> {
        let width = bytes_per_entry(plan.precision);
        let entries = self.entries_under(plan)?;
        let array = sum(&[product(entries, width)?, self.parts.headers_bytes])?;
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
        /// What every stored entry array holds, whatever its width.
        const STORED_ARRAY: &str = "one entry per state-action, one Vec header per node";
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
                representation: "Node records, u32 child edges, per-node board, payoff and parent links, runout ranges",
                bytes: self.parts.topology_bytes,
                entries: 0,
                arrays: 0,
                lifetime: MemoryLifetime::WholeSolve,
                overlap: MemoryOverlap::Counted,
                note: "step 6 flattens this to struct-of-arrays with u64 offsets",
            },
            MemoryRow {
                name: rows::CHANCE,
                representation: "one f64 probability and one usize mask index per outcome",
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
                note: "charged per ordered runout; the build interns them on the board's card set",
            },
            MemoryRow {
                name: rows::MASK_POOL,
                representation: "both players' f64 blocker masks over 1326 states, per dealt card",
                bytes: self.parts.mask_pool_bytes,
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
                note: "after step 6 the best-response walk normalises these per node as it reads them",
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
                note: "stored today; step 6 derives it from the regrets at visit time and drops this row",
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
            representation: "retained average strategies, same width and shape as one stored array",
            bytes: snapshots,
            entries: product(entries, plan.snapshots)?,
            arrays: plan.snapshots,
            lifetime: MemoryLifetime::PerQuery,
            overlap: MemoryOverlap::Counted,
            note: "taken at an iteration boundary by average_strategy, freed when the PostflopStrategy drops",
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
            representation: "f64 value vectors over 1326 states, one set per worker",
            bytes: product(self.traversal_bytes, self.workers)?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerIteration,
            overlap: MemoryOverlap::Counted,
            note: "above one worker a chance node gathers every outcome, so the width is the widest deal",
        });
        table.push(MemoryRow {
            name: rows::SCRATCH,
            representation: "one checked ShowdownScratch per worker",
            bytes: product(self.scratch_bytes, self.workers)?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::WholeSolve,
            overlap: MemoryOverlap::Counted,
            note: "allocated by PostflopSolver::new and held for the life of the solver",
        });
        table.push(MemoryRow {
            name: rows::QUERY_WORKSPACE,
            representation: "one serial traversal buffer set and one scratch",
            bytes: sum(&[self.traversal_bytes, self.scratch_bytes])?,
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
            name: rows::CONSTRUCTION,
            representation: "interning maps, per-board deal tables, path-validation flags, stack, scratch and pair matrix",
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
                product(self.snapshot_bytes, 1)?,
                product(self.traversal_bytes, self.workers)?,
                product(self.scratch_bytes, self.workers)?,
            ])?,
            entries: 0,
            arrays: 0,
            lifetime: MemoryLifetime::PerVerification,
            overlap: MemoryOverlap::Aliases(
                "average-strategy snapshots, traversal value buffers, terminal showdown scratch",
            ),
            note: "measurement takes one snapshot and the iteration workspaces; it allocates nothing else",
        });
        Ok(table)
    }

    /// Sum of the rows the bound counts under `plan`.
    ///
    /// Under [`StoragePlan::today`] this is [`Self::working_set_bound_bytes`],
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
        let entry = size_of::<crate::NodeId>()
            + size_of::<usize>()
            + 2 * size_of::<Vec<bool>>()
            + 2 * STATES * size_of::<bool>();
        // A depth-5 tree whose widest menu is 3 actions: a chance node's 52
        // outcomes are the widest fan-out, so the stack peaks at (5 + 1) * 52
        // entries and its Vec doubles to twice that. Plus one visited flag per
        // node, the capped pair matrix, three full-width f64 vectors, the
        // showdown scratch and 1 KiB of slack.
        assert_eq!(
            validation_bytes(1000, 5, 3).unwrap(),
            1000 + 2 * (6 * 52 * entry)
                + VALIDATION_PAIR_LIMIT * size_of::<f64>()
                + 3 * STATES * size_of::<f64>()
                + size_of::<ShowdownScratch>()
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
            workers: 1,
            shared_bytes: 0,
            solver_bytes: 0,
            snapshot_bytes: 0,
            traversal_bytes: 0,
            scratch_bytes: 0,
            decision_bytes: 0,
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
            memory.entries_under(&StoragePlan::today()).unwrap(),
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
