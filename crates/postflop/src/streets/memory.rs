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
//! These are allocations this crate makes under its own API. They are not
//! process resident set size.

use crate::{
    Cfr, SolveError, Strategy,
    game::{Node, TraversalLayout},
    terminal::ShowdownScratch,
};
use std::mem::size_of;
use tree::{PostflopNodeKind, PostflopTree, Street};

/// Private states per player: every unordered two-card combination.
const STATES: usize = 1326;
/// Cards in a standard deck.
const DECK: usize = 52;
/// Cards held by the two players, which never appear in a runout.
const PRIVATE_CARDS: usize = 4;
/// Upper bound on the chance-mask pool: one entry per card that can be dealt.
const MASK_POOL_ENTRIES: usize = DECK;
/// Bound on one checked `ShowdownTable`, the same bound the river game asserts.
const SHOWDOWN_TABLE_BYTES: usize = 65_536;

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
    /// Shared game, one solver, two snapshots, per-worker workspaces and one report.
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
        let traversal_bytes = product(
            tree.max_depth() + 2,
            product(totals.max_actions + 8, STATES * size_of::<f64>() + 128)?,
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
        let working_set_bound_bytes = sum(&[
            shared_bytes,
            solver_bytes,
            product(snapshot_bytes, 2)?,
            product(scratch_bytes, product(2, workers)?)?,
            product(traversal_bytes, workers)?,
            decision_bytes,
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
    fn a_board_that_cannot_be_dealt_is_named_rather_than_divided_by_zero() {
        let error = board_states(Street::Turn, 48).unwrap_err().to_string();
        assert!(error.contains("too few cards"), "{error}");
    }
}
