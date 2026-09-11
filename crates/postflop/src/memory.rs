//! Shared working-set accounting for every owned game in this crate.
//!
//! One `Budget` per game holds the configured limit and the bytes currently
//! reserved. Solvers, snapshots and query workspaces take a `Lease` before they
//! allocate; dropping the lease returns the bytes. The counter is atomic
//! because concurrent strategy queries share one game. These are allocations
//! this crate makes under its own API, not process RSS.
use crate::{Cfr, NodeId, Real, SolveError};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
pub(crate) struct Budget {
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
                .ok_or_else(|| SolveError::Allocation("memory reservation overflow".into()))?;
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
pub(crate) struct Lease {
    budget: Arc<Budget>,
    bytes: usize,
}

/// A diagnostic current-policy row whose allocation stays charged while held.
///
/// Values are state-major, as in the solver's regret and strategy-sum rows.
/// This is a read-only diagnostic; an average strategy certifies convergence.
/// Dropping the row returns its reservation to the owning game's budget.
#[derive(Debug)]
pub struct CurrentPolicyRow {
    values: Vec<Real>,
    _lease: Lease,
}

impl CurrentPolicyRow {
    pub(crate) fn derive(
        core: &Cfr,
        budget: &Arc<Budget>,
        node: NodeId,
    ) -> Result<Option<Self>, SolveError> {
        core.health()?;
        let Some(source) = core.regrets(node) else {
            return Ok(None);
        };
        let bytes = |capacity: usize| {
            capacity
                .checked_mul(size_of::<Real>())
                .and_then(|payload| payload.checked_add(size_of::<Self>()))
                .ok_or_else(|| SolveError::Allocation("current-policy row size overflow".into()))
        };
        let planned = bytes(source.len())?;
        let mut lease = budget.reserve(planned)?;
        let Some(values) = core.current_row(node)? else {
            return Ok(None);
        };
        // Exact reservation is requested by current_row. Account for any
        // capacity an allocator supplies beyond that request before returning it.
        let actual = bytes(values.capacity())?;
        if actual > planned {
            let mut extra = budget.reserve(actual - planned)?;
            lease.bytes = actual;
            extra.bytes = 0;
        }
        Ok(Some(Self {
            values,
            _lease: lease,
        }))
    }

    /// The normalized state-major values; no owning buffer leaves this lease.
    #[must_use]
    pub fn values(&self) -> &[Real] {
        &self.values
    }
}

impl std::ops::Deref for CurrentPolicyRow {
    type Target = [Real];
    fn deref(&self) -> &Self::Target {
        self.values()
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
