//! Owned street-aware postflop games, their CFR sessions and their policies.
//!
//! The compact [`tree::PostflopTree`] keeps one abstract chance node per street
//! transition. [`PostflopGame`] expands it into a public tree where every dealt
//! card has its own subtree, which is what the traversal contract in
//! `crate::game` requires: distinct public histories use distinct nodes. The
//! chance probability is one over the unseen cards less the four private cards,
//! and each outcome's masks zero the combos that hold the dealt card, so the
//! chance mass over any compatible pair is one.
//!
//! The payoff origin assigns half the root pot to each player, exactly as the
//! river module does. A river-start game reproduces `RiverGame` node for node
//! and bit for bit, which is the load-bearing check that expansion and terminal
//! mapping are right.
//!
//! Expansion is depth first, so the nodes below any node form one contiguous
//! half-open range. [`PostflopGame::subtree`] reports it and
//! [`PostflopGame::outcome_range`] reports one outcome's slice of a chance
//! node's range: pairwise disjoint, in outcome order, at every chance level.

mod game;
mod memory;
mod solver;
mod strategy;
mod terminal;

pub use game::{PostflopGame, PostflopNodeView, PostflopOptions, PostflopValidation};
pub use memory::{
    MemoryLifetime, MemoryOverlap, MemoryReservation, MemoryRow, PostflopMemory, StoragePlan,
    bytes_per_entry, rows,
};
pub use solver::PostflopSolver;
pub use strategy::{PostflopDecisionValues, PostflopStrategy};

/// Private states per player, one per unordered two-card combination.
pub(crate) const STATES: usize = 1326;
/// Cards held by the two players, which a runout can never repeat.
pub(crate) const PRIVATE_CARDS: usize = 4;
/// Cards in a standard deck.
pub(crate) const DECK: usize = 52;

/// Largest scoped private-state pair count a construction-time path validation
/// will walk: 512 live combos per player. Above it the walk is skipped, because
/// a chance node's mass check is one pass over every pair.
pub(crate) const VALIDATION_PAIR_LIMIT: usize = 512 * 512;
/// Largest expanded tree a construction-time path validation will walk.
pub(crate) const VALIDATION_NODE_LIMIT: usize = 200_000;
/// Largest number of terminal utility columns the zero-sum half of that
/// validation will read: one column per live state per terminal, each one a
/// full terminal evaluation.
pub(crate) const VALIDATION_COLUMN_LIMIT: usize = 100_000;

/// Traversal workers a requested thread count resolves to.
///
/// Zero asks for one per available core, which is what `config/solver.toml` and
/// [`crate::SolveConfig::threads`] promise. This is the only place that question
/// is answered, so the memory estimate charges for exactly the workspaces the
/// solver allocates. A platform that will not report its parallelism gives one.
pub(crate) fn resolve_workers(threads: usize) -> usize {
    if threads > 0 {
        return threads;
    }
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

#[cfg(test)]
mod tests {
    use super::resolve_workers;

    #[test]
    fn zero_threads_resolves_to_the_available_cores_and_any_other_value_is_itself() {
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        assert_eq!(resolve_workers(0), cores);
        assert!(resolve_workers(0) >= 1);
        for threads in [1, 2, 3, 64] {
            assert_eq!(resolve_workers(threads), threads);
        }
    }
}
