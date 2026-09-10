//! The terminal boundary for a street-aware postflop game.
//!
//! Every expanded node carries the payoff its compact node implies, and the
//! board it was expanded onto supplies the dead cards and, on a complete board,
//! the showdown table. A fold before the river is evaluated against a four-card
//! dead set; a showdown is only ever reached on a five-card board, because a
//! called all-in is expanded as chance nodes down to the river.

use super::STATES;
use super::game::Inner;
use crate::{
    NodeId, Real, SolveError,
    error::finite,
    game::TerminalColumns,
    terminal::{OutcomeUtilities, ShowdownScratch, ShowdownTable, evaluate_fold},
    traversal::TerminalEvaluator,
};
use cards::CardSet;

/// What one expanded node pays, decided once at construction.
///
/// These are interned: a tree pays only a handful of distinct amounts, and the
/// node record holds an index rather than the record itself.
#[derive(Clone, Copy)]
pub(super) enum Payoff {
    /// A decision node pays nothing itself.
    Decision,
    /// A chance node pays nothing itself.
    Chance,
    /// Net chips to player zero when the hand is folded out.
    Fold(f64),
    /// Win, tie and loss utilities at a showdown. One record covers both
    /// players: whoever is asking wins the same amount and loses the same
    /// amount, because a showdown pays out of one pot.
    Showdown(OutcomeUtilities),
}

/// One worker's terminal workspace.
///
/// The walk carries only the live combos, and `ShowdownTable` and
/// `evaluate_fold` are written against all 1326 combo IDs, so every terminal
/// evaluation scatters the compacted opponent reach into `opponent`, evaluates
/// into `output`, and gathers the live entries back. Both buffers are cleared
/// or completely overwritten before they are read, so nothing carries between
/// evaluations and nothing depends on which worker used the slot last.
pub(super) struct TerminalWorkspace {
    pub scratch: ShowdownScratch,
    opponent: Box<[f64; STATES]>,
    output: Box<[f64; STATES]>,
}

impl Default for TerminalWorkspace {
    fn default() -> Self {
        Self {
            scratch: ShowdownScratch::default(),
            opponent: Box::new([0.0; STATES]),
            output: Box::new([0.0; STATES]),
        }
    }
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
    pub workspace: &'a mut TerminalWorkspace,
}

/// Reads one expanded terminal's whole utility column for the construction-time
/// zero-sum check, by evaluating it against a one-hot opponent reach.
///
/// The read follows the same poisoning discipline as the CFR walk in
/// `crate::cfr`: the output is filled with NaN first, so an evaluator that
/// leaves an entry unwritten cannot pass a stale or zero value off as a
/// utility, and every entry is checked finite before the caller compares it.
pub(super) struct PostflopColumns<'a> {
    pub terminal: PostflopTerminal<'a>,
    /// Reusable one-hot opponent reach, zero again after every column.
    pub opponent: Vec<Real>,
}

impl TerminalColumns for PostflopColumns<'_> {
    fn column(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: usize,
        out: &mut [Real],
    ) -> Result<(), SolveError> {
        let states = self.terminal.game.layout.states;
        if self.opponent.len() < states[1 - player]
            || out.len() != states[player]
            || opponent >= states[1 - player]
        {
            return Err(SolveError::InvalidGame(
                "a postflop terminal column is one entry per live combo wide".into(),
            ));
        }
        let opponent_reach = &mut self.opponent[..states[1 - player]];
        // Prefill, evaluate, then check: an unwritten entry stays NaN and is
        // named here rather than reaching the zero-sum comparison. Iteration
        // zero is this crate's marker for an independent measurement.
        out.fill(Real::NAN);
        opponent_reach[opponent] = 1.0;
        let result = self.terminal.evaluate_terminal(
            node,
            player,
            &self.opponent[..states[1 - player]],
            out,
            0,
        );
        self.opponent[opponent] = 0.0;
        result?;
        finite(out, 0, node, player)
    }
}

impl TerminalEvaluator for PostflopTerminal<'_> {
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
        if player > 1 {
            return Err(fail("invalid player".into()));
        }
        let game = self.game;
        if opponent.len() != game.layout.states[1 - player] {
            return Err(fail("invalid opponent vector length".into()));
        }
        if output.len() != game.layout.states[player] {
            return Err(fail("invalid output vector length".into()));
        }
        // Scatter the compacted opponent reach over all 1326 combo IDs. The
        // entries this leaves at zero are the combos this game never deals, and
        // a zero is exactly the reach they carried before compaction, so the
        // evaluator below sums the same terms in the same order it always did.
        let workspace = &mut *self.workspace;
        let wide_opponent = &mut *workspace.opponent;
        wide_opponent.fill(0.0);
        for (slot, id) in opponent.iter().zip(&game.live[1 - player]) {
            wide_opponent[usize::from(*id)] = *slot;
        }
        let wide_output = &mut *workspace.output;
        let context = game
            .payoff(node)
            .ok_or_else(|| fail("node is outside the expanded tree".into()))?;
        match context.payoff {
            Payoff::Fold(value) => evaluate_fold(
                context.dead,
                wide_opponent,
                if player == 0 { *value } else { -*value },
                wide_output,
            ),
            Payoff::Showdown(utilities) => {
                let table = context
                    .table
                    .ok_or_else(|| fail("showdown node has no complete board".into()))?;
                table.evaluate(
                    wide_opponent,
                    *utilities,
                    wide_output,
                    &mut workspace.scratch,
                )
            }
            Payoff::Decision | Payoff::Chance => {
                return Err(fail("node is not a postflop terminal".into()));
            }
        }
        .map_err(|e| fail(e.to_string()))?;
        // Gather this player's live combos back out. A combo the runout blocks
        // reads the zero the evaluator left for it, which the walk's own live
        // mask would have multiplied it by anyway.
        for (slot, id) in output.iter_mut().zip(&game.live[player]) {
            *slot = wide_output[usize::from(*id)];
        }
        Ok(())
    }
}
