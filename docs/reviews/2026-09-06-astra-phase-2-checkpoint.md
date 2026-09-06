# Phase 2 cutoff checkpoint

**Verdict: needs changes.** Caleb requested a three-minute save cutoff during
verification on 2026-09-06. Phase 2 is implemented but is not accepted as complete.

## Reviewed scope

Cards, combos, weighted ranges, the rs_poker 5.1.0 adapter, independent evaluator
tests, and blocker-aware terminal sweeps were integrated at `d026317`.
Commit `3979f6f` adds 1000 independent Fraction fixtures for the exact accumulator.
Astra personally reviewed the agent code and dependency findings. All agent work
is committed and integrated; their isolated worktrees remain available.

## Findings

- **P2-01, medium, corrected pending rerun:** range whitespace handling rejected
  vertical tab despite the test contract. Linux reproduced it in
  `whitespace_empty_ranges_and_equal_overlaps_are_supported`. Parsing now includes
  vertical tab in both empty-input and token separation checks, with a regression.
- **P2-02, low, open:** hosted rustfmt requires formatting changes. Its checked
  artifact was applied except to `terminal/mass.rs`, where the newer golden test
  changed patch context. Obtain the next formatter patch and apply it after review.
  No local Rust compiler or formatter was run; Smart App Control stays enabled.
- **P2-03, verification gate, open:** the corrected parser and added Rust golden
  replay require the complete Windows/Linux CI rerun. Do not mark phase 2 complete.

## Evidence

[Run 34020235793](https://github.com/calebroot2006-art/SOLVER/actions/runs/34020235793)
tested `d026317efdb12377e0e91bd371a391650c6ca9d2`. Linux compiled and passed Clippy.
Its evaluator passed all 2598960 five-card hands and ten million seven-card samples
against the independent subset oracle. The seven-card gate took 13.620 seconds.
All Linux Rust targets passed except the identified range whitespace test;
the existing independent solver snapshot checks also passed. The full job failed
because of that test. Windows also reported a test failure; its full log still
needs inspection before claiming matching detailed results.

The original formatter archive has SHA-256
`b5caa724dbb8d9fbbf40d84f721d068e49ca8849b5f7171d09f2575cb276bac8`.
Local copies are `target/astra-temp/phase2-rustfmt.{zip,patch}` and
`target/astra-temp/phase2-first-linux.log`.

Astra ran Black, Ruff, and `python tests/reference/exact_mass_vectors.py --check`.
All 1000 fixture cases reproduce exactly. Their actual Rust replay is pending.
The [dependency evidence](../astra/phase-2/dependency-evidence.json) covers seven
new packages and the 15-package evaluator closure. No yanks or OSV entries were
returned; this static scope does not establish whole-app security.

## Next action

Finish hosted formatting and all numerical gates, inspect both platform logs,
record timing/storage results, and update this verdict. Preserve the mandatory
ten-million sample count and existing Kuhn/Leduc accuracy gates. The app remains
the starter preview; no playable table or coach is delivered by phase 2.
