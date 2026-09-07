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

mod game;
mod memory;
mod solver;
mod strategy;
mod terminal;

pub use game::{PostflopGame, PostflopNodeView, PostflopOptions};
pub use memory::PostflopMemory;
pub use solver::PostflopSolver;
pub use strategy::{PostflopDecisionValues, PostflopStrategy};
