# Turn acceptance corrections, 2026-09-10

B1/B2/B3 are corrected on `solver/astra-turn-gate-corrections`, based on `2f81339`.
No Rust, Decision 14 candidate, or committed capture/review record changed.

- Root summaries now require dimensions, finiteness, zero-sum consistency, the
  captured root policy/action-EV mixture, display-origin agreement, and the
  project's own best-response interval. Cross-solver agreement uses the sum of
  measured NashConv gaps plus the documented arithmetic allowance.
- Every C row is walked again from the current capture. Stored oracle values
  remain review evidence; they cannot substitute for the current walk.
- Before classification, project reach and compatible mass are reconstructed
  from ranges, policies, blockers and chance. EV availability follows the
  producer's decision/chance distinction. Reference checks enclose the pinned
  wrapper's decimal rounding, truncated empty-range flags and f32 arithmetic.

The numerical allowances and source links are in
[the turn README](../../tests/reference/turn/README.md). The reference display can
hide a tiny positive policy as zero, or withhold EV after rounding its compatible
joint weight to zero. Exact equality against displayed reference reach would
reject thousands of legitimate rows. The implementation checks explicit intervals;
it does not loosen the project check or the candidate B threshold.

The synthetic fixture now reports consistent root summaries and path reach. Its
one-chip action gaps remain deliberate inputs to isolated classification tests.
Nineteen new regressions use the shipped rules and cover the five accepted bad
mutations, malformed roots, BR interval violations, false mass, missing reference
EV, genuine zero-action reach, runout/private-card blockers, range scaling, f64 underflow and f32 presentation.
The C-row controls retain the original exact oracle values `[-1, 2.5]`; the changed
chance continuation produces `[49, 2.5]`, and the 1-point downstream policy change
produces `[-0.98, 2.5]`. Both are refused.

Validation:

- `python -B -m unittest discover -s tests/reference/turn -p 'test_*.py'`:
  163 passed. Black, Ruff, the input-contract check and the prose checker passed. The sandbox refused writes inside newly created temporary test
  folders; the approved run used `target/python-tmp` and passed.
- Both downloaded `e338d7a` OS captures accepted with zero gate failures, missing
  rows or stale rows, using the unchanged committed review and shipped rules.
  A/B/C counts stayed 10,998/32,193/351, 17,029/35,561/189 and 9,913/32,227/277.
  The final complete gate took 37.17 seconds for Linux and 40.36 for Windows, excluding
  15.40/16.91 seconds of TOML parsing. Every OS pass recomputed all 817 C
  rows; maximum error was `6.856737400084967e-13` chips. Summaries are retained in this checkout's
  ignored `target/linux-gate-summary.json` and `target/windows-gate-summary.json`.

The 117 oracle rows using reported turn chance values remain dependent on those
leaf values. Called-turn-all-in numerical coverage remains Astra's separate next
step. A corrected gate is not an independent complete turn solve, and this branch
is neither merged nor pushed by the executor. Astra still owns final acceptance.
