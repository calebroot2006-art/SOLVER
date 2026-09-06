//! Private terminal boundary shared by audited callbacks and owned river games.

use crate::{Game, NodeId, Real, SolveError};

pub(crate) trait TerminalEvaluator {
    fn evaluate_terminal(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError>;
}

pub(crate) struct LegacyTerminal<'a>(pub &'a dyn Game);

impl TerminalEvaluator for LegacyTerminal<'_> {
    fn evaluate_terminal(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        _iteration: u64,
    ) -> Result<(), SolveError> {
        // Traversals initialize output to NaN and check every entry after this
        // call, preserving the callback contract and error context.
        self.0.terminal_values(node, player, opponent, output);
        Ok(())
    }
}
