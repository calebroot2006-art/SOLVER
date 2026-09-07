//! Shared working-set accounting for every owned game in this crate.
//!
//! One `Budget` per game holds the configured limit and the bytes currently
//! reserved. Solvers, snapshots and query workspaces take a `Lease` before they
//! allocate; dropping the lease returns the bytes. The counter is atomic
//! because concurrent strategy queries share one game. These are allocations
//! this crate makes under its own API, not process RSS.
use crate::SolveError;
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

impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
