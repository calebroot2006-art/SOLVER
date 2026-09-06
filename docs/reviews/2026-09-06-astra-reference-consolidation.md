---
project: gto-solver-app
type: review
status: not-yet-verified
date: 2026-09-06
---

# Shared DCFR reference review

**Verdict: not yet verified.** Local numerical checks pass; the integrated
Windows/Linux CI result is pending. This change completes Fable's uncommitted
reference cleanup on `solver/phase-1-review`, based on `713c43e`.

## Findings and corrections

**R01, medium, resolved locally:** the draft's average-policy check compared only
NashConv (`tests/reference/openspiel/test_dcfr_reference.py`). Equal residuals do
not establish equal action probabilities. Tests now compare every legal action's
current and normalized average probabilities, signed regrets, and scaled averaging
sums. A normalized policy mutation and an altered averaging discount both fail
the intended checks. Kuhn is checked through 30 iterations; Leduc through five.

The whole-sum recurrence gives project accumulator `A_T = S_T / (T+1)^2`, where
`S_T` is upstream's sum of contributions weighted by `t^2`. Positive regrets at
each information set share a discount factor, so normalization preserves the
current policy in exact arithmetic. Longer independent floating-point trajectories
can diverge. The existing shared-state verifier still checks later updates.

**R02, medium, resolved locally:** the three entry points duplicated discount
arithmetic even though the draft described one shared implementation. `capture.py`,
`sensitivity.py`, and `verify_snapshots.py` now use `dcfr_reference.py` for solver
construction and discounts. New outputs record its executed hash. The exporter
records current reproducer hashes separately from historical capture metadata,
and identifies the actual installed wheel tags instead of hard-coding Windows.
Original numerical captures, their recorded source hashes, and fixtures are unchanged.

## Verification

Astra ran Black and Ruff, all 18 Python tests, the capture exporter, and the full
snapshot verifier against the previously verified Windows artifact `9983633878`.
Its archive hash was checked before reading the 108 expected CSV files. All
18 update pairs and 54 snapshots pass. All six short capture paths reproduce the
original first three checkpoints exactly on this machine. The exporter preserves
all six historical capture entries and both TOML fixtures. Source hashes and
local results are in [local-evidence.json](../astra/reference-consolidation/local-evidence.json).

The first temporary-file test run hit the local sandbox's directory ACL restriction;
the same tests pass under tool escalation with Windows security settings unchanged.
An independent agent checked the math and integrated code, reproduced the direct
average and sum comparisons, and inspected both official wheel source forms.
The test pins the canonical LF hash of OpenSpiel 2.0.2's `discounted_cfr.py`.

## Limits and closure

OpenSpiel's DCFR module cautions that its team has not verified reproduction of
the paper's results. This is a comparison with that implementation, not a claim
of paper reproduction or long-run bitwise equality. The existing capture extension
and Rust solver remain unchanged in algorithm and accuracy targets.
[Upstream module](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/open_spiel/python/algorithms/discounted_cfr.py).

Close the remaining gate only after both hosted solver jobs pass on the integrated
commit. No hold'em or app feature is certified by this reference cleanup.
