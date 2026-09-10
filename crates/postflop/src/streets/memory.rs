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
//! These are allocations this crate makes under its own API. They are not
//! process resident set size.

use super::{DECK, PRIVATE_CARDS, STATES, VALIDATION_PAIR_LIMIT};
use crate::{
    Cfr, SolveError, Strategy,
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
    /// Sum of action counts over the street's decision nodes.
    action_slots: [usize; 3],
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
            PostflopNodeKind::Decision { .. } => {
                let actions = node.actions().len();
                totals.action_slots[street] = totals.action_slots[street]
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
        let mut action_slots = 0;
        let mut chance_outcomes = 0;
        let mut board_state_total = 0;
        for street in [Street::Flop, Street::Turn, Street::River] {
            let index = street.index();
            expanded_nodes = sum(&[expanded_nodes, product(totals.nodes[index], states[index])?])?;
            action_slots = sum(&[
                action_slots,
                product(totals.action_slots[index], states[index])?,
            ])?;
            chance_outcomes = sum(&[
                chance_outcomes,
                product(
                    product(totals.chance[index], states[index])?,
                    outcomes[index],
                )?,
            ])?;
            board_state_total = sum(&[board_state_total, states[index]])?;
        }
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
        let snapshot_bytes = sum(&[size_of::<Strategy>(), rows, headers, 256])?;
        let solver_bytes = sum(&[
            size_of::<Cfr>(),
            product(rows, 3)?,
            product(headers, 3)?,
            1024,
        ])?;
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
        let shared_bytes = sum(&[
            tree.storage_bytes(),
            product(showdown_tables, SHOWDOWN_TABLE_BYTES)?,
            mask_pool_bytes,
            312_320,
            4096,
            size_of::<TraversalLayout>(),
            product(4 * STATES, size_of::<f64>())?,
            product(expanded_nodes, size_of::<Node>() + 96)?,
            product(edges, size_of::<u32>())?,
            // Chance probabilities and mask-pool indices, one pair per outcome.
            product(chance_outcomes, size_of::<f64>() + size_of::<usize>())?,
            // Per-node board index, payoff and parent link, and one runout
            // range per dealt card.
            product(expanded_nodes, 96)?,
            product(chance_outcomes, 2 * size_of::<crate::NodeId>())?,
            // Board metadata: five cards, a card set, a street and the deck of
            // cards still to come, with room for the vector headers.
            product(board_state_total, DECK + 128)?,
        ])?;
        // A serial walk holds one value vector per action at a decision node and
        // one at a time at a chance node. Above one worker a chance node instead
        // collects every outcome's vector before it reduces them in outcome
        // order, so the widest level is the widest deal rather than the widest
        // bet menu. One worker keeps the serial width, and every number a serial
        // solve has already recorded with it.
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
            shared_bytes,
            solver_bytes,
            snapshot_bytes,
            traversal_bytes,
            scratch_bytes,
            decision_bytes,
            construction_bytes,
            working_set_bound_bytes,
        })
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
}
