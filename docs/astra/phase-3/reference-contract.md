# External river reference capture

Tooling executor owns only `tests/reference/river/**` in isolated worktree
`.claude/worktrees/astra-phase3-reference`, branch `solver/astra-phase3-reference`.
Root owns CI wiring, fixture acceptance, solver implementation, and final review.
Do not run Rust locally, push, or copy vendor source/binaries into this repository.

Build and run the pinned upstream single-thread WASM interface as a separate
external reference program in a fresh directory under the CI runner's temporary
root. Pin wasm-postflop to `97360db7644329b1c23a7adf06e9aa59406e4d4b` and its engine
dependency to `9d1509fe5077d019825f833eed04b16d342dfda1`. Record the manifest pin as
a build adjustment; do not claim this recreates the hosted website's old build.
Retain licenses in that external checkout. Our app, crates and Cargo.lock never
link or import the reference. Export only numerical input/output and provenance.

Create a Python orchestration script and the minimal independent JavaScript
capture driver needed to call upstream GameManager. The script validates and
bounds input, source revision, output size and JSON before saving; invoke tools
without shell string interpolation. Compiler/toolchain/wasm-bindgen or wasm-pack
versions must be exact and recorded. Any additional dependency installation runs
only in the external temporary tree. No paid API or credential is needed. Avoid
new application dependencies. Document the exact Linux CI invocation for root.

Input JSON format: top-level `schema_version: 1`, `cases: [...]`. Each case has
`id`, `board` (five card labels), `ranges` (two independently authored range
strings), `chips_per_bb`, `starting_pot`, `effective_stack`, `bets` (two strings),
`raises` (two strings), `min_bet: 1`, `max_raises: 32`,
`add_all_in_threshold: 0`, `force_all_in_threshold: 0`,
`target_pct_of_pot: 0.001`, `max_iterations: 20000`, `check_every: 100`.
Use zero rake, zero merging, no compression, no added/removed lines. The maximum
raise cap is not expected to bind with these geometric pot growth settings;
our later exact tree comparison must establish that rather than assume it.

Write independently authored input fixtures for three named cases with
`chips_per_bb: 1`, `starting_pot: 10`, both bet menus `50%`, both raise menus `100%`:

- `river_20bb_dry`: stack 20, board `Ac Kd 7s 4h 2c`,
  ranges `AA,KK,77,AK,AQ` and `AA,KK,77,KQ,QJ,JT`.
- `river_100bb_paired`: stack 100, board `Kh Kd 7s 7c 2d`,
  ranges `AA,QQ,JJ,KQ,AQ,QJ` and `AA,QQ,TT,AK,QJ,JT`.
- `river_200bb_flush`: stack 200, board `As Js 8s 4s 2h`,
  ranges `AA,KK,QQ,AK,AQ,KQ,QT,T9` and `JJ,TT,99,AJ,KJ,QJ,JT,T9`.

Add two diagnostic input cases separately if needed: forced check/showdown and
single bet/fold. These verify per-hand/action EV origin before using it to
explain strategy differences. These settings are validation inputs, not product
defaults or proprietary charts.

Export every public decision/terminal history with labeled legal actions and
amounts, player, contributions, physical private-card labels, reach/normalized
weights, action-major strategy and action EV, root EVs, actual exploitability in
chips and pot percent, iteration count and stop reason. Navigate with explicit
history indices but preserve labels so root can independently match trees.
Do not drop tiny-frequency actions or unreachable histories. Mark zero-reach or
impossible combo EVs unavailable; never turn missing values into zeros. Record
WASM interface rounding and reach truncation. Avoid EQR as an acceptance metric.

Output includes exact inputs, source commits, compiler and capture versions,
lockfile and WASM hashes, invocation/settings, timing and reference memory use.
Record the schema in README. Actual outputs may be saved only after root runs
and reviews CI; do not invent or hand-author reference results. Use bounded
timeouts and resource limits. Keep vendor/generated files out of commits and
uploaded artifacts; root uploads only the factual capture and provenance.

If the WASM route fails, report the concrete failure. A separately compiled
native engine process can be a diagnostic fallback only when labeled accurately;
it must not be reported as a measured WASM comparison. Keep all application
license boundaries intact. Do not quietly replace the roadmap reference gate.

## Reviewed presentation instrumentation

The first measured WASM capture at project commit `82f8f4c` built and ran.
All named trees match the project; 579 policy rows differ by more than two
percentage points. Many lack reference EVs because its display truncates reach.
Root therefore authorizes a separately labeled `--raw-display` capture in addition
to the unchanged default capture. This extends the tooling executor's contract.

Only the pinned wrapper's `round(f64)` function and the two weight-display `trunc`
closures may change, returning their input unchanged. Require the exact expected
source shapes and replacement counts. No engine source, initialization, memory
allocation, solve step, exploitability calculation, or finalization may change.
The display queries run after finalization. Keep the original external licenses.

Record original and instrumented wrapper hashes, a versioned instrumentation ID,
the unchanged upstream revisions, the generated WASM hash, and explicit raw
presentation metadata. Raw results retain f32 arithmetic; promotion to f64 does
not add precision. They expose existing values without decimal rounding or the
0.0005 display cutoff. Actual zero-mass EVs remain unavailable. Default captures
must continue to perform no wrapper edits. Compare a default/raw pair at the same
iteration budget after applying the original display rounding to shared cells.
Use `--finish-budget` for a separately labeled 20,000-iteration refinement; do not
change the original fixtures or describe a capped run as target-driven stopping.
