# Phase 4 step 3 and Decision 11: merged, ready for Astra's review

Written by Claude on 2026-09-09. Branch `solver/phase-4` at d1d6622, pushed. Merge commits
f3f55b7 (step 3) and abcb86d (Decision 11).

## The problem and the expected behaviour

Step 3 extends the accepted river solver to turn-start and flop-start trees in f64:
`PostflopGame` expands a compact street-aware tree into one traversal layout with a
contiguous node range per runout, chance probability 1/(unseen cards minus four) with
per-card masks, showdown tables interned per completed board, a memory estimate that
refuses before allocating, and the structural validation of every expanded tree. Decision
11 prunes the wasm-postflop reference with upstream's `removed_lines` so it solves exactly
our one-raise tree.

Expected: a river-start `PostflopGame` reproduces `RiverGame` bit for bit; called turn and
flop all-ins equal a brute-force seven-card enumeration; the small turn fixture reaches the
0.25% target with a measured stop; the estimate matches every reservation; the gate flop
tree is refused under 12 GiB; all three reference cases reach 0.25% at `max_raises: 1`.

## What to review

* Code: `crates/postflop/src/streets/**`, `crates/postflop/src/game.rs`
  (`validate_traversal`, `PairScope`), `crates/postflop/src/config.rs` (one 16 GiB
  ceiling), `crates/postflop/src/streets/memory.rs` (the bound), `crates/bestresponse`
  (`streets` module), `crates/postflop/tests/streets.rs`.
* Reference: `tests/reference/turn/raise_cap.py`, `test_raise_cap.py`, `capture.py`,
  `cases.json`, `README.md`.
* Plan: `docs/phase-4/PLAN.md` revision 2 (state table, History) and the two new plans
  `docs/phase-5/PLAN.md` and `docs/phase-6/PLAN.md`, which need your plan review before
  their executors start.

## Commands and results

| Check | Where | Result |
|---|---|---|
| `check` job (clippy `-D warnings`, `cargo test --workspace`, river capture and independent evaluation) | CI run 34393031096 at 7522b91, both OSes | success |
| `turn-reference` at `max_raises: 1` with pruning | same run; first at run 34285342323 (4147355) | success; 0.1587/0.1783/0.1851% of pot |
| River regression against `tests/reference/river/measured/2930550/` | main session, artifact of run 34388450561 (ubuntu), 17,518 fields per capture | identical except `elapsed_seconds`, `project_revision`, and `working_set_bound_bytes`/`reserved_bytes` at exactly +24 bytes |
| Small turn fixture | CI test `a_small_turn_solve_reaches_a_measured_target_rather_than_the_cap` | 0.195407% of pot, `nash_conv` 0.039081 chips, 50 iterations, `TargetReached` |
| River equivalence | CI test `a_river_start_game_reproduces_the_river_solver_bit_for_bit` | pass |
| Brute-force all-in, turn and flop start | CI tests `a_called_turn_all_in_...` and `a_called_flop_all_in_...` | pass, under 1e-9 chips |
| Gate flop tree refusal | CI test | 89,793,081,162 bytes required, refused under 12 GiB |
| Merged branch CI | run on d1d6622 | see `ASTRA-UPDATE.md` (in progress at writing time) |

Review rounds: round one (nine findings) closed at de8219a; round two (nine findings from a
reader fact sheet and an eight-angle `/code-review`) closed at 940ec23; round three (three
findings from the `/code-review` of the round-two delta) closed at 7522b91. All three
lists are in `docs/phase-4/PLAN.md` under History and the resolved note.

## Known limitations and open items

* The traversal is still serial; `threads` is resolved and charged but step 4 wires the pool.
* The zero-sum and chance-mass checks are size gated (512 x 512 pairs, 200,000 nodes,
  100,000 columns); a gate tree gets the structural walk only and reports zero for the
  rest. `PostflopValidation` says which budget stopped which check.
* The `Layout` callback path (toy games, `Strategy::from_rows`) holds a compatibility
  table, terminal kernels, and the zero-sum matrix that no estimate charges; the river
  solve does not use that path. Recorded for step 6.
* The plan's open questions 8 (flop gate host) and 9 (your product proposals) wait on
  Caleb.
* Nothing in `app/` changed. No files are being edited at the time of writing.
