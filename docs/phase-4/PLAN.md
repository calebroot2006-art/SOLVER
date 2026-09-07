---
project: gto-solver-app
type: plan
status: proposed
date: 2026-09-06
---

# Turn and flop solver: the plan

Research: `docs/research/solver-algorithms.md`, `docs/research/how-to-build-a-solver.md`,
`docs/astra/phase-3/PLAN.md` (the accepted river solver), and the fact sheet and researcher
report gathered on 2026-09-06 (summarised where they bind a step).

Written with Caleb on 2026-09-06. The plan below is what was agreed; "Decisions" holds his
answers to the open questions, with the date each one was given. Nothing else is assumed.

Who builds: every step is built by `executor` (Claude Opus) in a worktree. The main session
plans and reviews. Steps 3, 4, 6, 7, 9, and 10 are solver-core numerical code: the main
session reruns their accuracy gates itself before acceptance (`docs/ROADMAP.md:153-156`).

## Progress (updated 2026-09-06)

Read this first when picking the work up. It says what is done and verified, what is half
done, and what was learned that the plan below did not know. The executor updates it after
every step it finishes; the main session updates it after review.

**Where it stands (saved 2026-09-06, session limit hit mid-review):** steps 1, 2, and 5a
are built on three executor branches, none merged yet. Resume by reviewing and merging them
into `solver/phase-4`, in the order 2, 1, 5a.

* **Step 2** on `origin/worktree-agent-a7fb0c6c1c47171f6` at 9cbb479, CI run 34061377609
  all six jobs green. A reader summarised the diff: files stay inside crates/postflop,
  config/solver.toml and this plan; mask pool dedupes on the exact bit pattern of both
  players' masks, pool order deterministic; the two chance read sites keep operand order;
  Budget/Lease moved with one message change ("river" dropped); the threads>1 rejection and
  its test case removed; `Precision` added with default f64. Fresh river captures match the
  accepted `measured/2930550/` record in every solved field; only `working_set_bound_bytes`
  and `reserved_bytes` grew by 24 bytes (the pool's Vec header). Decision pending for the
  main session: leave the accepted record as a snapshot at its commit (recommended) or
  refresh it. `/code-review` was started on the branch and did not complete (limit).
* **Step 1** on `origin/worktree-agent-ae2bfd64bf170c0c6` at 0c368e5, CI run 34061497535
  all six jobs green, `river.rs` blob unchanged, 11 new tree tests. Not yet reader-reviewed.
  The executor's findings to check at review: the plan's per-street anchor of 18 decision
  nodes is really 16 (memory table is conservative, no change needed); gate-menu counts
  are flop [10, 50, 384] decision nodes and [5, 25, 209] live continuations, 1,267 compact
  nodes, using 60% raises and 33%/75% river bets, which the Decisions do not fix (confirm
  with Caleb or record as the default); `PostflopNode::street()` was added beyond the
  listed API; contributions are cumulative from the root; called all-ins are not live
  continuations. Its Noticed list (eight items) is in the executor report and matters for
  step 3, especially: per-street counts are not uniform at deep bases, so the estimate must
  sum built counters; chance nodes carry no card or probability; `max_nodes` bounds only
  the compact tree; two copies of the rounding helpers exist in river.rs and postflop.rs.
* **Step 5a** on `origin/worktree-agent-a5e67e5e396d18066` at 102316c, seven commits,
  worktree clean and pushed; the executor was cut off by the session limit while waiting
  for its final CI run, so the CI result for 102316c is unknown. Read it with
  `python docs/astra/development-takeover/ci_status.py` before review. Commit fd5dc6e
  applied the approved ranges and the flop sampler; commits 3173533 and 576fddc assert the
  reference merges isomorphic runouts, which the compare step must account for.
* Two facts learned: this machine does compile and test Rust (both executors ran cargo
  locally), contrary to the "GitHub Actions is the compiler" note; keep CI as the gate but
  local cargo is available for executors. And chance masks in Leduc pool 30 pairs to 6
  entries, harmlessly.
* After the three merges: run `git worktree remove` on each, delete the remote branches,
  push `solver/phase-4`, then brief the step 3 executor with the fact sheet, the step 1 and
  step 2 Noticed lists, and Decisions 1 to 9.

Step 2: done on `worktree-agent-a7fb0c6c1c47171f6`. Mask pool, `src/memory.rs`, unrestricted
`threads`, and a `precision` key accepting only `"f64"`. CI green on both OSes. The river
captures reproduce every solved value in `measured/2930550/` exactly: iterations, stop
reason, exploitability, root EVs, best responses and every strategy cell. Two bookkeeping
fields do move. `working_set_bound_bytes` and `reserved_bytes` are 24 bytes higher in every
case, which is the `mask_pool` vector header that `RiverMemory::estimate` charges through
`size_of::<TraversalLayout>()`. Adding a field to that struct cannot avoid it and the
estimate is a bound on what is retained, so the honest number went up; the accepted record
was left alone for the main session to decide on. Three things the step did not know. This
machine runs `cargo check`, `test`, `clippy` and `fmt` after all, so only the desktop app
needs CI. Leduc has five chance nodes and 30 (node, outcome) pairs that pool to six entries,
not one entry each as the step assumed. And `Traversal` in `cfr.rs` holds `&mut dyn
TerminalEvaluator` over one shared `ShowdownScratch`, so step 4 needs an evaluator per worker
before anything can be `Sync`.

Step 1: done and CI-verified. `crates/tree/src/postflop.rs` adds `Street`, `PostflopTreeConfig`, `PostflopNodeKind`, `PostflopNode` and `PostflopTree` beside the untouched `RiverTree`, with `tests/postflop.rs` proving a river-start tree is node-for-node identical to `RiverTree` on the three reference fixtures; contributions are cumulative across streets and raise-to multipliers scale the current street's wager, the anchor street measures 16 decision nodes (not the plan's approximate 18) and 9 live continuations, and per-street counts are not uniform because a deep raise target can merge into the all-in, so step 3's memory estimate must read the built tree rather than multiply one block's anchors.

## Task

Extend the accepted river-only solver (`crates/postflop`, `crates/tree`) to turn trees (one
chance node) and then flop trees (two chance nodes), meeting the `docs/ROADMAP.md:139-156`
gate: a 100bb single-raised-pot flop tree with two bet sizes solves under 0.5% of pot, peak
memory is measured against the 16 GB target, f32 and 16-bit storage are each measured
against an f64 baseline, flop frequencies are cross-checked against the pinned wasm-postflop
on the 49-flop subset, all-in and pending-call fixtures pass, and suit merging is skipped
when a range breaks the symmetry.

## Approach

Extend `crates/tree` and `crates/postflop` in place, not new crates. Both traversals already
implement `NodeKind::Chance` with per-outcome masks and probabilities (`cfr.rs:357-384`,
`best_response.rs:187-220`), and `Cfr`, `Strategy`, `drive`, `Budget`/`Lease`, NaN poisoning
and the `TerminalEvaluator` boundary are `pub(crate)` generics over `TraversalLayout`; a new
crate would either duplicate them or force them public. The `river/` module stays untouched
so every river gate remains a bit-identical regression check.

Shape: a compact street-aware betting tree (`PostflopTree`, one abstract `Chance` node per
street transition) is expanded at game construction into a `TraversalLayout` where every
runout's subtree gets its own contiguous `NodeId` range. That respects the existing contract
"distinct public histories use distinct nodes" (`game.rs:28-32`), lets the two walks run
unchanged on chance nodes, gives rayon disjoint accumulator slices per runout, and lets the
`TerminalEvaluator` stay `NodeId`-keyed (node to runout to per-runout `ShowdownTable`). An
all-in called before the river is expanded as chance nodes down to a showdown terminal with
no decision rows, so the existing sweep is reused exactly.

Sequence, per the recorded decisions (`solver-algorithms.md:198-202`): turn in f64,
validated by exploitability and the reference comparison; then f32 on the turn; then the
storage refactor the flop needs; then the flop in f32, validated; then isomorphism and
16-bit, each measured against the f64 baseline where one fits.

### Memory finding that shapes the plan

Using the river's per-node accounting (`river/memory.rs:46-107`: rows = states x actions x
bytes, three solver arrays plus two snapshots), a street block with two non-all-in sizes,
one raise, and all-in has about D = 18 decision nodes, average A = 3 actions, and L = 9
live continuations to the next street (check-check, plus bet-call and raise-call for each
side and size). The executor computes exact counts from the built tree; these are the
planning anchors.

| Tree | Decision nodes | Entries, S = 1326 | Entries, S = 500 (in-range compaction) |
|---|---|---|---|
| Turn: 18 + 9 x 48 x 18 | 7,794 | 31.0 M | 11.7 M |
| Flop: 18 + 9 x 49 x 18 + 9 x 49 x 9 x 48 x 18 | 3,437,172 | 13.7 G | 5.16 G |

Per array: turn at S = 1326 in f64/f32/i16 is 248/124/62 MB (S = 500: 94/47/23 MB); flop at
S = 1326 is 109/55/27 GB, at S = 500 is 41/21/10 GB. Multiply by five under today's
accounting, by three after step 6 (regrets, strategy sums, one snapshot). Suit isomorphism
saves nothing on rainbow flops (every suit is distinguishable), about 25 to 30% on two-tone
flops, about 55% on monotone flops, so the 16 GB budget must be met on rainbow flops without
it.

Consequences. (a) The flop gate with L = 9 per street does not fit 16 GB at any precision in
this layout. With a menu of one non-all-in size plus all-in on flop and turn (L = 5) the
flop has about 1.06 M decision nodes and 1.59 G entries at S = 500: three i16 arrays are
9.5 GB and f32 is 19 GB. The gate menu is therefore open question 1. (b) Compacting private
states to in-range combos (S from 1326 to roughly 500) is implied but not stated by the
roadmap; it is lossless and required, so it is step 6 and is flagged here as added scope.
Fixed costs are small: per-runout showdown tables are 2,352 x about 40 KB = 94 MB on the
flop and 48 x 40 KB on the turn; shared per-card chance masks are 52 x 2 x S x 8 B, about
1 MB; per-thread traversal buffers are about 4 MB. Topology metadata under the current
`Node` plus `Vec<Vec<Real>>` layout is about 150 B per node: 2 MB for the turn, 0.4 to
1.3 GB for a flop, which is why step 6 also flattens it.

### Facts established by the researcher (2026-09-06)

* kdub0/hand-isomorphism is not MIT or Apache. Its `LICENSE.txt` is a modified BSD-style
  notice with an unfilled "<organization>" placeholder and an acknowledgement clause. The only
  Rust wrapper found (cleverpiggy/poker-hand-indexer) embeds the same C source and carries no
  licence file. So step 9 is written from the published algorithm, not the code, unless Caleb
  decides otherwise (open question 6). The C implementation is one 20 KB file plus headers,
  so a from-scratch Rust version is a few hundred lines.
* The pinned reference (b-inary postflop-solver, AGPL, read for approach only, never linked)
  solves flop, turn, and river from a three-card flop. In the wasm-postflop binding the street
  is inferred from board length (3, 4, or 5 cards) and bet sizes arrive as separate per-street,
  per-player strings (`oop_flop_bet`, `oop_flop_raise`, `oop_turn_bet`, ..., `ip_river_raise`,
  plus turn and river donk sizes). Its README states isomorphic turn and river deals are
  combined into one, that it uses 32-bit floats with 64-bit summation temporaries, and that it
  offers 16-bit storage with a per-node 32-bit scale. How its main CFR walk is parallelised was
  not verified. The wasm-postflop repository is marked development suspended, which is fine
  for a pinned build.
* Neither repository publishes a 49-flop subset. The subset must come from our own research
  notes or be chosen by Caleb (open question 1).

## Steps

**1. `PostflopTree` in `crates/tree`.** Independent of step 2.
Files: create `crates/tree/src/postflop.rs`; edit `crates/tree/src/lib.rs:1-18` (docs and
re-exports). `RiverTree` unchanged.
Changes: `Street {Flop, Turn, River}`; `PostflopTreeConfig` = the `RiverTreeConfig` fields
(`docs/astra/phase-3/tree-contract.md`) plus `start_street` and `sizes: [[BetSizeOptions; 2]; 3]`
per street; `PostflopNodeKind::{Decision{player}, Chance{next: Street}, Terminal(Terminal)}`;
the same all-in and pending-call rules (`tree-contract.md:60-80`): an all-in called before the
river becomes `Chance` nodes down to a `Showdown` terminal with no decisions. Root is OOP with
no wager. `PostflopTree::{new, config, root, nodes, node, max_depth, storage_bytes,
decision_nodes_per_street, live_continuations_per_street}`; the last two feed the memory
estimate and the README maths.
Tests: `start_street = River` yields a node-for-node identical tree to `RiverTree` on the three
river fixtures; each chance node has exactly one child block; stack and contribution
invariants hold at every terminal; `max_nodes` and syntax bounds still enforced.
Gate: `check` job (`cargo test --workspace`, clippy), no new job.

**2. Shared plumbing in `crates/postflop`.** Independent of step 1.
Files: `src/game.rs:74-83` (`Node`, `TraversalLayout`), `src/cfr.rs:357-384`,
`src/best_response.rs:187-220`, `src/config.rs:9-47`, `src/river/memory.rs:109-155`, new
`src/memory.rs`, `src/lib.rs`.
Changes: (a) chance masks stored once per outcome card in a layout-level pool
(`TraversalLayout.mask_pool: Vec<[Vec<Real>; 2]>`, `Node.masks` becomes indices), so 2,352
chance nodes do not each hold 48 x 2 x 1326 reals; Kuhn and Leduc get one pool entry per
(node, outcome). (b) `Budget`/`Lease` move to `src/memory.rs` as `pub(crate)`, re-exported by
`river/memory.rs`. (c) `SolveConfig.threads` accepts any value (0 = available cores,
1 = serial, n = pool of n); the rejection at `config.rs:40-44` is removed and the doc at
`config.rs:18-20` rewritten. (d) `SolverConfig` gains `precision: "f64" | "f32" | "i16"`, with
only `f64` accepted until steps 7 and 10 land (rejected with a clear `SolveError::Config`).
Tests: existing `tests.rs` and `tests/river.rs` unchanged and passing; new unit test that two
chance nodes sharing a card share one pool entry.
Gate: `check` job; the river capture's measured policy hashes must equal the accepted
`tests/reference/river/measured/<sha>/` values bit for bit (arithmetic order is unchanged).
Kuhn, Leduc, OpenSpiel green.

**3. `PostflopGame`, turn start street, f64.** Depends on 1 and 2.
Files: create `src/streets/{mod.rs, game.rs, solver.rs, strategy.rs, memory.rs, terminal.rs}`;
edit `src/lib.rs`; `crates/bestresponse/src/lib.rs:8-27` (add a `streets` submodule
mirroring `river`).
Changes: `PostflopGame::new(board: &[Card] (3 or 4 cards), ranges, tree, options
{memory_limit_bytes, precision, threads})` mirrors `RiverGame::new` (`river/game.rs:53-165`):
validates board length against `start_street`, runs the pair-underflow check once on the
board-prefix weights (not per runout), then calls `PostflopMemory::estimate` before any row
allocation and returns `SolveError::MemoryLimit{required, limit}` if exceeded;
`memory_usage()` exposes the estimate publicly. Expansion is depth-first per runout so each
runout subtree is a contiguous `NodeId` range (recorded in `runout_ranges:
Vec<Range<NodeId>>`); chance probability is 1/(unseen cards minus the four private cards),
1/44 on the turn and 1/45 then 1/44 on the flop, so the contract at `game.rs:35-38` holds;
masks zero combos containing the outcome card. `PostflopTerminal` implements
`TerminalEvaluator` (`traversal.rs:5-17`) by mapping `NodeId` to (runout, payoff) and calling
the existing `ShowdownTable` and `evaluate_fold` with the per-runout table and dead set;
tables built once at construction and counted in `shared_bytes`. `PostflopMemory::estimate`
extends the river formula with runout count, chance-node fan-out, tables, mask pool, and
per-thread scratch and traversal buffers (times `threads`). `PostflopSolver` reuses `drive`
(`solver.rs`), cancellation between iterations, and `Cfr` poisoning. `PostflopStrategy`
returns per-history policies with the runout card in the history and carries reach so
consumers keep the rare-history context the phase 3 review required.
Tests (`crates/postflop/tests/streets.rs`): (i) river equivalence: `PostflopTree{start_street:
River}` on the three river fixtures produces bit-identical regrets, strategies and
exploitability to `RiverGame` after N iterations; (ii) all-in on the turn: the terminal vector
equals the mean over 48 rivers of the river all-in showdown vector, and both equal a
brute-force `evaluate_seven` enumeration; (iii) chance-probability sum contract; (iv) `Budget`
reservations match the estimate's components and release on drop; (v) a small turn fixture
reaches its target and the reported exploitability is a real measurement (stop reason names
the target, not the cap).
Gate: `check` job (the fixtures are small enough to run inside the 60-minute job).

**4. Rayon over runouts, deterministic.** Depends on 2; can be built against Leduc while step
3 is in progress, verified on the turn once 3 lands.
Files: `src/cfr.rs:330-449`, `src/best_response.rs:166-280`, `src/streets/solver.rs`,
`Cargo.toml` (rayon, MIT/Apache).
Changes: at a chance node with `num_outcomes > 1` and a pool, the walk maps outcomes in
parallel, each outcome computed serially by the same code, collected into
`Vec<Result<Vec<Real>>>` in outcome order, then reduced sequentially in outcome order; the
first `Err` by outcome index is returned (not whichever thread failed first). Accumulators are
split with `split_at_mut` along `runout_ranges` so each task owns its slice; `ShowdownScratch`
comes from a per-thread pool sized `threads`; `Traversal` becomes `Sync` over its shared
parts. Nested chance nodes (flop) nest `par_iter` naturally. `threads == 1` bypasses the pool.
Invariant: identical bits for any thread count.
Tests: Leduc and the turn fixture solved with threads 1, 2, 4 give identical strategy hashes
and exploitability; a poisoned terminal in runout 7 and runout 3 reports runout 3 regardless
of thread count.
Gate: `check` job on both OSes (the Windows and Linux runners have 4 cores).

**5a. Turn reference tooling and CI.** Independent of 3 and 4 (Python and YAML only).
Files: create `tests/reference/turn/{cases.json, capture.py, capture.mjs, compare.py,
review_combos.py, oracle.py, README.md}` by copying and adapting the river set
(`tests/reference/river/`); edit `.github/workflows/ci.yml` (new `turn-reference` job cloned
from lines 185-225 against the same pinned commits `97360db`/`9d1509f`; new `turn-solve` job,
matrix both OSes, 120-minute timeout, running the step 5b example and recording peak RSS via
`/usr/bin/time -v` on Linux and `Get-Process` on Windows).
Changes: `cases.json` schema adds `street`, a four-card `board`, and per-street `bets` and
`raises` for each player; the capture driver passes a four-card board (the binding infers the
street from board length) and the per-street size strings listed under Facts; compare output
rows carry the runout card in `history`; the two-percentage-point per-row rule and
`per-combo-review.json` with `review_reasoning` are unchanged (`docs/astra/phase-3/PLAN.md:104-105`).
Gate: `turn-reference` job fails on any row over two points lacking a committed
`review_reasoning`.

**5b. Turn gate capture.** Depends on 3, 4, 5a.
Files: create `crates/postflop/examples/turn_capture.rs` mirroring `river_capture`;
`tests/reference/turn/measured/<sha>/`.
Changes: solves the gate turn fixtures in f64 to the turn target (open question 3), emits
policies, exploitability, iteration count, stop reason, estimate, and peak RSS.
Gate: `turn-solve` (exploitability under target, RSS printed) plus `turn-reference`
(per-combo review complete). Acceptance = main session reruns both.

**6. Flat layout and in-range compaction.** Depends on 5b acceptance. Touches `cfr.rs` and
`strategy.rs`, so it is serial with 7 and 10.
Files: `src/game.rs` (`TraversalLayout`, `Node`), `src/strategy.rs`, `src/cfr.rs`,
`src/streets/{game,strategy,memory}.rs`, `src/river/strategy.rs` if it indexes `rows`.
Changes: `states[p]` = number of combos with positive weight not blocked by the board prefix;
compact-to-`Combo::id` index tables per player; scatter and gather at the terminal boundary so
`ShowdownTable` and `evaluate_fold` still see `[f64; 1326]` (the sweep and its tests are
unchanged). Rows for all nodes live in one contiguous buffer with a per-node offset table
(u64 offsets); topology becomes struct-of-arrays (kind, first child, child count, payoff
index, runout). Current policy is derived from regrets at visit time instead of stored,
dropping one array. The memory maths is documented in `crates/postflop/README.md` (numerical
layout section, `README.md:115-117`).
Invariant: numerically identical results for the river and turn (same arithmetic order per
node).
Tests: river and turn measured hashes unchanged; a range with 37 combos produces
`states == 37`; estimate versus `Budget` still matches.
Gate: `check` and `turn-solve`.

**7. f32 storage on the turn.** Depends on 6.
Files: `src/cfr.rs` (storage trait `Precision` with `F64` and `F32` impls; arithmetic in f64
inside a node update, stored rounded), `src/config.rs` (accept `"f32"`),
`src/streets/memory.rs` (bytes per precision), `examples/turn_capture.rs` (`--precision`),
`tests/reference/turn/compare.py` (f32-versus-f64 report).
Invariant: exploitability is always computed by the best-response walk in f64 from decoded
strategies; the report states max and mean absolute frequency difference against the f64
baseline and both exploitabilities.
Tests: f32 turn solve under target; difference report produced; a synthetic regret of 1e-30
that underflows f32 fails loudly through `finite` and `reach_product` (`error.rs:63-118`),
never silently.
Gate: `turn-solve` runs f64 and f32 and uploads the comparison; main session reruns.

**8. Flop start street, f32, gate and reference.** Depends on 6 and 7.
Files: `src/streets/game.rs` (nested expansion, `start_street: Flop`),
`examples/flop_capture.rs`, `tests/reference/flop/` (49-flop `cases.json`, sharded),
`.github/workflows/ci.yml` (three additions: a `flop-smoke` step in `check` solving a small
flop tree in under 10 minutes; a `flop-gate` job on ubuntu-latest with 16 GB, 360-minute
timeout, `workflow_dispatch` and commit-message opt-in, solving the gate tree in f32 and
recording exploitability, estimate and peak RSS; a `flop-reference` matrix job of seven
shards of seven flops plus an aggregating `flop-review` job applying the two-point rule).
Invariants: memory estimate before allocation, refusal on exceed; exploitability under 0.5%
on the gate tree; peak RSS under 16 GB minus the headroom in open question 4.
Tests: flop all-in fixture equals the mean over 2,352 runouts of river showdown vectors
(brute force); nested chance probability contract; `start_street: Turn` inside a flop tree's
sub-solve reproduces step 3's turn results for that runout.
Gate: `flop-smoke` in `check`; `flop-gate`; `flop-reference` and `flop-review`. The f64
baseline for the full gate tree cannot fit 16 GB (41 GB per array), so f32-versus-f64 on the
flop is measured on a reduced flop tree that fits in f64 (one size, ranges under 200 combos)
and on the turn gate tree; the full gate tree compares f32 against i16 in step 10. Flagged in
Risks.

**9. Suit isomorphism, written from the algorithm.** The cards-crate part is independent of
6 to 8; integration depends on 8.
Files: create `crates/cards/src/isomorphism.rs` (plus tests), edit `crates/cards/src/lib.rs:13-19`;
then `src/streets/game.rs`, `src/streets/memory.rs`, and a note in `docs/research/` recording
the licence finding and the decision.
Route B (from scratch, licence-clean, the default): compute the suit permutation group fixing
the board prefix; check both ranges are invariant under each generator (weights equal after
permuting suits); if not, merging is disabled and the solve logs why; otherwise group runout
cards into orbits, expand only one representative per orbit with chance probability
multiplied by orbit size, and answer strategy queries on a merged card by permuting the combo
index. Route B needs no full hand index and is what the roadmap's "suit merging skipped when a
range breaks the symmetry" describes.
Route A (port of kdub0's canonical index, only if Caleb accepts its BSD-style terms with the
acknowledgement clause; open question 6): a Rust port of the canonical hand index and its
inverse in `crates/cards/src/isomorphism.rs`, property-tested against a brute-force
canonicaliser that applies all 24 suit permutations and takes the lexicographic minimum.
Invariant (either route): the best-response walk always runs over the full, unmerged runout
set against the expanded strategy, so a wrong merge shows up as exploitability, never as a
plausible strategy.
Tests: monotone and two-tone flops: merged solve exploitability under target when measured
unmerged; a range that adds only `Ah`-suited combos disables merging (asserted by
`merged_runouts == full_runouts`); a rainbow flop reports zero merges; property test against
the brute-force canonicaliser.
Gate: `check`; `flop-gate` rerun with merging shows the memory reduction in the estimate and
in RSS.

**10. 16-bit compression.** Depends on 7 (and 6); serial with 7.
Files: `src/cfr.rs` (`I16` storage: regrets as i16 with a per-node f32 scale, strategy sums as
u16 with a per-node scale, DCFR discounting applied to the scale not the entries),
`src/config.rs` (accept `"i16"`), `src/streets/memory.rs`, `crates/postflop/README.md`
(derived bound and measured error, per `solver-algorithms.md:156-158`), capture examples.
Invariant: decode(encode(x)) error at most scale/2 per entry, asserted in a unit test;
overflow of a scaled value fails loudly.
Tests: round-trip bound; turn i16 solve under target with difference report against f64; flop
gate i16 versus f32 report.
Gate: `turn-solve` (three precisions) and `flop-gate`; main session reruns the
compressed-error check itself.

**11. Docs and handoff.** Depends on all.
Files: `crates/postflop/README.md`, `crates/tree/README.md`, `docs/phase-4/tree-contract.md`
and `docs/phase-4/reference-contract.md` (new, mirroring phase 3), `docs/ROADMAP.md`
progress, `ASTRA-UPDATE.md` listing the typed contracts the app will need as open items only
(turn and flop solve request, runout-aware strategy query with reach context, memory estimate
display, progress with runout count).
Gate: `format` job; `slopcheck.py` clean.

## Tests

* Known solutions: Kuhn and Leduc (`tests/`) and OpenSpiel stay in `check` and must pass after
  every step; Leduc is also the determinism fixture for step 4.
* River regression: measured river hashes bit-identical after steps 2, 4, 6, 7, 10 (the river
  stays f64 unless configured otherwise).
* River equivalence of `PostflopGame{start_street: River}` (step 3) is the strongest single
  check that expansion and terminal mapping are right.
* All-in and pending-call: turn and flop all-in values equal brute-force enumeration over
  runouts.
* Exploitability: computed in f64 over the full runout set for every solve, every precision,
  merged or not; reported as a percentage of pot with the stop reason.
* Reference: turn and flop per-combo review, two points per row, no aggregate waiver.
* Memory: estimate versus `Budget` accounting in tests; peak RSS on the 16 GB CI runners in
  `turn-solve` and `flop-gate`; refusal test with a limit one byte below the estimate.
* Determinism: thread counts 1, 2, 4 identical.
* Most likely failure path: a NaN or underflow inside one runout on one thread. Test: a
  poisoned terminal in a chosen runout yields `SolveError` naming that runout
  deterministically, with no partial strategy exposed
  (`failed_iteration_never_exposes_partial_strategy` generalised).

## Risks and edge cases

* **The flop gate does not fit 16 GB with an L = 9 menu** (see the table). Check:
  `PostflopMemory::estimate` printed by `flop-smoke` for the chosen menu before any long
  solve; open question 1.
* **Isomorphism saves nothing on rainbow flops**; the research note's "roughly halves" is an
  average. Check: estimate with and without merging logged per flop in `flop-reference`.
* **A wrong chance probability or mask** produces plausible strategies. Check: contract test
  (sum = 1 per compatible pair), river equivalence, and the reference comparison.
* **A non-deterministic parallel reduction** hides tiny drifts. Check: hash equality across
  thread counts, first-error-by-index test.
* **The f64 baseline is unavailable on the full flop tree.** Check: baseline comparisons on
  the turn gate and a reduced flop tree; recorded in the README so nobody reads the
  f32-versus-i16 comparison as f64-validated.
* **Per-node `Vec` allocation churn** in a multi-million-node traversal may make flop
  iterations too slow for CI. Check: iteration time logged; if the gate tree cannot converge
  in the 360-minute job, replace per-node returns with the depth-indexed arena already
  counted in `traversal_bytes` (no numeric change; hash regression proves it).
* **Cancellation latency** equals one flop iteration (tens of seconds). Acceptable for
  phase 4; noted for the app contract.
* **Rare-history conditional values** (phase 3 review): `PostflopStrategy` carries reach and
  runout so a consumer cannot quote a river policy without context.
* **The pinned reference is a suspended project.** Fine for a pinned commit; if a future
  toolchain cannot build it, the river capture breaks first and says so.
* **i16 overflow or scale underflow** during DCFR discounting. Check: loud failure test;
  per-node scale recomputed each update.
* **A range that breaks symmetry** silently merged. Check: explicit invariance test per
  generator; merging disabled with a logged reason.

## Open questions

1. Bet-size menus, raise counts, and `add_all_in_threshold` for the turn gate fixtures, the
   flop gate tree, and the 49-flop cross-check, plus which 49 flops. The flop gate is
   feasible on 16 GB only with about five live continuations per street (one non-all-in
   size plus all-in on flop and turn, or two sizes with no raises); two non-all-in sizes
   plus a raise on every street is three times over budget even at i16 with compaction.
2. Ranges for the gate trees (ours, per `docs/research/README.md:37`): which positions and
   roughly how many combos.
3. Per-street accuracy targets: flop 0.5% is fixed by the roadmap; the turn target (0.5% or
   tighter, since it is cheap) and the reduced-flop f64 baseline target are unstated.
4. Default `memory_limit_bytes` and the desktop headroom subtracted from 16 GB (the estimate
   refuses above the limit; a default such as 12 GiB is a product decision).
5. Do turn solves ship as a user feature before the flop, or is the turn only an internal
   milestone? This decides whether step 11's app contracts include a turn-solve request now.
6. Isomorphism route: B (from scratch, licence-clean, smaller) is the default. Does Caleb
   want route A (port kdub0 under its BSD-style terms with the acknowledgement clause)
   instead?
7. May `flop-gate` and `flop-reference` run only on demand (manual dispatch or a commit tag)
   rather than on every push, given the 360-minute runtime?

## Decisions

All given by Caleb on 2026-09-06, in multiple-choice form.

1. **Gate menu:** flop and turn get one bet size, 33% pot, plus all-in, with one raise; the
   river keeps two sizes plus one raise. `add_all_in_threshold` and `force_all_in_threshold`
   stay at the phase 3 fixture values unless the estimate forces a change (record it here).
2. **Ranges:** button open versus big blind call at 100bb, single-raised pot. The ranges are
   ours: a reader drafts them from `docs/research/preflop-charts-and-ranges.md`, Caleb
   approves the text before it enters `cases.json`. No vendor chart is copied.
3. **Turn target:** 0.25% of pot. The reduced-flop f64 baseline uses the same 0.25%.
4. **Memory limit default:** 12 GiB. The estimate refuses above the configured limit; the
   configured default is 12 GiB and the hard ceiling stays 16 GiB.
5. **Turn is an internal milestone only.** No turn-only app contract in phase 4; Astra
   receives turn and flop contracts together in step 11.
6. **Isomorphism route B:** written from the published algorithm, property-tested against
   brute-force suit permutation. No kdub0 code is ported.
7. **Long flop jobs run on demand only:** `workflow_dispatch` or a commit-message tag. The
   ten-minute `flop-smoke` step runs in `check` on every push.
8. **49-flop subset (revised 2026-09-06):** the reader found no recorded list; the 25/49/85/184
   subsets are a GTO Wizard product feature, not a published list. Caleb chose to generate
   our own: a script enumerates the 1,755 canonical flops and samples 49 across textures
   (paired, monotone, two-tone, rainbow, connected, dry, high and low) with a fixed seed,
   committed as `tests/reference/flop/flops.json` with the script beside it.
9. **Gate ranges approved (2026-09-06),** hand-drafted, ours, in the parser's syntax:
   BTN open: `22+, A2s-AKs, K2s-KQs, Q3s-QJs, J5s-JTs, T6s-T9s, 96s-98s, 86s-87s, 75s-76s,
   65s, 54s, A3o-AKo, K8o-KQo, Q9o-QJo, J9o-JTo, T9o`.
   BB call: `22-TT, JJ:0.5, QQ:0.25, A2s-AJs, AQs:0.5, K2s-KQs, Q4s-QJs, J6s-JTs, T6s-T9s,
   96s-98s, 85s-87s, 74s-76s, 64s-65s, 53s-54s, 43s, A2o-AJo, K9o-KQo, Q9o-QJo, J9o-JTo,
   T8o-T9o, 98o`.
   OOP is the BB caller, IP is the BTN opener. The preflop pot at 100bb with a 2.5bb open
   and a call is 5.5bb (blinds only, no ante); effective stack behind is 97.5bb. If the
   parser rejects any token, the executor reports it rather than rewriting the range.
