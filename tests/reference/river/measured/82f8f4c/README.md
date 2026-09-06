# First measured river capture

These files are unchanged factual outputs from project commit
`82f8f4ccd0feeead16ccd9415b7c16ed56180622`,
[CI run 34051757916](https://github.com/calebroot2006-art/SOLVER/actions/runs/34051757916).
`provenance.json` binds each file hash to its artifact digest and records all job
steps. The reference JSON also records upstream revisions, toolchain, dependency
resolution, source adjustments and generated WASM hashes.

Both project capture steps, all Rust tests, and existing independent numerical
checks passed. The real WASM reference capture passed. This commit's overall CI
was not green: a tree test used a Clippy-rejected slice-size expression, and the
project example needed hosted formatting. Those findings require later validation.
No phase 3 acceptance is implied by these measured fixtures.

The project and reference match all public histories, action labels, contributions
and physical combo sets. All three named cases measured below 0.001% of root pot
exploitability; the largest root EV difference was below 0.00008 chips. Strategy
differences above two percentage points remain in 579 later-history rows. The
original display cutoff leaves many reference EVs unavailable. These fixtures
preserve the initial evidence for refinement and comparison-guard regression tests.
They are not hand-authored solutions or product range charts.

`project-*.toml` contains the project capture. `reference-*.json` contains the
unmodified pinned wrapper's displayed output. Diagnostic files establish the
known AA-versus-QQ payoff origin. The comparator also checks raised-history fold
cells to establish the contribution offset, requiring at least one such cell.
