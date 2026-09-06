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

The solver source is unmodified. wasm-bindgen generates Node bindings for the
compiled `wasm32-unknown-unknown` module; a separate Node process executes that
WASM. No native reference solver substitutes for it. No application crate,
manifest or application process imports the reference. Upstream source, licenses,
generated dependencies and binaries remain outside the checkout and are removed
when the invocation exits. The [upstream AGPL license](https://raw.githubusercontent.com/b-inary/postflop-solver/9d1509fe5077d019825f833eed04b16d342dfda1/LICENSE)
stays with the external source. Retained artifacts contain factual inputs,
measurements and provenance only.

## Output schema 1

The top-level object contains `schema_version`, `capture_version`, `runtime`,
`interface`, `cases` and `provenance`. Provenance records revisions, compiler
version, dependency resolution, input/driver/manifest/lock/WASM hashes, build
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

The [WASM interface](https://raw.githubusercontent.com/b-inary/wasm-postflop/97360db7644329b1c23a7adf06e9aa59406e4d4b/rust/solver-src/lib.rs)
rounds its results, using six decimal places below one and fewer places for larger
values. It reports individual reach weights below 0.0005 as zero. These exported
reach values are display data, not exact mathematical reach. `wasm_empty_range_flag`
identifies a whole range below that display threshold. At those nodes normalized
weights are unavailable and all EVs are null. Elsewhere a hand with zero reported
normalized weight has null EV/equity. `ev_available` identifies the usable cells.
Undefined cells never become numerical zero. Rounded strategy values still cover
every action, including zero values. EQR is discarded because that interface can
produce infinities when equity is zero.

The requested target is 0.001 percent of pot, or 0.0001 chips with a ten-chip pot.
`solve_step` receives indices starting at zero; the driver records actual residual
checks and finalizes once after stopping. An iteration cap remains a cap even if
the strategy looks plausible. The API's f32 residual is preserved as returned;
small negative values, if observed, require numerical interpretation during review.
This tooling does not assert convergence, pass the two-percentage-point strategy
gate, or certify a per-decision EV error bound.

The Python tests cover input and source guards. Node tests use explicitly synthetic
protocol buffers to check parsing, missing EVs and rejected malformed results.
Those buffers are not captured poker solutions. Local checks do not establish
that the historical toolchain builds on the CI runner; a failed WASM build remains
an open reference gate and must be reported with its command failure.
