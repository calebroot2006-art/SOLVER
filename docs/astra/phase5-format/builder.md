---
type: checkpoint
status: incomplete-unreviewed
date: 2026-09-10
---

# Bounded builder checkpoint

Saved at Caleb's stop request. This is unfinished implementation, not accepted
phase 5 step 1 work. Base is `c450806`, branch `spots/astra-phase5-builder`.

The current files implement borrowed version 1 DTOs, finite scalar wrappers,
private header/node arenas, structural validation, exact lookup and a shared
caller-owned resource budget. Nodes sort by ID. Full-key duplicate detection
uses a bounded allocation-free scan and permits identical contexts with disjoint
combos. Imported provenance, accuracy, root mass and completeness remain claims.
Only `cards` and `tree` production dependency edges were added.

`ResourceLimits` requires explicit values and has no default. Leases distinguish
temporary live charges from retained charges. Vector growth reserves both old
and new live allocations, charging only the retained delta. Storage fields
precede leases in drop order. Capacity accounting and inline housekeeping are
implemented but still need independent review. Caller-owned DTO allocations
require caller accounting, optionally through an external reservation.

Source metrics use the existing normalized negative-sum allowance of `1e-10`
and exact `(nash_conv / starting_pot) * 50.0` operation order. Signed zero is
preserved except the two canonical tree thresholds. Root compatible mass stays
a positive finite source claim. Range validation removes root blockers before
scaling each side by its surviving maximum and rejects positive compatible
product underflow. Calendar validation currently accepts years 0001 through
9999 and rejects leap-second timestamps; this API choice needs review.

Typed input supplies no encoded byte count or JSON nesting facts. The builder
checks semantic/string ceilings, a header byte lower bound and exact binary
node size. Actual encoded bytes, remaining input and JSON depth remain codec
gates. No codec, capture binding, SQLite or completeness certificate was added.

## Last checks

- `cargo check -p spots --offline --target-dir target-builder` passed.
- `cargo test -p spots --locked --offline --target-dir target-builder` passed:
  12 unit tests and 12 builder integration tests, 24 total; zero doc tests.
- The passing regressions cover duplicate IDs/keys/combos, missing and off-path
  EVs, later-board blockers, identity mapping, cumulative counts, binary node
  size, shared-budget refusal rollback, canonical tiny ranges, blocked maxima,
  source staleness/lowered caps, numerical operation order and error maxima.
- The last Clippy run failed on one `drop_non_drop` and two `collapsible_if`
  diagnostics. Those source lines were subsequently corrected, but Clippy has
  not been rerun. Formatting and prose checks have not been run.
- After the 24-test pass, `resource.rs` gained a lock-free live-charge observer
  for the planned allocator test. This final change has not been compiled or
  tested. It also changes the budget's size-derived housekeeping baseline.

## Resume work

The deterministic allocator test was planned but never written. Add actual
deallocation observation and forced fallible allocation failure. Existing unit
tests observe element drop, growth overlap, spare capacity, retained deltas and
overflow; they do not replace the missing allocator evidence. Review parser
scratch bounds, partial-construction cleanup, excess-capacity refusal and
constructor/result housekeeping end to end. Review the linear full-key scan's
cost at the caller's node ceiling. Complete malformed-input coverage, inspect
all public APIs against the schema, update the crate README, then run tests,
Clippy, formatting and prose checks. Astra's independent source and allocation
review remains outstanding.
