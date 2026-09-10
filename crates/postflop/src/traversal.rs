//! Private terminal boundary shared by audited callbacks and owned river games,
//! and the shared boundary and node ranges a parallel walk splits along.

use crate::{Game, NodeId, Real, SolveError};

pub(crate) trait TerminalEvaluator {
    fn checks_reach_underflow(&self) -> bool {
        false
    }
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

/// A terminal boundary several traversal workers may call at the same time.
///
/// The serial walk holds one `&mut dyn TerminalEvaluator` over one workspace,
/// which is exactly what a parallel walk cannot do, so the shared boundary
/// takes `&self` and hands each call its own workspace. An implementation must
/// carry no order-dependent state: what it writes for a node must not depend on
/// which worker asked, or on what that worker evaluated before it.
pub(crate) trait SharedTerminal: Sync {
    fn checks_reach_underflow(&self) -> bool {
        false
    }
    fn evaluate_terminal(
        &self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError>;
}

/// One worker's [`TerminalEvaluator`] view of a shared boundary, so an outcome
/// task walks its runout through exactly the same code as a serial walk.
pub(crate) struct SharedRef<'a>(pub &'a dyn SharedTerminal);

impl TerminalEvaluator for SharedRef<'_> {
    fn checks_reach_underflow(&self) -> bool {
        self.0.checks_reach_underflow()
    }
    fn evaluate_terminal(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError> {
        self.0
            .evaluate_terminal(node, player, opponent, output, iteration)
    }
}

/// Half-open descendant ranges for a depth-first expanded tree.
///
/// `PostflopGame::subtree` is the implementation that matters: expansion is
/// depth first, so one chance outcome owns a contiguous block of node IDs and a
/// walk can hand that block's accumulators to one worker and no other.
pub(crate) trait SubtreeRanges: Sync {
    /// One past the last node expanded below `node`, or `None` when the tree
    /// does not know the node.
    fn end(&self, node: NodeId) -> Option<NodeId>;
}

/// Everything a walk needs to spread one chance node's outcomes over workers.
pub(crate) struct Parallel<'a> {
    /// The shared terminal boundary, one workspace per worker behind it.
    pub terminal: &'a dyn SharedTerminal,
    /// Where each outcome's nodes live, so accumulators can be split.
    pub ranges: &'a dyn SubtreeRanges,
    /// The pool every outcome task runs on. Its size and the memory estimate's
    /// worker count come from the same `streets::resolve_workers` answer.
    pub pool: &'a rayon::ThreadPool,
}

impl Parallel<'_> {
    /// One disjoint node range per chance outcome, in outcome order.
    ///
    /// `owned` is the node range the splitting walk holds, and `children` the
    /// chance node's outcomes in order. Each outcome takes
    /// `child..ranges.end(child)`, which is exactly the range
    /// `PostflopGame::outcome_range` reports. Ranges that are not increasing,
    /// run past the parent's own nodes, or name a node the tree does not have
    /// are refused rather than quietly overlapped: two workers sharing one
    /// regret row would make the walk's result depend on which of them finished
    /// first. The caller turns these ranges into slices of its own arrays,
    /// because a node's rows are contiguous exactly when its nodes are.
    pub fn split(
        &self,
        owned: std::ops::Range<NodeId>,
        children: &[NodeId],
    ) -> Result<Vec<(NodeId, NodeId)>, SolveError> {
        let mut parts = crate::allocation::reserved(children.len())?;
        let mut consumed = owned.start;
        for (outcome, child) in children.iter().copied().enumerate() {
            let refuse = |reason: &str| {
                SolveError::InvalidGame(format!(
                    "chance outcome {outcome} cannot own a private accumulator slice: {reason}"
                ))
            };
            let end = self
                .ranges
                .end(child)
                .ok_or_else(|| refuse("the tree does not know this node"))?;
            if child < consumed || end <= child {
                return Err(refuse("outcome ranges must be non-empty and increasing"));
            }
            if end > owned.end {
                return Err(refuse("the range leaves the parent's own nodes"));
            }
            parts.push((child, end));
            consumed = end;
        }
        Ok(parts)
    }
}
