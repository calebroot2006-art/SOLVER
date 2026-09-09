---
project: gto-solver-app
type: plan
status: proposed
date: 2026-09-09
---

# Phase 5: solved-spot format and library generator

Research: `docs/research/how-to-build-a-solver.md`.

Written by the `planner` subagent from a reader fact sheet on 2026-09-09, on Caleb's
instruction to start phases 5 and 6 beside phase 4. "Decisions" holds Caleb's answers to
the open questions, with the date each one was given. Nothing else is assumed. Astra
reviews this plan through `docs/reviews/` before the executor starts.

## Progress (updated 2026-09-09)

Read this first when picking the work up. It says what is done and verified, what is half
done, and what was learned that the plan below did not know. The executor updates it after
every step it finishes; the main session updates it after review.

**Where it stands:** nothing started. Waiting on Astra's plan review and on open
questions 1 to 4. Base: `solver/phase-4` at 0210d4a; the capture step (2) builds against
the turn API that merges with phase 4 step 3.

## Task

Fix the solved-spot and range-chart formats in `crates/spots`, add a SQLite library index,
and build `spotgen`, a CLI that solves a flop list for one scenario through the postflop
crate's public API and writes a library with a readable log. It replaces the ad hoc TOML
capture in `crates/postflop/examples/river_capture.rs` as the contract the app reads.

## Approach

One in-memory type, `Spot`, with two encodings: a hand-framed little-endian binary
(authoritative, specified byte by byte in `docs/phase-5/spot-format.md`) and a JSON export
via serde. Values are quantised at capture: strategy as `u16` (probability x 65535), action
EVs as `i16` with a per-node `f32` scale (b-inary's layout,
`docs/research/how-to-build-a-solver.md:140-142`), own reach as `f32`. Both encodings carry
the same quantised values, so round trips are exact and the only lossy step is capture,
whose bound is tested.

Every spot stores its coverage rule, because a full flop tree is not shippable. At about
1,176 flop combos x 3 actions x (2 B strategy + 2 B EV) + 4 B reach, a decision node is
about 19 KB, and a flop-gate tree of roughly 2 M decision nodes is about 38 GB. A
start-street cut (every decision node on the start street, roughly 100 to 300 nodes for
the approved menu) is 2 to 6 MB per spot, about 150 MB for 25 flops. Start street plus
every next-street runout is x49 turn cards x about 100 nodes, 0.5 to 1.5 GB per spot and
12 to 37 GB for 25 flops, rejected for the first library. The format also supports
`action_depth(N)` coverage and a per-node `runout_mapping` (representative runout plus
suit permutation) so phase 4 step 9's suit merging can be stored later; phase 5 writes
identity mappings only.

No solver-side accessor is missing: `PostflopGame::node`, `initial_weights`, `subtree`,
`PostflopStrategy::row` and `decision_values`, and `SolveReport` cover the payload. The
executor may not edit `crates/postflop`, `crates/tree`, `crates/cards`, `app/`, or
`docs/phase-4/`; phase 4 executors work there in parallel.

## Steps

**1. Format types and schema doc.** Files: `crates/spots/src/format.rs`,
`crates/spots/src/lib.rs`, `docs/phase-5/spot-format.md`.
`Spot { header: SpotHeader, nodes: Vec<NodeRecord> }`. Header: `format_version: u32`,
`spot_id`, `scenario_id`, `positions: [String; 2]`, `range_labels`, `ranges: [String; 2]`
(Pio canonical strings), `board`, `start_street`, `chips_per_bb`, `starting_pot`,
`effective_stack`, `tree: TreeSpec` (bet menus per street, `min_bet`, `max_raises`,
thresholds, `donk_option`, mirroring `PostflopTreeConfig` fields), `payoff_model:
"chip_ev_no_rake"`, `units: "chips"`, `solve: { iterations, exploitability_pct_of_pot,
best_response_values, root_expected_values, stop_reason, elapsed_seconds,
target_pct_of_pot, variant, precision }`, `provenance: { solver_revision,
spots_crate_version, generated_at (RFC 3339), host: {os, cpu_count, memory_bytes},
config_path }`, `coverage: StartStreet | ActionDepth(u16)`, `uncovered_policy:
"ungraded"`. Node: `node_id`, `compact_id`, `history` (action labels from the root),
`street`, `player`, `actions: Vec<String>`, `contributions: [u64; 2]`, `ev_scale: f32`,
`runout_mapping`, `combos: Vec<ComboRecord { combo_id: u16, strategy: Vec<u16>, ev:
Vec<Option<i16>>, own_reach: f32 }>`. `Spot::validate()` rejects non-finite numbers,
action-count mismatches, `combo_id >= 1326`, board and range conflicts, and strategy rows
not summing to 65535 within one unit per action. The doc includes the memory arithmetic
above and a versioning rule: bump `format_version` on any layout change; readers refuse
unknown versions.
Tests: `cargo test -p spots` validation cases; `slopcheck.py` on the doc.
Gate: `check` job.

**2. Capture from the solver.** File: `crates/spots/src/capture.rs`. Depends on 1 and on
the phase 4 step 3 merge. `Spot::capture(&PostflopStrategy, &SolveReport, &CaptureInputs)
-> Result<Spot, SpotError>` walks `game.node(id)` over `game.subtree(root)`, keeps decision
nodes matching the coverage rule, reads `strategy.row(node, combo)` and
`strategy.decision_values(node)` for combos with nonzero `initial_weights(player)`, sets
`ev_scale = max|ev| / 32767` per node, and errors (never clamps) if quantisation error
exceeds the bound.
Tests: on a small river-start `PostflopGame` (one bet size, 20-combo ranges), every stored
strategy is within 1/65535 of `row()`, every EV within `ev_scale / 2` of
`decision_values()`, and `header.solve.exploitability_pct_of_pot` equals
`strategy.exploitability().pct_of_pot`.
Gate: `check` job. Acceptance: the main session re-exports one spot and compares its rows
to a fresh `PostflopStrategy` on the same inputs.

**3. Binary codec.** File: `crates/spots/src/binary.rs`. Depends on 1; independent of 4
and 5. Magic `GTOS`, `format_version`, header length plus serde-JSON header bytes, a node
section with per-node `u32` lengths, CRC32 trailer (hand-rolled table, tested against
`"123456789" -> 0xCBF43926`). Decoder caps: header at most 1 MiB, nodes at most 4,000,000,
combos per node at most 1326, file at most 4 GiB, every length checked before allocation.
Tests: seeded in-test generators (xorshift, no new crate) produce 200 random valid spots;
`decode(encode(s)) == s` and `encode(decode(b)) == b`; a mutation test flips, truncates,
and inflates length fields in 1,000 variants and asserts `Err`, no panic, and no allocation
above the caps.
Gate: `check` job.

**4. JSON export and import.** File: `crates/spots/src/json.rs`. Depends on 1; independent
of 3 and 5. `Spot::to_json_string` and `from_json_str` with `deny_unknown_fields`,
documented in the schema doc.
Tests: the same 200 random spots round trip through JSON equal to the binary round trip.
Gate: `check` job.

**5. Range-chart format.** Files: `crates/spots/src/chart.rs`, `spots/charts/README.md`,
`spots/charts/example-btn-open.json`. Independent of 2 to 4 except `lib.rs` re-exports
(merge sequentially). `RangeChart { format_version, id, scenario, position,
stack_depth_bb, actions: Vec<{ label, range: PioString }>, provenance }`; each range parsed
with `Range::parse`; per-combo action weights must sum to at most 1 + 1e-9.
Tests: round trip; rejection of oversize text (`MAX_RANGE_BYTES`), unknown tokens, and
sums over 1.
Gate: `check` job.

**6. Library index.** File: `crates/spots/src/index.rs`. Depends on 1 and 3. `rusqlite`
with `bundled`; table `spots(spot_id PRIMARY KEY, scenario_id, street, board, positions,
stack_depth_bb, pot_chips, coverage, exploitability_pct_of_pot, stop_reason,
solver_revision, format_version, file, size_bytes, crc32, generated_at)`, `PRAGMA
user_version = 1`; `Index::rebuild(dir)` re-reads every `.spot`.
Tests: a temp-dir test inserts, looks up by scenario and board, and rebuild matches insert.
Gate: `check` job.

**7. Generator CLI.** Files: `crates/spots/src/bin/spotgen.rs`, `spots/scenarios/README.md`,
`spots/.gitignore` (for `library/`), `crates/spots/README.md`. Depends on 2, 3, 4, 6.
`spotgen generate --scenario <toml> --flops <json> --config <solver.toml> --out <dir>
--coverage <rule> --revision <sha>`, `spotgen export <spot> --json <path>`, `spotgen verify
<dir>`. Scenario TOML mirrors `tests/reference/turn/cases.json` fields; menus become
`BetSizeOptions` and `PostflopTreeConfig`, then `PostflopGame::new(board, ranges,
PostflopTree::new(cfg)?, PostflopOptions::from_config(&cfg)?)`, `PostflopSolver::new(game,
cfg.dcfr.variant())`, `solve(&cfg.solve, on_progress)`. The flop list is
`tests/reference/flop/flops.json` (`--take 25` for the gate). Log (`env_logger` plus
`<out>/generate.log`): per flop a start line with board and node count, each `Progress`
as iterations, exploitability percentage, and elapsed time, the stop reason, and bytes
written; a final summary table. A failed flop aborts with the solver's error unless
`--continue-on-error`, which records the failure in the log and the index.
Tests: `spotgen generate` on the river-start test scenario with 3 boards, then `spotgen
verify` exits 0 and the JSON export of one spot matches the `RiverStrategy` rows for that
board.
Gate: `check` job.

**8. Gate library.** Files: `spots/scenarios/<scenario>.toml`, `.github/workflows/ci.yml`
(a job) or a documented local run, this plan's Progress. Blocked on open questions 1 to 3.
Generate 25 flops for Caleb's scenario, keep `generate.log` and the index, record
exploitability per spot with its residual and stop reason.
Gate: every spot's `stop_reason == TargetReached` at the scenario's `target_pct_of_pot`,
or the log states which stopped on the cap; `spotgen verify` passes; total library size
within the arithmetic in the doc.

## Tests

* `cargo test -p spots --locked`, `cargo clippy --workspace --all-targets --locked -- -D
  warnings`, `cargo fmt --all --check`, in CI (this PC cannot compile reliably).
* Step 7's end-to-end test runs `spotgen generate` on the 3-board river scenario inside
  the crate's `tests/`, marked `#[ignore]` locally and run in CI.
* Failure cases: truncated file, wrong magic, version bump, `NaN` in the header, path
  traversal in the `file` column (`verify` rejects entries outside `dir`), inflated length
  fields.
* Most likely failure path: a capture that quantises an EV outside its node's scale and
  silently clamps. Caught by step 2's bound test and by the capture erroring instead of
  clamping.

## Risks and edge cases

* Capture correctness: the step 2 bound test is the accuracy gate; the main session
  independently re-exports one gate spot and compares rows to a fresh `PostflopStrategy`
  on the same inputs.
* The 25-flop library needs accepted flop solves; if the phase 4 flop gate is still open
  when step 8 runs, step 8 waits (open question 1).
* Coverage versus usefulness: a start-street cut grades flop decisions only; turn and
  river decisions in the same hand are ungraded until a wider cut or a live re-solve
  exists. Astra's coverage measurement proposal (roadmap Decisions) is where that gets
  quantified.
* Memory: generation stays under the solver's limit because `PostflopGame::new` refuses
  oversize trees; the writer streams nodes so the encoder never holds a second copy.
* Windows paths and line endings: the index stores relative POSIX paths; the log uses
  `\n`.
* Long runs: `Progress` lines are timestamped; a cancelled or capped solve is stored with
  its stop reason, never dropped.

## Open questions

1. Which preflop scenario for the first 25-flop library (positions, ranges, stack, pot,
   menus)? Should step 8 wait for the phase 4 flop gate, or first generate turn and river
   spots for the same scenario?
2. Coverage rule for shipped strategies: the start-street cut (recommended, about 150 MB
   for 25 flops), or `action_depth(N)` with N given?
3. Generate the library in CI (an artifact; needs a runner with enough memory and time,
   the same question as phase 4 open question 8) or on Caleb's machine under WSL2?
4. New crates, both MIT or Apache-2.0, pinned exact: `serde_json` (JSON export, required)
   and `rusqlite` with `bundled` (index, required). No binary-format or property-test
   crate is proposed.

## Decisions

None yet. Format: `* 2026-09-09: question. Answer: ...`
