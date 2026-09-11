//! Explicit shared resource accounting for bounded spot storage.

use std::mem::size_of;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// A named refusal, containing no allocated diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceError(pub &'static str);

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ResourceError {}

/// Caller-selected ceilings. Byte and nesting limits also gate future codecs;
/// a typed builder cannot attest to an encoded input it has never seen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLimits {
    /// Aggregate live allocations, including temporary overlap and housekeeping.
    pub live_bytes: u64,
    /// Aggregate retained allocations and housekeeping.
    pub retained_bytes: u64,
    /// Actual encoded bytes (codec gate); the builder checks minimum feasibility.
    pub encoded_bytes: u64,
    /// Actual encoded header bytes (codec gate); the builder checks a lower bound.
    pub header_bytes: u64,
    /// Encoded bytes per node, checked against the exact binary payload size.
    pub node_bytes: u64,
    /// Actual JSON nesting (codec gate).
    pub nesting: u64,
    /// Cumulative number of nodes.
    pub nodes: u64,
    /// Cumulative number of combos across nodes.
    pub combos: u64,
    /// Cumulative menus, actions, history actions and probability/EV pairs.
    pub action_entries: u64,
    /// Cumulative UTF-8 string bytes.
    pub string_bytes: u64,
}

impl ResourceLimits {
    pub(crate) fn validate(self) -> Result<(), ResourceError> {
        if self.retained_bytes > self.live_bytes
            || self.encoded_bytes > 4 * 1024 * 1024 * 1024
            || self.header_bytes > 1024 * 1024
            || self.node_bytes > 16 * 1024 * 1024
            || self.nesting > 32
            || self.nodes > 4_000_000
        {
            return Err(ResourceError("resource limit exceeds a schema ceiling"));
        }
        Ok(())
    }
}

/// Whether an external reservation is transient or survives as retained data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryClass {
    /// Charge both live and retained bytes.
    Retained,
    /// Charge live bytes only.
    Temporary,
}

/// Current shared charges and the greatest live reservation reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetSnapshot {
    /// Currently reserved live bytes.
    pub live_bytes: u64,
    /// Currently reserved retained bytes.
    pub retained_bytes: u64,
    /// Greatest live reservation, including allocation attempts.
    pub peak_live_bytes: u64,
}

/// Caller-owned aggregate budget. It allocates no heap memory and initially
/// charges its own inline size to both limits. Leases borrow it, so storage
/// cannot outlive its accounting. This accounts capacities, not allocator RSS.
#[derive(Debug)]
pub struct ResourceBudget {
    live_limit: u64,
    retained_limit: u64,
    counters: Mutex<BudgetSnapshot>,
    observed_live: AtomicU64,
}

impl ResourceBudget {
    /// Construct without defaults or a heap allocation.
    pub fn new(live_limit: u64, retained_limit: u64) -> Result<Self, ResourceError> {
        let baseline = size_of::<Self>() as u64;
        if retained_limit > live_limit || retained_limit < baseline {
            return Err(ResourceError("budget is smaller than its housekeeping"));
        }
        Ok(Self {
            live_limit,
            retained_limit,
            counters: Mutex::new(BudgetSnapshot {
                live_bytes: baseline,
                retained_bytes: baseline,
                peak_live_bytes: baseline,
            }),
            observed_live: AtomicU64::new(baseline),
        })
    }

    /// Read all counters consistently. This operation allocates no memory.
    pub fn snapshot(&self) -> BudgetSnapshot {
        *self.counters.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Lock-free live charge for allocator/drop observers. A concurrent reserve
    /// can make this value immediately stale; use `snapshot` for all counters.
    pub fn reserved_live_bytes(&self) -> u64 { self.observed_live.load(Ordering::Acquire) }

    /// Reserve caller-owned input, source snapshots, or other overlapping work.
    /// The caller must free that payload before dropping this lease.
    pub fn reserve_external(
        &self,
        bytes: u64,
        class: MemoryClass,
    ) -> Result<Lease<'_>, ResourceError> {
        self.reserve(
            bytes,
            if class == MemoryClass::Retained { bytes } else { 0 },
            self.live_limit,
            self.retained_limit,
        )
    }

    pub(crate) fn reserve(
        &self,
        live: u64,
        retained: u64,
        live_limit: u64,
        retained_limit: u64,
    ) -> Result<Lease<'_>, ResourceError> {
        let mut state = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        let next_live = add(state.live_bytes, live)?;
        let next_retained = add(state.retained_bytes, retained)?;
        if next_live > live_limit.min(self.live_limit) {
            return Err(ResourceError("live byte limit exceeded"));
        }
        if next_retained > retained_limit.min(self.retained_limit) {
            return Err(ResourceError("retained byte limit exceeded"));
        }
        state.live_bytes = next_live;
        state.retained_bytes = next_retained;
        state.peak_live_bytes = state.peak_live_bytes.max(next_live);
        self.observed_live.store(next_live, Ordering::Release);
        Ok(Lease { budget: self, live, retained })
    }
}

/// Non-cloneable reservation. External owners must put their payload before
/// this lease in declaration/drop order, including on failure paths.
#[derive(Debug)]
pub struct Lease<'a> {
    budget: &'a ResourceBudget,
    live: u64,
    retained: u64,
}

impl Lease<'_> {
    /// This lease's live charge.
    pub const fn live_bytes(&self) -> u64 { self.live }

    /// This lease's retained charge.
    pub const fn retained_bytes(&self) -> u64 { self.retained }

    fn absorb(&mut self, mut other: Self) {
        self.live += other.live;
        self.retained += other.retained;
        other.live = 0;
        other.retained = 0;
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let mut state = self.budget.counters.lock().unwrap_or_else(|e| e.into_inner());
        state.live_bytes -= self.live;
        state.retained_bytes -= self.retained;
        self.budget.observed_live.store(state.live_bytes, Ordering::Release);
    }
}

pub(crate) fn add(a: u64, b: u64) -> Result<u64, ResourceError> {
    a.checked_add(b).ok_or(ResourceError("resource arithmetic overflow"))
}

pub(crate) fn bytes<T>(count: usize) -> Result<u64, ResourceError> {
    u64::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(size_of::<T>() as u64))
        .ok_or(ResourceError("resource arithmetic overflow"))
}

/// The vector precedes its lease, including during partially completed copies.
/// Parent storage separately prices this wrapper's inline size.
pub(crate) struct BudgetVec<'a, T> {
    data: Vec<T>,
    lease: Lease<'a>,
    limits: ResourceLimits,
}

impl<'a, T> BudgetVec<'a, T> {
    pub(crate) fn new(budget: &'a ResourceBudget, limits: ResourceLimits) -> Result<Self, ResourceError> {
        Ok(Self { data: Vec::new(), lease: budget.reserve(0, 0, limits.live_bytes, limits.retained_bytes)?, limits })
    }

    pub(crate) fn as_slice(&self) -> &[T] { &self.data }

    pub(crate) fn reserve_len(&mut self, len: usize, ceiling: usize) -> Result<(), ResourceError> {
        if len > ceiling { return Err(ResourceError("collection count limit exceeded")); }
        if len <= self.data.capacity() { return Ok(()); }
        let capacity = self.data.capacity().saturating_mul(2).max(len).min(ceiling);
        let requested = bytes::<T>(capacity)?;
        // Retained growth needs only its delta, but live growth includes both
        // complete buffers. The old lease remains in force until deallocation.
        let growth = self.lease.budget.reserve(
            requested,
            requested.saturating_sub(self.lease.retained),
            self.limits.live_bytes,
            self.limits.retained_bytes,
        )?;
        let mut pending = Self { data: Vec::new(), lease: growth, limits: self.limits };
        pending.data.try_reserve_exact(capacity).map_err(|_| ResourceError("allocation failed"))?;
        let actual = bytes::<T>(pending.data.capacity())?;
        if actual > requested {
            let extra = self.lease.budget.reserve(actual - requested, actual - requested, self.limits.live_bytes, self.limits.retained_bytes)?;
            pending.lease.absorb(extra);
        }
        pending.data.append(&mut self.data);
        let old = std::mem::replace(&mut self.data, pending.data);
        drop(old);
        // Transfer the retained credit only after the old allocation is gone.
        pending.lease.retained += self.lease.retained;
        self.lease.retained = 0;
        self.lease = pending.lease;
        Ok(())
    }

    pub(crate) fn copy_from(&mut self, input: &[T]) -> Result<(), ResourceError>
    where T: Copy {
        self.reserve_len(input.len(), input.len())?;
        self.data.extend_from_slice(input);
        Ok(())
    }

    pub(crate) fn insert_reserved(&mut self, index: usize, value: T) { self.data.insert(index, value); }
    pub(crate) fn push_reserved(&mut self, value: T) { self.data.push(value); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(live: u64, retained: u64) -> ResourceLimits {
        ResourceLimits { live_bytes: live, retained_bytes: retained, encoded_bytes: 1, header_bytes: 1, node_bytes: 1, nesting: 1, nodes: 1, combos: 1, action_entries: 1, string_bytes: 1 }
    }

    #[test]
    fn overflow_and_external_classes_are_explicit() {
        let budget = ResourceBudget::new(u64::MAX, u64::MAX).unwrap();
        let baseline = budget.snapshot();
        assert_eq!(budget.reserve_external(u64::MAX, MemoryClass::Temporary).unwrap_err().0, "resource arithmetic overflow");
        let temporary = budget.reserve_external(100, MemoryClass::Temporary).unwrap();
        assert_eq!(budget.snapshot().retained_bytes, baseline.retained_bytes);
        let retained = budget.reserve_external(200, MemoryClass::Retained).unwrap();
        assert_eq!(budget.snapshot().live_bytes, baseline.live_bytes + 300);
        drop((temporary, retained));
        assert_eq!(budget.snapshot().live_bytes, baseline.live_bytes);
    }

    #[test]
    fn growth_prices_overlap_and_only_the_retained_delta() {
        let baseline = size_of::<ResourceBudget>() as u64;
        let budget = ResourceBudget::new(baseline + 24, baseline + 16).unwrap();
        let mut values = BudgetVec::<u64>::new(&budget, limits(baseline + 24, baseline + 16)).unwrap();
        values.copy_from(&[1]).unwrap();
        values.reserve_len(2, 2).unwrap();
        assert_eq!(values.as_slice(), &[1]);
        assert_eq!(budget.snapshot().live_bytes, baseline + 16);
        assert_eq!(budget.snapshot().retained_bytes, baseline + 16);
        assert_eq!(budget.snapshot().peak_live_bytes, baseline + 24);
        assert!(values.reserve_len(3, 3).is_err());
        assert_eq!(values.as_slice(), &[1]);
        drop(values);
        assert_eq!(budget.snapshot().live_bytes, baseline);
    }

    #[test]
    fn elements_drop_while_their_capacity_is_still_reserved() {
        struct Observe<'a> { budget: &'a ResourceBudget, expected: u64 }
        impl Drop for Observe<'_> {
            fn drop(&mut self) { assert_eq!(self.budget.snapshot().live_bytes, self.expected); }
        }
        let budget = ResourceBudget::new(10_000, 10_000).unwrap();
        let mut values = BudgetVec::new(&budget, limits(10_000, 10_000)).unwrap();
        values.reserve_len(2, 2).unwrap();
        let expected = budget.snapshot().live_bytes;
        values.push_reserved(Observe { budget: &budget, expected });
        drop(values);
        assert_eq!(budget.snapshot().live_bytes, size_of::<ResourceBudget>() as u64);
    }
}
