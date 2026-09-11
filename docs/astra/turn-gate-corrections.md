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

## Follow-up: tiny compatible mass

Astra's review of `4aec013` found that its absolute mass allowance could suppress
material normalized reach. The direct evidence-unit reproduction uses board
`Ac Ad Kh Qh` and both ranges `AA,KK:0.000000000000001`. Their dominant `AhAs`
combos block each other. Root compatible mass is `6e-15`; `AhAs` has opponent
mass `3e-15` and normalized reach `0.5`. A forged zero passed the former evidence
check and changed a C classification to B. The new tests also cover forged
`1e-100` and `1e-22` masses.

`opposing_mass` multiplies each opponent reach by chance, then `evaluate_fold`
uses `ExactMass`/`Bucket::compatible` to sum the compatible weights exactly and
round once to f64. The comparator now mirrors that multiplication order and
uses relative tolerance `1e-10` with no absolute floor for own reach, opponent
mass or the root normalizer. Root compatible mass is summed with `math.fsum`;
positive pair-product underflow is refused. Classification and review generation
normalize with the reconstructed root mass, even if the parsed capture is changed
later. No candidate threshold changed.

Three added regressions cover the demonstrated bypass, root-mass scale and the
normalizer used by classification. The complete Python suite now passes 166 tests.

Both complete OS gates passed again after this correction, with zero failures,
missing rows or stale rows. Each recomputed all 817 C rows; maximum error remained
`6.856737400084967e-13` chips and A/B/C counts stayed unchanged. Gate runtimes were
39.84 seconds for Linux and 38.74 for Windows, excluding parsing. Black, Ruff,
input validation and the prose checker also passed.
