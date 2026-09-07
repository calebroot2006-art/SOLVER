//! The terminal boundary for a street-aware postflop game.
//!
//! Every expanded node carries the payoff its compact node implies, and the
//! board it was expanded onto supplies the dead cards and, on a complete board,
//! the showdown table. A fold before the river is evaluated against a four-card
//! dead set; a showdown is only ever reached on a five-card board, because a
//! called all-in is expanded as chance nodes down to the river.

use super::game::Inner;
use crate::{
    NodeId, Real, SolveError,
    terminal::{OutcomeUtilities, ShowdownScratch, ShowdownTable, evaluate_fold},
};
use cards::CardSet;

/// Private states per player, one per unordered two-card combination.
const STATES: usize = 1326;

/// What one expanded node pays, decided once at construction.
#[derive(Clone, Copy)]
pub(super) enum Payoff {
    /// A decision node pays nothing itself.
    Decision,
    /// A chance node pays nothing itself.
    Chance,
    /// Net chips to player zero when the hand is folded out.
    Fold(f64),
    /// Win, tie and loss utilities for each player at a showdown.
    Showdown([OutcomeUtilities; 2]),
}

/// One node's payoff and everything its board contributes to evaluating it.
pub(super) struct TerminalContext<'a> {
    /// What the node pays.
    pub payoff: &'a Payoff,
    /// Board cards known at the node, which no private hand may hold.
    pub dead: CardSet,
    /// Ranked combos for a complete board; absent before the river.
    pub table: Option<&'a ShowdownTable>,
}

/// Evaluates postflop terminals against the board their node was expanded onto.
pub(super) struct PostflopTerminal<'a> {
    pub game: &'a Inner,
    pub scratch: &'a mut ShowdownScratch,
}

impl crate::traversal::TerminalEvaluator for PostflopTerminal<'_> {
    fn checks_reach_underflow(&self) -> bool {
        true
    }

    fn evaluate_terminal(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError> {
        let fail = |reason: String| SolveError::Terminal {
            iteration,
            node,
            player,
            reason,
        };
        let opponent: &[f64; STATES] = opponent
            .try_into()
            .map_err(|_| fail("invalid opponent vector length".into()))?;
        let output: &mut [f64; STATES] = output
            .try_into()
            .map_err(|_| fail("invalid output vector length".into()))?;
        if player > 1 {
            return Err(fail("invalid player".into()));
        }
        let context = self
            .game
            .payoff(node)
            .ok_or_else(|| fail("node is outside the expanded tree".into()))?;
        match context.payoff {
            Payoff::Fold(value) => evaluate_fold(
                context.dead,
                opponent,
                if player == 0 { *value } else { -*value },
                output,
            ),
            Payoff::Showdown(utilities) => {
                let table = context
                    .table
                    .ok_or_else(|| fail("showdown node has no complete board".into()))?;
                table.evaluate(opponent, utilities[player], output, self.scratch)
            }
            Payoff::Decision | Payoff::Chance => {
                return Err(fail("node is not a postflop terminal".into()));
            }
        }
        .map_err(|e| fail(e.to_string()))
    }
}
