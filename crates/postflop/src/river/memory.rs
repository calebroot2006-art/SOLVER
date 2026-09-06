use crate::{
    Cfr, SolveError, Strategy,
    game::{Node, TraversalLayout},
    terminal::ShowdownScratch,
};
use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tree::{RiverNodeKind, RiverTree};

/// Conservative allocations for one river game and its checked operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiverMemory {
    /// Retained tree, input ranges, traversal metadata, rank table and static evaluator data.
    pub shared_bytes: usize,
    /// CFR current policy, regrets, averaging buffers and their metadata.
    pub solver_bytes: usize,
    /// One retained average or imported strategy.
    pub snapshot_bytes: usize,
    /// Maximum temporary recursive traversal buffers, including vector headers.
    pub traversal_bytes: usize,
    /// One checked terminal evaluation workspace.
    pub scratch_bytes: usize,
    /// One returned decision-value report and its combo reach vectors.
    pub decision_bytes: usize,
    /// Shared game, one solver, two snapshots, two workspaces, traversal and decision report.
    pub working_set_bound_bytes: usize,
}

fn sum(values: &[usize]) -> Result<usize, SolveError> {
    values
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(*value))
        .ok_or_else(|| SolveError::Allocation("river memory estimate overflow".into()))
}

fn product(a: usize, b: usize) -> Result<usize, SolveError> {
    a.checked_mul(b)
        .ok_or_else(|| SolveError::Allocation("river memory estimate overflow".into()))
}

impl RiverMemory {
    pub(super) fn estimate(tree: &RiverTree) -> Result<Self, SolveError> {
        let mut entries = 0;
        let mut max_actions = 0;
        let mut edges = 0;
        for node in tree.nodes() {
            edges = sum(&[edges, node.children().len()])?;
            if matches!(node.kind(), RiverNodeKind::Decision { .. }) {
                entries = sum(&[entries, product(1326, node.actions().len())?])?;
                max_actions = max_actions.max(node.actions().len());
            }
        }
        let rows = product(entries, size_of::<f64>())?;
        let headers = product(tree.nodes().len(), size_of::<Vec<f64>>())?;
        let snapshot_bytes = sum(&[size_of::<Strategy>(), rows, headers, 256])?;
        let solver_bytes = sum(&[
            size_of::<Cfr>(),
            product(rows, 3)?,
            product(headers, 3)?,
            1024,
        ])?;
        // Rank groups use at most 2048 entries of two usize values, plus 1081
        // ranked combos. 64 KiB bounds the current checked ShowdownTable layout.
        let shared_bytes = sum(&[
            tree.storage_bytes(),
            65_536,
            312_320,
            4096,
            size_of::<TraversalLayout>(),
            product(4 * 1326, size_of::<f64>())?,
            product(tree.nodes().len(), size_of::<Node>() + 96)?,
            product(edges, size_of::<u32>())?,
        ])?;
        let traversal_bytes = product(
            tree.max_depth() + 2,
            product(max_actions + 8, 1326 * size_of::<f64>() + 128)?,
        )?;
        let decision_bytes = sum(&[
            product(1326 * max_actions, size_of::<Option<f64>>())?,
            2 * 1326 * size_of::<f64>(),
            512,
        ])?;
        let scratch_bytes = size_of::<ShowdownScratch>() + 128;
        let working_set_bound_bytes = sum(&[
            shared_bytes,
            solver_bytes,
            product(snapshot_bytes, 2)?,
            product(scratch_bytes, 2)?,
            traversal_bytes,
            decision_bytes,
        ])?;
        Ok(Self {
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

#[derive(Debug)]
pub(super) struct Budget {
    limit: usize,
    used: AtomicUsize,
}

impl Budget {
    pub fn new(limit: usize, shared: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicUsize::new(shared),
        })
    }

    pub fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }

    pub fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Lease, SolveError> {
        let mut used = self.used();
        loop {
            let required = used
                .checked_add(bytes)
                .ok_or_else(|| SolveError::Allocation("river reservation overflow".into()))?;
            if required > self.limit {
                return Err(SolveError::MemoryLimit {
                    required,
                    limit: self.limit,
                });
            }
            match self.used.compare_exchange_weak(
                used,
                required,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(Lease {
                        budget: self.clone(),
                        bytes,
                    });
                }
                Err(actual) => used = actual,
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct Lease {
    budget: Arc<Budget>,
    bytes: usize,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
