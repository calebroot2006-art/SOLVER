---
type: plan
status: awaiting-independent-review
date: 2026-09-10
---

# Phase 6 payout types and ICM stub

This isolated step 8 implementation follows the reviewed `docs/phase-6/PLAN.md`
at base `69dc3ff`. Work is confined to payoff source, its README and this note.
It adds no dependencies, lockfile changes, engine behavior or equity calculation.
The existing Payoff trait and ChipEv implementation/tests remain unchanged.

`PayoutStructure::new(Vec<Real>)` accepts explicit finishing-place amounts,
requiring finite, nonnegative values in non-increasing order. `amounts()` exposes
a read-only slice. Ties, trailing zeros and all-zero structures are preserved.
There is no Default or seat-count cap.

API decision for Astra's review: an empty vector is invalid because a payout
structure must name at least one finishing place. A caller can explicitly name
unpaid places with zero amounts. Validation does not sum the prize pool; finite
amounts whose sum would overflow still satisfy this type's per-amount contract.
Tournament accounting must validate its own aggregate arithmetic separately.

`Icm::new(PayoutStructure)` owns those payouts; `payouts()` returns a read-only
reference. `try_utilities` always returns
`PayoffError("ICM not implemented in phase 6")`, leaving output unchanged.
This includes malformed or empty requests. The unchanged Payoff trait's Icm
implementation fills every supplied output with NaN, including ragged requests;
an empty output remains empty. No finite substitute equity is returned.

Tests reject missing, non-finite, negative and increasing payouts. Accepted
cases preserve supplied float bits, ties, zeros, more than nine places and large
finite amounts. Six ICM request shapes cover valid-looking requests, malformed
numbers, mismatched dimensions, empty inputs with nonempty output, and empty
outputs. Checked-output comparisons use float bits, including an existing NaN
payload and signed zero. Unchecked tests dispatch through `&dyn Payoff`.

Validation commands:

- `cargo test -p payoff --locked --offline`: five unit tests passed; doc tests
  passed with zero cases. Both original ChipEv tests are retained.
- `cargo clippy -p payoff --all-targets --locked --offline -- -D warnings`.
- `cargo fmt --all -- --check` and `git diff --check`.
- `python .claude/skills/anti-ai-slop-writing/slopcheck.py --internal
  crates/payoff/README.md docs/astra/phase6-payoff/PLAN.md`.

Checks run locally on Windows without changing security settings. The README
replaces its stale compiler-blocked statement with that observed result.
Independent source review, integration checks and full phase 6 acceptance remain
Astra's. This executor does not push, merge or mark the full phase complete.
