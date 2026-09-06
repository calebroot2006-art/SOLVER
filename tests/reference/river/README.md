# River reference capture

This directory contains independently authored inputs and a driver for the
upstream single-thread WASM solver. It contains no measured solver results yet.
Run and review the capture in GitHub Actions before treating it as phase 3 evidence.

`cases.json` supplies the agreed 20, 100 and 200bb cases. `diagnostics.json`
supplies check/showdown and bet/fold cases for checking the EV origin. Their
boards, ranges and menus are validation inputs. They are not product defaults.
The parser deliberately accepts only the input vocabulary used by these cases:
comma-separated pairs or unrestricted two-rank classes, such as `AA,AK,AQ`.
`AK` expands to all sixteen suit combinations. Weighted, suited, plus and dash
range notation are outside this capture's input contract.

## CI invocation

Use Linux with Python 3.12.10, Node 24.19.0, Git and rustup available. The runner
installs nightly-2023-10-01 with `rust-src` and wasm-bindgen-cli 0.2.87 inside its
fresh external temporary directory. Upstream's Cargo configuration builds the
standard library and requires that source component. It needs no Python packages,
npm installation or credentials.
Allow a 60-minute job budget for compilation and both capture invocations.

```sh
python tests/reference/river/capture.py --validate-only
python -m unittest discover -s tests/reference/river -p 'test_*.py'
node --test tests/reference/river/capture.test.mjs
python tests/reference/river/capture.py \
  --inputs tests/reference/river/diagnostics.json \
  --temp-root "$RUNNER_TEMP" \
  --output target/river-reference/diagnostics.json
python tests/reference/river/capture.py \
  --inputs tests/reference/river/cases.json \
  --temp-root "$RUNNER_TEMP" \
  --output target/river-reference/cases.json
```

Each invocation builds in its own temporary tree. Upload only the two output JSON
files. Do not upload the temporary directory, generated bindings, WASM, compiler
outputs, Cargo files or command logs. An existing output file causes failure.
Subprocesses have individual deadlines, logs are bounded to 16 MiB, public trees
to 10,000 nodes and 64 actions per history, solver memory estimates to 512 MiB,
and input/output JSON to 64 KiB/64 MiB. Node's heap limit is 1024 MiB. These are
capture guards, not an operating-system memory certificate: WASM and compiler
allocations also consume runner memory. The job timeout supplies the outer limit.

For a convergence comparison that uses every configured iteration, add
`--finish-budget` to the Python capture invocation and choose a separate output
file. The driver then runs exactly `input.max_iterations`, even if a residual
checkpoint already reaches the target. The original inputs remain unchanged.
Both the top-level output and each case record
`execution_stop_policy: "fixed_iteration_budget"`; each case's `stop_reason` is
also `fixed_iteration_budget`, and the validator requires the full iteration
count. Without the flag, `execution_stop_policy` is `target_or_cap` and the
existing target-or-cap stopping behavior applies. Direct Node driver calls accept
`--finish-budget` after the three positional file arguments.
For earlier schema 1 captures, a missing policy field means `target_or_cap`.
Those captures cannot satisfy validation for a requested fixed iteration budget.

For a separately labeled capture without wrapper display rounding or the 0.0005
reach cutoff, add `--raw-display`. Combine it with `--finish-budget` to compare
both presentation modes at the same full budget, using distinct output paths:

```sh
python tests/reference/river/capture.py --inputs tests/reference/river/cases.json \
  --output "$RUNNER_TEMP/river-raw-fixed.json" --temp-root "$RUNNER_TEMP" \
  --finish-budget --raw-display
```

The Python orchestrator applies and verifies the wrapper instrumentation before
building. The internal Node flag labels that build; it does not modify a WASM
binary. Node accepts the two optional flags in either order. Earlier schema 1
captures without `presentation_mode` mean `upstream_display` and cannot satisfy
raw-mode validation.

## Source and build separation

The upstream app is pinned to
[`97360db7644329b1c23a7adf06e9aa59406e4d4b`](https://github.com/b-inary/wasm-postflop/commit/97360db7644329b1c23a7adf06e9aa59406e4d4b),
and the engine to
[`9d1509fe5077d019825f833eed04b16d342dfda1`](https://github.com/b-inary/postflop-solver/commit/9d1509fe5077d019825f833eed04b16d342dfda1).
Git verifies both checkouts. The driver replaces the app manifest's floating engine
dependency with the verified external checkout and fixes wasm-bindgen to 0.2.87,
once_cell to 1.18.0 and regex to 1.9.6. It generates a Cargo lockfile there and
builds with `--locked`. Remaining resolved dependency versions and checksums are
recorded, so changing registry resolution is visible rather than implied identical.
The capture is not an archival reproduction of the hosted website's deployment.

The engine source is unmodified. wasm-bindgen generates Node bindings for the
compiled `wasm32-unknown-unknown` module; a separate Node process executes that
WASM. No native reference solver substitutes for it. No application crate,
manifest or application process imports the reference. Upstream source, licenses,
generated dependencies and binaries remain outside the checkout and are removed
when the invocation exits. The [upstream AGPL license](https://raw.githubusercontent.com/b-inary/postflop-solver/9d1509fe5077d019825f833eed04b16d342dfda1/LICENSE)
stays with the external source. Retained artifacts contain factual inputs,
measurements and provenance only.

Default captures perform no wrapper edits. Raw mode changes only the pinned
wrapper's one `round(f64)` function and its two weight-display `trunc` closures
to return their inputs unchanged. The guard requires exactly those three spans,
checks their SHA-256 hashes and the whole source hash before replacement, then
checks the expected whole source hash afterward. Any mismatch stops before
writing. Initialization, allocation, solve steps, exploitability and finalization
remain untouched; display queries still run after finalization.

Instrumentation ID: `wasm_wrapper_raw_display_v1`. The original wrapper SHA-256 is
`b28410955c073a656381c76d8ebde9b231ef4f4800fd9f25733a0848fb0d8c08`;
the instrumented SHA-256 is
`43be71b38f47ba7d187609f491f407dc6df5ed666647c2a0c4262baf99094c1a`.
These identify `rust/solver-src/lib.rs` at the pinned app revision. Provenance
records both hashes, the instrumentation ID, replacement counts and the generated
WASM hash. Default provenance records equal wrapper hashes, a null instrumentation
ID and zero replacements. Hashes and independently authored identity replacements
are retained here; the upstream implementation remains in the external checkout.

## Output schema 1

The top-level object contains `schema_version`, `capture_version`, `runtime`,
`presentation_mode`, `interface`, `cases` and `provenance`. Provenance records
revisions, compiler version, dependency resolution, input/driver/manifest/lock/WASM hashes, build
adjustments and timed command arguments. The temp paths in command arguments
describe an expired build directory; they are not reusable executable paths.

Each result contains its exact `input`, `private_cards` for both players, every
public `node`, measured `iterations`, `stop_reason`, residual in chips and percent
of root pot, checkpoints, root EVs, elapsed time and the reference memory estimate.
Runtime metadata records process peak RSS and Node memory at completion. The
`wasm_memory_bytes` field is null because GameManager does not expose that value.
No memory estimate is labeled measured WASM memory.

Every node carries index and labeled histories, legal action labels/amounts,
acting player, terminal reason, contributions, reported reach/normalized weights,
equity, per-hand EV, strategy and action EV. Strategy and action EV use
`action_index * private_hand_count + hand_index`. Private hands carry both card
labels and IDs. Card IDs follow `4 * rank + suit`, with ranks `2..A` and suits
`c,d,h,s`. Each flop is sorted by card ID before passing the board upstream.
See the [card API](https://b-inary.github.io/postflop_solver/postflop_solver/type.Card.html).

`reported_contributions` preserves GameManager's amounts before a fold refund;
`contributions` records retained chips after the uncalled excess is returned.
`fold_winner` is explicit. No node or policy action is dropped for low reach.
The driver's independent Python validation checks complete parent/child coverage,
physical input ranges, row dimensions and the nonbinding 32-raise cap. The project
must still independently compare the full exported betting tree before comparing
frequencies.

## EV and precision limits

GameManager reports current-decision EV with folding equal to zero. For a player
with river contribution `c`, the root-centered value is
`display_ev - starting_pot / 2 - c`. At the root, subtract half the pot only.
`root_expected_values` uses the returned normalized weights; its centered version
subtracts half the pot. Check this origin on `diagnostics.json` before explaining
frequency differences with action EVs. [Engine EV query](https://raw.githubusercontent.com/b-inary/postflop-solver/9d1509fe5077d019825f833eed04b16d342dfda1/src/game/interpreter.rs)

By default, the [WASM interface](https://raw.githubusercontent.com/b-inary/wasm-postflop/97360db7644329b1c23a7adf06e9aa59406e4d4b/rust/solver-src/lib.rs)
rounds its results, using six decimal places below one and fewer places for larger
values. It reports individual reach weights below 0.0005 as zero. These exported
reach values are display data, not exact mathematical reach. `wasm_empty_range_flag`
identifies a whole range below that display threshold. At those nodes normalized
weights are unavailable and all EVs are null. Elsewhere a hand with zero reported
normalized weight has null EV/equity. `ev_available` identifies the usable cells.
Undefined cells never become numerical zero. Rounded strategy values still cover
every action, including zero values. EQR is discarded because that interface can
produce infinities when equity is zero.

Raw mode records `presentation_mode: "raw_f32"`, `reach_display_cutoff: 0`,
`values_rounded_by_upstream: false` and `values_below_1_decimal_places: null`.
Default mode records `upstream_display`, 0.0005, true and 6 respectively. Both
record `arithmetic_precision: "f32"`; exporting those values as f64 adds no
precision. Raw mode preserves tiny positive weights and existing unrounded EVs.
Actual zero-mass or impossible-combo EVs remain null in both modes. Compare paired
captures at the same iteration budget, applying the original display rounding to
shared cells; the root-owned numerical review performs that check. Local parser
tests alone do not establish equivalence of measured captures.

The requested target is 0.001 percent of pot, or 0.0001 chips with a ten-chip pot.
`solve_step` receives indices starting at zero; the driver records actual residual
checks and finalizes once after stopping. An iteration cap remains a cap even if
the strategy looks plausible. The API's f32 residual is preserved as returned;
small negative values, if observed, require numerical interpretation during review.
This tooling does not assert convergence, pass the two-percentage-point strategy
gate, or certify a per-decision EV error bound.

The Python tests cover input guards, exact source replacement counts and hashes,
default no-write behavior, provenance and raw metadata. Node tests use explicitly
synthetic protocol buffers to check parsing, tiny positive reach, missing EVs,
option combinations and rejected malformed results.
Those buffers are not captured poker solutions. Local checks do not establish
that the historical toolchain builds on the CI runner; a failed WASM build remains
an open reference gate and must be reported with its command failure.


## Independent scalar evaluation and per-combo evidence

`oracle.py` independently expands the explicit unweighted rank classes used by
these fixtures, ranks each seven-card hand by its five-card subsets, and sums
compatible physical deals. A best response chooses after summing opposing hands.
It cannot choose differently after seeing the opponent's private cards.

Exported probability rows are checked and normalized in f64 before evaluation.
Reports identify that policy basis and its largest probability adjustment. The
reference's native f32 residual stays a separate measurement; normalization does
not supply a per-hand error bound. Weighted or abbreviated range syntax is outside
this scalar fixture evaluator's contract and is rejected.

`verify_project.py` independently checks both root EVs, both best responses,
exploitability units, every own reach and compatible opponent mass, and every
available action EV, with absolute tolerance 1e-9. Undefined action EV arrays must
be empty. CI applies it to diagnostics, target-stopped cases and full-budget cases
on both platforms. Its tests include malformed probabilities and a response whose
choice must not depend on opposing private cards.

`review_combos.py` preserves every combo that differs by over two percentage points
in either the initial or refined comparison. For each row it records both policies,
independent counterfactual action EVs, action gaps, own and opposing reach, and the
root EV effect of changing only that row. It also records the initial measurements
for comparison with refinement. Own reach zero does not make counterfactual action
values zero; opposing compatible mass zero leaves those values undefined. Large
conditional losses on rare branches remain explicitly visible. Single-row effects
are not additive and do not certify simultaneous deviations or advice at other
starting states. The report requires a separate written numerical review.


The [measured refinement record](measured/2930550/README.md) explains all 588 rows
that differed by over two percentage points in either capture. Its linked JSON
contains individual histories, physical combos, action gaps and written reasoning;
27 rows return within tolerance, 469 have zero reference own reach, six have zero
reference opponent mass, and 86 are reached with directly measured small incentives.
The record preserves large conditional losses on rare paths and distinguishes
them from indifferent or nearly indifferent mixtures.
