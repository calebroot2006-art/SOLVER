---
project: gto-solver-app
type: plan
status: in progress
date: 2026-09-06
revision: 2
---

# Turn and flop solver: the plan

Research: `docs/research/solver-algorithms.md`, `docs/research/how-to-build-a-solver.md`,
`docs/astra/phase-3/PLAN.md` (the accepted river solver), and the fact sheet and researcher
report gathered on 2026-09-06 (summarised where they bind a step).

Written with Caleb on 2026-09-06. The plan below is what was agreed; "Decisions" holds his
answers to the open questions, with the date each one was given. Nothing else is assumed.

Revision 2 (2026-09-09) answers Astra's plan review,
`docs/reviews/2026-09-09-astra-phase4-plan-review.md`, findings R1 to R8. Steps 4 to 11
changed; steps 5c and 5d are new; the Progress section is now a state table plus a History.
Nothing in Caleb's decisions changed. Two questions that revision opened are his to answer
(open questions 8 and 9).

Who builds: every step is built by `executor` (Claude Opus) in a worktree. The main session
plans and reviews. Steps 3, 4, 6, 7, 8, 9, and 10 are solver-core numerical code: the main
session reruns their accuracy gates itself before acceptance (`docs/ROADMAP.md:153-156`).
Step 8 is on that list because a flop-start expansion changes numerical behaviour even
though its arithmetic is step 3's.

## Progress (updated 2026-09-09, plan revised, step 3 round two pending)

Read this first when picking the work up. The state table says where every step is, on
which revision, and what accepts it. "Next actions" is the order of work. "History" keeps
the earlier review outcomes and executor reports as written, because they hold facts the
steps below depend on.

### State table

Base revision for new work: `solver/phase-4` at 8b892f3 (Astra's review commit on top of
b593f40), clean. Executor branches listed by their pushed name; the local worktree that
holds each head is in brackets.

| Step | State | Revision and branch | Gate | Acceptance |
|---|---|---|---|---|
| 1 `PostflopTree` | Merged | 7f19a4d on `solver/phase-4` | `check` | Accepted 2026-09-07 |
| 2 Shared plumbing | Merged | 281c661 on `solver/phase-4` | `check`, river hashes | Accepted 2026-09-07 |
| 5a Turn reference tooling | Merged | 0a076b6 on `solver/phase-4` | `turn-reference` | Accepted 2026-09-07 |
| 3 `PostflopGame`, turn, f64 | Built, round-two findings open | `origin/worktree-agent-a53c5f7bb472f1e9a` at de8219a [`.claude/worktrees/agent-a0b0ee81b7b660544`]; CI run 34286324836 green at bfb2162 | `check` | Not accepted: findings A to I below, then main-session rerun of the small turn solve and the brute-force all-in tests from the CI artifacts |
| Decision 11 reference pruning | Built and reviewed, unmerged | `origin/worktree-agent-a865507f49449f42b` at 4147355 (based on da6477d); CI run 34285342323 green | `turn-reference` | Accepted 2026-09-08; merges after step 3 |
| 4 Rayon over runouts | Not started | after 3 | `check` both OSes | Main-session rerun of the thread-count hash test |
| 5b Turn gate capture and joint comparison | Not started | after 3, 4 | `turn-solve`, `turn-reference`, new `turn-compare` | Main session reruns `compare.py` on the artifacts |
| 5c Storage lifetime and memory table | Not started | after 3 | `check` (table test) | Main session reconciles table, reservations, and measured RSS |
| 5d Job lifecycle contract | Not started (main session drafts) | after 3 | prose check; Astra review | Astra's review in `docs/reviews/` |
| 6 Flat layout and compaction | Not started | after 5b, 5c, 5d | `check`, `turn-solve` | Main session reruns river and turn hashes |
| 7 f32 storage | Not started | after 6 | `turn-solve` f64 and f32 | Main session reruns the difference report |
| 8 Flop start street and gate | Not started | after 6, 7; host per open question 8 | `flop-smoke`, `flop-gate`, `flop-reference`, `flop-compare` | Main session reruns the gate's exploitability and the comparison |
| 9 Suit isomorphism | Not started | cards part after 3; integration after 8 | `check`, `flop-gate` merged versus unmerged | Main session reruns the merged-versus-unmerged per-combo test and the unmerged exploitability |
| 10 16-bit compression | Not started | after 7; integrated after 9 is accepted | `turn-solve` three precisions, `flop-gate` | Main session reruns the compressed-error check |
| 11 Docs and handoff | Not started | after all | `format`, prose check | Astra review |

The worktrees `.claude/worktrees/agent-a53c5f7bb472f1e9a` (bfb2162, one commit behind the
branch head) and `agent-a5c19d7e711c4fc07` (763faba, phase 0) are stale and get removed at
integration.

### Next actions, in order

1. **Step 3 round two.** Spawn a fresh executor on de8219a (worktree
   `agent-a0b0ee81b7b660544`, pushes to `origin/worktree-agent-a53c5f7bb472f1e9a`) with
   findings A to I. Owned paths: `crates/**`, `config/**`, its own Progress paragraph.
   It does not touch `tests/reference/turn/`. Review the diff, rerun the accuracy checks,
   then merge into `solver/phase-4`.
2. **Merge Decision 11** (4147355) after step 3. Fix its README and plan paragraph to cite
   run 34285342323 (they cite 34284399884, which was the earlier green run). Its "cases are
   back at 32" paragraph is superseded: the cases are at `max_raises: 1` with pruning.
3. **Plan merge rule.** Both branches append paragraphs to this file's Progress section.
   At merge keep this revision's structure, put the executor's step 3 and Decision 11
   paragraphs under History, and update the state table rows. Then remove both executor
   worktrees, delete their branches, and push.
4. **Caleb's answers** to open questions 8 (flop gate host) and 9 (product milestones from
   Astra's review). Question 8 blocks only step 8's `flop-gate` job; everything up to step
   7 runs on standard runners.
5. **Brief step 4**, then 5b, 5c and 5d in parallel (disjoint files), then 6.

### Step 3 round-two findings (open, to fix on the branch before merge)

From the reader fact sheet and the eight-angle `/code-review` of da6477d..bfb2162.

* (A) Run the linear structural validation on every tree and gate only the quadratic
  mass and zero-sum checks, with a full-range small-tree test.
* (B) The split `validate_traversal` allocates a `states[0] * states[1]` pair matrix on the
  river and toy `Layout` path (14 MB at 1,326 states) that da6477d never did and no
  estimate charges. Stream it or charge it.
* (C) Report the number of pairs actually checked.
* (D) Document that the working-set bound covers one concurrent strategy query.
* (E) Charge the validation's `ShowdownScratch` and its stack doubling.
* (F) One 16 GiB constant instead of three.
* (G) Exact validation counts in the tests.
* (H) The validation stack term must use `max(52, max_actions)` per level.
* (I) NaN prefill and check in `PostflopColumns`.

Not now, for step 6: the river/streets duplication (`scaled`, root normaliser,
`evaluate_terminal`, `Payoff`, memory formulas, `from_rows`, `decision_values`,
`PostflopSolver` versus `RiverSolver`), `Payoff::Showdown([u, u])`'s symmetric pair, and
`showdown_tables` charging 2,352 ordered runouts where the build interns 1,176.

### Facts the later steps depend on

* **From step 3 (executor report, CI run 34286324836):** the estimate needs the per-street
  sum of action counts, not node counts, since menus differ inside one street block. Chance
  masks intern on the dealt card; showdown tables intern on the completed board's card set
  (a flop tree builds 1,176 tables). Runout ranges are derived per chance node:
  `PostflopGame::subtree(node)` and `outcome_range(chance, outcome)` partition that node's
  own subtree in outcome order, at every level. `PostflopNodeView` exposes `kind`, `street`,
  `contributions`, `actions`, `children`, `board`, `runout`, `possible_cards`,
  `chance_probability`, `compact_id`; `PostflopDecisionValues` carries street, board,
  runout, values, own reach, opponent mass. `threads: 0` resolves through
  `streets::resolve_workers` and the bound charges `workers + 1` traversal buffers. The
  12 GiB default is `memory_limit_mib = 12288` in `config/solver.toml`. The small turn
  fixture reaches 0.195407% of pot in 50 iterations against the 0.25% target. The gate flop
  tree asks for 89,791,021,146 bytes at f64 under today's five-array accounting and is
  refused under 12 GiB.
* **Local Rust is best-effort.** Smart App Control blocks `cargo.exe` on this machine
  while `rustc.exe` runs. CI is the compiler for every gate. Python tooling runs locally,
  so the main session can rerun `compare.py` and `oracle.py` on downloaded artifacts.
* **CI hosts.** This repository is private. GitHub's standard runners for private
  repositories are 2 CPUs and 8 GB (Linux and Windows); the 4-CPU, 16 GB tier applies to
  public repositories only, and larger runners need an organisation on a Team or
  Enterprise plan (GitHub runner reference, read 2026-09-09). Every job in `ci.yml` runs on
  standard runners today. The turn gate fits them (the turn tree is about 250 MB per f64
  array at 1,326 states). The flop gate does not; see open question 8.
* **Reference behaviour (step 5a, Decision 11):** with donk sizes unset the reference still
  gives OOP its ordinary river bet menu after calling a turn bet, so our tree must match.
  The reference merges isomorphic runouts whenever a suit permutation fixes the four-card
  board (12 on the paired board, 13 on the flush board, 0 on the rainbow one);
  `compare.py` only requires the `possible_cards` sets to agree, so merging on our side is
  step 9's choice, not a comparison requirement. `raise_cap.py` derives the 18
  `removed_lines` per case at capture time; all three cases reach 0.25% at
  `max_raises: 1` (0.1587/0.1783/0.1851% of pot). Export is capped at 64 MiB per case,
  three or four named runouts.
* **Step 2 review outcome.** The accepted river record in
  `tests/reference/river/measured/2930550/` stays as a snapshot at its own commit. Fresh
  captures on the merged code reproduce every solved field; only `working_set_bound_bytes`
  and `reserved_bytes` are 24 bytes higher (the `mask_pool` Vec header charged through the
  layout size). A new capture compared against that record must allow those two fields to
  differ by exactly 24 bytes and nothing else.

## Task

Extend the accepted river-only solver (`crates/postflop`, `crates/tree`) to turn trees (one
chance node) and then flop trees (two chance nodes), meeting the `docs/ROADMAP.md:139-156`
gate: a 100bb single-raised-pot flop tree with the approved menu solves under 0.5% of pot,
peak memory is measured against the 16 GB target on a host that actually has it, f32 and
16-bit storage are each measured against an f64 baseline where one fits, flop frequencies
are cross-checked against the pinned wasm-postflop on the 49-flop subset, all-in and
pending-call fixtures pass, and suit merging is skipped when a range breaks the symmetry.

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

Sequence, per the recorded decisions (`solver-algorithms.md:198-202`) and Astra's revised
order: turn in f64, validated by exploitability and a joint comparison against the reference
(5b); then the exact memory table and the job lifecycle contract (5c, 5d), which decide
whether the flop needs compression before its full gate; then the storage refactor (6); f32
on the turn (7); the flop in f32, validated on a reduced tree against f64 and on the full
gate against the reference (8); then suit merging (9) and 16-bit storage (10), each added
alone and checked against the accepted baseline before they are combined.

### Memory anchors (superseded 2026-09-09; kept for the arithmetic)

These were the 2026-09-06 planning anchors, computed before Decisions 1, 9, and 10 fixed
the menu and ranges. They no longer describe the approved tree. Step 3 measured the
approved gate tree at 89.8 GB for f64 under five arrays; step 5c produces the exact
component table that replaces this section for every later decision. Do not use these
numbers to argue that the gate does or does not fit.

Using the river's per-node accounting (`river/memory.rs:46-107`: rows = states x actions x
bytes, three solver arrays plus two snapshots), a street block with two non-all-in sizes,
one raise, and all-in has about D = 18 decision nodes, average A = 3 actions, and L = 9
live continuations to the next street. Step 3's built tree measures 14 decision nodes and
9 live continuations for that block.

| Tree | Decision nodes | Entries, S = 1326 | Entries, S = 500 (in-range compaction) |
|---|---|---|---|
| Turn: 18 + 9 x 48 x 18 | 7,794 | 31.0 M | 11.7 M |
| Flop: 18 + 9 x 49 x 18 + 9 x 49 x 9 x 48 x 18 | 3,437,172 | 13.7 G | 5.16 G |

Per array: turn at S = 1326 in f64/f32/i16 is 248/124/62 MB (S = 500: 94/47/23 MB); flop at
S = 1326 is 109/55/27 GB, at S = 500 is 41/21/10 GB. Suit isomorphism saves nothing on
rainbow flops, about 25 to 30% on two-tone flops, about 55% on monotone flops, so the
budget must be met on rainbow flops without it. With the approved menu (one size plus
all-in, one raise) the anchor count was about 1.06 M decision nodes and 1.59 G entries at
S = 500. Compacting private states to in-range combos is lossless and required (step 6).
Fixed costs are small: per-runout showdown tables about 40 KB each, shared per-card chance
masks about 1 MB, per-thread traversal buffers about 4 MB. Topology metadata under the
current `Node` plus `Vec<Vec<Real>>` layout is about 150 B per node, which is why step 6
also flattens it.

### Facts established by the researcher (2026-09-06)

* kdub0/hand-isomorphism is not MIT or Apache. Its `LICENSE.txt` is a modified BSD-style
  notice with an unfilled "<organization>" placeholder and an acknowledgement clause. The only
  Rust wrapper found (cleverpiggy/poker-hand-indexer) embeds the same C source and carries no
  licence file. Decision 6: step 9 is written from the published algorithm, not the code.
* The pinned reference (b-inary postflop-solver, AGPL, read for approach only, never linked)
  solves flop, turn, and river from a three-card flop. In the wasm-postflop binding the street
  is inferred from board length (3, 4, or 5 cards) and bet sizes arrive as separate per-street,
  per-player strings (`oop_flop_bet`, `oop_flop_raise`, `oop_turn_bet`, ..., `ip_river_raise`,
  plus turn and river donk sizes). Its README states isomorphic turn and river deals are
  combined into one, that it uses 32-bit floats with 64-bit summation temporaries, and that it
  offers 16-bit storage with a per-node 32-bit scale. How its main CFR walk is parallelised was
  not verified. The wasm-postflop repository is marked development suspended, which is fine
  for a pinned build.
* Neither repository publishes a 49-flop subset. Decision 8 generates our own.

## Gates

Every gate names its command, its artifact, what passes it, and who accepts it. A green
job is evidence; acceptance is the named reviewer's rerun.

| Gate | Command or job | Artifact | Passes when | Accepted by |
|---|---|---|---|---|
| `check` | `cargo test --workspace`, clippy `-D warnings`, both OSes | `cfr-traces-*`, `river-project-*` | All tests pass; river measured hashes unchanged | Main session reads the job; reruns river hashes locally on the artifact when the diff touches `cfr.rs`, `strategy.rs`, or `game.rs` |
| `turn-reference` | `capture.py`, `compare.py --reference` | `turn-wasm-reference` | Capture valid, every case reaches its target at `max_raises: 1` | Main session reads the summary |
| `turn-solve` | `turn_capture` example, both OSes, peak RSS recorded | `turn-project-*` | Every case under 0.25% of pot, stop reason names the target, RSS under the recorded host memory | Main session reruns `oracle.py` on the capture |
| `turn-compare` (step 5b) | `compare.py --project --reference --review` | `turn-comparison` | Both captures present, same inputs and revisions, both converged, no unexplained row over two points, no stale review | Main session reruns `compare.py` locally on the downloaded artifacts |
| `flop-smoke` (step 8, in `check`) | small flop tree under 10 minutes | in `cfr-traces-*` | Under target; estimate printed | Main session reads it |
| `flop-gate` (step 8, on demand) | `flop_capture` on the host of open question 8 | `flop-project` | Prerequisite host check passes; under 0.5% of pot; peak RSS under the limit plus headroom | Main session reruns exploitability from the capture with the best-response tool |
| `flop-reference`, `flop-compare` (step 8) | sharded captures, then the joint comparison | `flop-wasm-reference`, `flop-comparison` | Same rules as `turn-compare` over 49 flops | Main session reruns `compare.py` |
| Precision reports (steps 7, 10) | `compare.py --baseline` | in the solve artifacts | Both solves converged; rows over two points carry review reasoning; residuals stated | Main session reruns the report |
| Merge report (step 9) | merged-versus-unmerged per-combo test; unmerged exploitability | in `flop-project` | Per-combo equality within f64 tolerance; unmerged exploitability under target | Main session reruns both |

## Steps

**1. `PostflopTree` in `crates/tree`.** Merged. Independent of step 2.
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

**2. Shared plumbing in `crates/postflop`.** Merged. Independent of step 1.
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

**3. `PostflopGame`, turn start street, f64.** Built, round two open. Depends on 1 and 2.
Files: create `src/streets/{mod.rs, game.rs, solver.rs, strategy.rs, memory.rs, terminal.rs}`;
edit `src/lib.rs`; `crates/bestresponse/src/lib.rs:8-27` (add a `streets` submodule
mirroring `river`).
Changes: `PostflopGame::new(board: &[Card] (3 or 4 cards), ranges, tree, options
{memory_limit_bytes, precision, threads})` mirrors `RiverGame::new` (`river/game.rs:53-165`):
validates board length against `start_street`, runs the pair-underflow check once on the
board-prefix weights (not per runout), then calls `PostflopMemory::estimate` before any row
allocation and returns `SolveError::MemoryLimit{required, limit}` if exceeded;
`memory_usage()` exposes the estimate publicly. Expansion is depth-first per runout so each
runout subtree is a contiguous `NodeId` range, read off the chance node through
`subtree(node)` and `outcome_range(chance, outcome)`; chance probability is 1/(unseen cards
minus the four private cards), 1/44 on the turn and 1/45 then 1/44 on the flop, so the
contract at `game.rs:35-38` holds; masks zero combos containing the outcome card.
`PostflopTerminal` implements `TerminalEvaluator` (`traversal.rs:5-17`) by mapping `NodeId`
to (runout, payoff) and calling the existing `ShowdownTable` and `evaluate_fold` with the
per-runout table and dead set; tables built once at construction and counted in
`shared_bytes`. `PostflopMemory::estimate` extends the river formula with runout count,
chance-node fan-out, tables, mask pool, construction transients, and per-worker scratch and
traversal buffers (`workers + 1`, the extra one for a concurrent strategy query).
`PostflopSolver` reuses `drive` (`solver.rs`), cancellation between iterations, and `Cfr`
poisoning. `PostflopStrategy` returns per-history policies with the runout card in the
history and carries reach so consumers keep the rare-history context the phase 3 review
required. The memory limit is `memory_limit_mib` in `config/solver.toml` (12 GiB default,
16 GiB ceiling, Decision 4).
Tests (`crates/postflop/tests/streets.rs`): (i) river equivalence: `PostflopTree{start_street:
River}` on the three river fixtures produces bit-identical regrets, strategies and
exploitability to `RiverGame` after N iterations; (ii) all-in on the turn: the terminal vector
equals the mean over 48 rivers of the river all-in showdown vector, and both equal a
brute-force `evaluate_seven` enumeration; (iii) chance-probability sum contract at both
levels of a flop-start fixture; (iv) `Budget` reservations match the estimate's components
and release on drop; (v) a small turn fixture reaches the 0.25% target and the reported
exploitability is a real measurement (stop reason names the target, not the cap); (vi)
`outcome_range` partitions each chance node's subtree on a flop-start tree.
Gate: `check` job (the fixtures are small enough to run inside the 60-minute job).

**4. Rayon over runouts, deterministic.** Depends on 3 (it splits along `outcome_range`).
Files: `src/cfr.rs:330-449`, `src/best_response.rs:166-280`, `src/streets/solver.rs`,
`Cargo.toml` (rayon, MIT/Apache).
Changes: at a chance node with `num_outcomes > 1` and a pool, the walk maps outcomes in
parallel, each outcome computed serially by the same code, collected into
`Vec<Result<Vec<Real>>>` in outcome order, then reduced sequentially in outcome order; the
first `Err` by outcome index is returned (not whichever thread failed first). Accumulators are
split with `split_at_mut` along `outcome_range(chance, k)` so each task owns its slice;
nested chance nodes (flop) nest the split because each level partitions its own parent's
range. `ShowdownScratch` comes from a per-worker pool sized by `resolve_workers`;
`Traversal` becomes `Sync` over its shared parts. `threads == 1` bypasses the pool. The
estimate's `workers` term and the pool size come from the same `resolve_workers` call.
Invariant: identical bits for any thread count.
Tests: Leduc and the turn fixture solved with threads 1, 2, 4 give identical strategy hashes
and exploitability (oversubscribing a 2-CPU runner is fine; determinism does not depend on
physical cores); a poisoned terminal in runout 7 and runout 3 reports runout 3 regardless of
thread count.
Gate: `check` job on both OSes.

**5a. Turn reference tooling and CI.** Merged. Python and YAML only.
Files: `tests/reference/turn/{cases.json, capture.py, capture.mjs, compare.py,
review_combos.py, oracle.py, raise_cap.py, README.md}`; `.github/workflows/ci.yml`
(`turn-reference`, `turn-solve-gate`, `turn-solve`).
Changes: `cases.json` schema adds `street`, a four-card `board`, per-street `bets` and
`raises` for each player, and `max_raises: 1`; the capture driver passes a four-card board
and the per-street size strings listed under Facts, and prunes the reference tree with
`removed_lines` derived by `raise_cap.py` (Decision 11); compare output rows carry the
runout card in `history`; the two-percentage-point per-row rule and
`per-combo-review.json` with `review_reasoning` are unchanged (`docs/astra/phase-3/PLAN.md:104-105`).
Gate: `turn-reference` job fails on any row over two points lacking a committed
`review_reasoning`, on a malformed capture, a wrong pinned revision, a missing runout
branch, or an unreachable convergence claim.

**5b. Turn gate capture and the joint comparison.** Depends on 3, 4, 5a. Astra R4.
Files: create `crates/postflop/examples/turn_capture.rs` mirroring `river_capture`;
`tests/reference/turn/measured/<sha>/`; edit `tests/reference/turn/compare.py`,
`tests/reference/turn/README.md`, `.github/workflows/ci.yml` (new `turn-compare` job).
Changes: the example solves the gate turn fixtures in f64 to 0.25% of pot and emits
policies, exploitability, iteration count, stop reason, estimate, the repository revision,
the host's physical memory and CPU count, and three timings recorded separately: mean
iteration time, best-response measurement time, and cancellation latency (a second run
cancelled at a known iteration). `turn-compare` needs `turn-solve` and `turn-reference`,
downloads both artifacts, and runs `compare.py --project --reference --review` for each OS
capture. `compare.py` in that mode fails on: a missing or skipped project capture; any case
on either side with `reached_target: false` or a stop reason other than the target; any
difference in case id, board, ranges, sizes, raise cap, target, or starting pot between the
two captures; a project revision that is not the workflow's commit or a reference commit
that is not the pinned one; any row over two points without a `review_reasoning`; and a
stale review, meaning a review entry whose recorded row values no longer match the current
captures. The reference-only mode stays for diagnostics and says `project_capture: absent`,
which is never acceptance.
Tests (Python, in `tests/reference/turn/test_compare.py`): fixtures proving that each of
these fails the joint mode: missing project data, an above-target solve on either side, a
changed case input, and an unexplained frequency difference; and that a stale review is
rejected while a current one passes.
Gate: `turn-solve`, `turn-reference`, and `turn-compare` green. Acceptance = the main
session downloads the three artifacts and reruns `compare.py` and `oracle.py` locally.

**5c. Storage lifetime and the exact memory table.** Depends on 3. Astra R5. Can run
beside 5b (disjoint files).
Files: `crates/postflop/examples/memory_table.rs` (new), `crates/postflop/README.md`
(numerical layout section), `src/streets/memory.rs` (component breakdown made public).
Changes: a table computed from the exact approved tree (Decisions 1, 9, 10) and the
positive-weight combos of the approved ranges after board-prefix blocking, for the turn
gate and the flop gate, in f64, f32, and i16. One row per buffer: name, representation,
bytes, lifetime (construction only, whole solve, per iteration, per query, per
verification), and whether it may overlap another. The rows are: regrets; strategy sums;
current policy (stored today, derived at visit time after step 6); average-strategy
snapshots; per-node compression scales; topology and offsets; showdown tables and mask
pool; construction transients; per-worker traversal buffers and scratch; the query buffer;
and the full unmerged best-response walk's working set. The snapshot row states how many
exist, at what precision, when one is taken, and when it is freed. The design target is
zero snapshots retained during the solve: the best-response walk normalises strategy sums
per node as it reads them. Browsing uses at most one compact snapshot, taken at an
iteration boundary and charged against the budget. The project and the reference are
estimated separately (the reference reports `reference_memory_estimate_bytes`). The
example prints the table and the sum; the README records it with the date and revision.
Invariant: the table's sum equals `PostflopMemory::estimate` for the same tree, and every
`Budget` reservation maps to one row.
Tests: table sum equals the estimate on the turn fixture and the flop-start fixture; each
reservation names its row.
Gate: `check`. Acceptance = the main session reconciles the table with the reservations
and with measured peak RSS, first on the reduced flop tree, then on the gate tree once a
host exists. If the table shows f32 with three arrays over 12 GiB on the gate tree, step
10 moves ahead of step 8's full gate run and the dependency lines here change explicitly;
ranges, menus, and targets do not change to make a benchmark fit.

**5d. Job lifecycle contract.** Depends on 3. Astra R7. Drafted by the main session,
reviewed by Astra through `docs/reviews/` before step 6 starts. Turn solves stay internal
(Decision 5); this is the internal contract that step 6's storage work implements and that
step 11 hands to Astra with the flop.
Files: `docs/phase-4/job-contract.md` (new).
Contents: `GameId`, an immutable hash of board, ranges, tree config, precision, and memory
limit; `JobId` and a generation counter; `SnapshotId`. A request returns a quick
acknowledgement, either accepted with the estimate and the reservation or refused with
`{required, limit}`, before any worker starts. Progress events carry iteration,
exploitability, elapsed time, a timestamp, and a sequence number, so a consumer can tell a
stale event from a fresh one. Cancellation has three recorded moments: requested,
acknowledged (the worker observed the flag), released (buffers freed and the budget
returned); a replacement job may reserve only after release. A completion or measurement
that arrives after its job was cancelled is rejected by job id and generation. Browsing
during a solve reads a snapshot taken at an iteration boundary, compact and charged, at
most one alive by default; a query never reads accumulators mid-iteration. Cancel does not
run a best-response measurement: `drive` returns the last measurement with its iteration
and marks it stale when older than the current iteration (implemented in step 6). The
contract states what phase 4 measures (iteration, measurement, and cancel latency, from
5b's capture) and what the app-ready cancellation gate is, so phase 7 can hold the app to
it: acknowledged within one second, released within one iteration.
Gate: prose check clean; Astra's review recorded in `docs/reviews/`. Closure per Astra:
cancel during traversal and during measurement, start a replacement job, verify release
timing, valid snapshots, and stale-result rejection (tested above the numerical core in
step 6's `streets/solver.rs` tests).

**6. Flat layout and in-range compaction.** Depends on 5b, 5c, and 5d. Touches `cfr.rs`
and `strategy.rs`, so it is serial with 7 and 10.
Files: `src/game.rs` (`TraversalLayout`, `Node`), `src/strategy.rs`, `src/cfr.rs`,
`src/solver.rs` (cancel path), `src/streets/{game,strategy,memory,solver}.rs`,
`src/river/strategy.rs` if it indexes `rows`.
Changes: `states[p]` = number of combos with positive weight not blocked by the board prefix;
compact-to-`Combo::id` index tables per player; scatter and gather at the terminal boundary so
`ShowdownTable` and `evaluate_fold` still see `[f64; 1326]` (the sweep and its tests are
unchanged). Rows for all nodes live in one contiguous buffer with a per-node offset table
(u64 offsets); topology becomes struct-of-arrays (kind, first child, child count, payoff
index, runout). Current policy is derived from regrets at visit time instead of stored,
dropping one array. Snapshots follow 5c's table: none retained during the solve, the
best-response walk normalises strategy sums per node as it reads them, and browsing uses
one compact snapshot at a time. `drive` stops measuring on cancel as 5d specifies. The
memory maths in `crates/postflop/README.md` is rewritten from 5c's table.
Invariant: numerically identical results for the river and turn (same arithmetic order per
node).
Tests: river and turn measured hashes unchanged; a range with 37 combos produces
`states == 37`; the table sum still equals the estimate and every reservation; cancel
during traversal and during measurement returns within the stated bound, releases the
budget, and a replacement job reserves; a late completion is rejected.
Gate: `check` and `turn-solve`.

**7. f32 storage on the turn.** Depends on 6. Astra R6.
Files: `src/cfr.rs` (storage trait `Precision` with `F64` and `F32` impls; arithmetic in f64
inside a node update, stored through a checked conversion), `src/config.rs` (accept
`"f32"`), `src/streets/memory.rs` (bytes per precision), `examples/turn_capture.rs`
(`--precision`), `tests/reference/turn/compare.py` (`--baseline` report).
Changes: storage conversion is a separate checked step from the arithmetic checks.
`store_f32(x)` fails loudly with a named reason on a non-finite value, on overflow
(`|x| > f32::MAX`), and on cast underflow (`x != 0` and the f32 result is zero or
subnormal, for example `1e-50`); it accepts every normal value, so `1e-30` stores and
round-trips within f32 relative precision. `reach_product` and `weighted_product` keep
checking the multiplication; `finite` keeps accepting zero. Decoding reads f32 into f64
rows and normalises each state's action row to sum to one (uniform when the row is all
zero), and that decoded, normalised row is what the best-response walk and every strategy
query see.
Invariant: exploitability is always computed by the best-response walk in f64 from decoded
strategies.
Report and acceptance rule: `compare.py --baseline` compares the f32 solve with the f64
solve on the same inputs and revision: both must reach the target; it lists max and mean
absolute frequency difference and action-EV difference per row, and the two residuals. A
row over two points needs a `review_reasoning`, the same protocol as the reference
comparison. A root residual is never quoted as a per-decision error bound.
Tests: f32 turn solve under target; positive and negative `1e-30` store and round-trip;
`1e-50` fails with cast underflow; `1e39` fails with overflow; NaN fails non-finite;
decoded rows normalise; the difference report is produced and its rule enforced on a
fixture with one unexplained row.
Gate: `turn-solve` runs f64 and f32 and uploads the comparison; main session reruns it.

**8. Flop start street, f32, gate and reference.** Depends on 6 and 7. Host per open
question 8. Astra R3, R4.
Files: `src/streets/game.rs` (nested expansion, `start_street: Flop`),
`examples/flop_capture.rs`, `tests/reference/flop/` (49-flop `cases.json`, sharded,
`removed_lines` per case from `raise_cap.py`), `.github/workflows/ci.yml`.
CI additions, four of them. A `flop-smoke` step in `check` solves a small flop tree in
under 10 minutes and prints the estimate. A `flop-gate` job runs on the host Caleb chooses
in open question 8, on demand (Decision 7), with a 360-minute timeout. Its first step
records physical memory, CPU count, image, and any cgroup or container memory limit, and
fails before the solve when physical memory is below `memory_limit_mib` plus 2 GiB of tool
and OS headroom. It then solves the gate tree in f32 and records exploitability, estimate,
and peak RSS. A `flop-reference` matrix job captures seven shards of seven flops. A
`flop-compare` job joins the project and reference captures under 5b's rules. Ordinary
smoke jobs stay on standard runners.
Invariants: memory estimate before allocation, refusal on exceed; exploitability under 0.5%
on the gate tree; peak RSS under the configured limit plus headroom on the recorded host.
Tests: flop all-in fixture equals the mean over 2,352 runouts of river showdown vectors
(brute force); nested chance probability contract; `start_street: Turn` inside a flop tree's
sub-solve reproduces step 3's turn results for that runout; the host prerequisite step
fails on a fixture reporting 8 GB.
Gate: `flop-smoke` in `check`; `flop-gate`; `flop-reference` and `flop-compare`. The f64
baseline for the full gate tree does not fit any 16 GB host, so f32-versus-f64 on the flop
is measured on a reduced flop tree that fits in f64 (one size, ranges under 200 combos,
0.25% target per Decision 3) and on the turn gate tree; the full gate tree compares f32
against i16 in step 10. The README says so wherever the comparison is quoted. A Windows
desktop measurement of the same solve, including the app's own overhead, belongs to phase
7 and is not claimed here.

**9. Suit isomorphism, written from the algorithm.** The cards-crate part depends on 3
only; integration depends on 8 being accepted. Astra R2.
Files: create `crates/cards/src/isomorphism.rs` (plus tests), edit
`crates/cards/src/lib.rs:13-19`; `src/streets/game.rs`, `src/streets/memory.rs`,
`src/cfr.rs` (chance-node orbit loop), `src/best_response.rs` (expanded-strategy view),
`src/streets/strategy.rs` (member queries), and a note in `docs/research/` recording the
licence finding and Decision 6.
Route B (Decision 6, from scratch): compute the suit permutation group fixing the board
prefix; check both ranges are invariant under each generator (weights equal after
permuting suits); if not, merging is disabled and the solve logs why. Otherwise group
runout cards into orbits and expand only one representative per orbit.
Traversal contract (this is the part the first draft left out): every orbit carries its
representative card and, for each member card, the suit permutation from representative to
member and its induced permutation on combo indices (compact indices after step 6). At a
merged chance node the walk descends the representative branch once, with the
representative's own mask and reach. For each member it adds the returned value vector
permuted by that member's combo permutation, weighted by the ordinary per-card chance
probability. No mask or probability is ever multiplied by the orbit size: a hero hand that
holds the representative card is zero in the representative branch and its value for a
member comes from the permuted entry, so `2s3s` on `Ah Kh Qh` receives the values of `2c3c`
and `2d3d` from the representative `2s` branch (Astra's counterexample). Regrets and
strategy sums for the representative subtree are stored once and stand for every member by
symmetry. On the flop the river orbits are computed on the representative turn board and
their permutations compose with the turn member's permutation when values return. A
strategy query for a member card returns the representative's row at the permuted combo
index, and that expanded view is what the best-response walk reads. The pinned reference
permutes returned vectors the same way; read it for approach only, never copied.
Invariant: the best-response walk always runs over the full, unmerged runout set against
the expanded strategy, so a wrong merge shows up as exploitability, never as a plausible
strategy. This is a separate check from the per-combo equality tests below.
Tests: small merged and unmerged games agree per combo, within f64 tolerance, on chance
mass, terminal values, action EVs, and the expanded strategy. That comparison runs on a
monotone flop and a two-tone flop, with a hero hand that blocks one orbit member, and on
a flop-start fixture where both chance levels merge. A weighted asymmetric range disables
merging (asserted by `merged_runouts == full_runouts` and the logged reason). A rainbow
flop reports zero merges. A property test checks the orbit computation against a
brute-force canonicaliser over all 24 suit permutations. A merged solve's exploitability
is under target when measured unmerged.
Gate: `check`; `flop-gate` rerun with merging shows the memory reduction in the estimate
and in RSS, with exploitability still under target unmerged. Added alone, after step 8's
baseline is accepted and before step 10 is combined with it.

**10. 16-bit compression.** Depends on 7 and 6; integrated with the flop only after step 9
is accepted, so each is checked alone against the accepted baseline. Astra R1.
Files: `src/cfr.rs` (`I16` storage), `src/config.rs` (accept `"i16"`),
`src/streets/memory.rs`, `crates/postflop/README.md` (derived bound and measured error, per
`solver-algorithms.md:156-158`), capture examples.
Representation: regrets as i16 with one f32 scale per node row block; strategy sums as u16
with one f32 scale per node. DCFR discounts positive and negative regrets by different
factors (`cfr.rs:167-174`, `discount` at `cfr.rs:311-314`), so the regret discount cannot be
applied to a shared scale; the strategy-sum discount is sign-free and is applied to the
scale alone with no rounding.
Update order per node visit in iteration t, exactly once per node because the full walk
visits every node every iteration. (1) Decode the stored codes to f64 (`code * scale`).
(2) Apply iteration t-1's discount factors to the decoded values: the positive factor to
entries that are zero or positive, the negative factor to negative entries. This is the
same operation the f64 path performs in its end-of-iteration pass. (3) Add this
iteration's regret contribution in f64. (4) Choose the new scale as `max |entry| / 32767`,
zero when every entry is zero. (5) Encode with round-to-nearest. Rounding happens once
per entry per iteration, at step 5. The f64 path keeps its end-of-iteration discount
pass. The i16 path skips that pass and applies it lazily at the next decode, which is
equivalent as long as every node is decoded every iteration. If a future change prunes
nodes from the walk, a per-node "last discounted iteration" counter makes the lazy form
exact; record that here before pruning is added. Overflow of a scaled value and a
non-finite scale fail loudly.
Invariant: decode(encode(x)) error at most scale/2 per entry, asserted in a unit test; an
entry below scale/2 rounding to zero is expected quantisation, not an error, and the tests
say so.
Tests: round-trip bound; a node trace with mixed signs, zero crossings, and ten
iterations agrees with the f64 algorithm within the stated quantisation error; turn i16
solve under target with the difference report against f64 under step 7's rule; Kuhn and
Leduc with `precision = "i16"` actually selected; flop gate i16 versus f32 report.
Gate: `turn-solve` (three precisions) and `flop-gate`; main session reruns the
compressed-error check itself.

**11. Docs and handoff.** Depends on all.
Files: `crates/postflop/README.md`, `crates/tree/README.md`, `docs/phase-4/tree-contract.md`
and `docs/phase-4/reference-contract.md` (new, mirroring phase 3), `docs/phase-4/job-contract.md`
(final form), `docs/ROADMAP.md` progress, `ASTRA-UPDATE.md` handing Astra the typed contracts
the app needs together (Decision 5): turn and flop solve request, job lifecycle per 5d,
runout-aware strategy query with reach context, memory estimate display, progress with
runout count.
Gate: `format` job; `slopcheck.py` clean; Astra's review.

## Tests

* Known solutions: Kuhn and Leduc (`tests/`) and OpenSpiel stay in `check` and must pass after
  every step and at every accepted precision; Leduc is also the determinism fixture for step 4.
* River regression: measured river hashes bit-identical after steps 2, 4, 6, 7, 10 (the river
  stays f64 unless configured otherwise).
* River equivalence of `PostflopGame{start_street: River}` (step 3) is the strongest single
  check that expansion and terminal mapping are right.
* All-in and pending-call: turn and flop all-in values equal brute-force enumeration over
  runouts.
* Exploitability: computed in f64 over the full runout set for every solve, every precision,
  merged or not; reported as a percentage of pot with the stop reason.
* Reference: turn and flop joint comparison (`turn-compare`, `flop-compare`), per-combo
  review, two points per row, no aggregate waiver, no acceptance from a reference-only run.
* Precision: f32 and i16 each compared with f64 on the same inputs where f64 fits (turn
  gate, reduced flop); f32 versus i16 on the full flop gate, labelled as such.
* Memory: the 5c table equals the estimate and the reservations; peak RSS on `turn-solve`
  (standard 8 GB runners) and `flop-gate` (the host of open question 8, with its resources
  recorded); refusal test with a limit one byte below the estimate; host prerequisite check.
* Storage conversion: representable tiny values store; cast underflow, overflow, and
  non-finite fail with named reasons; compression rounding to zero is expected.
* Determinism: thread counts 1, 2, 4 identical.
* Lifecycle: cancel during traversal and measurement, release, replacement, stale rejection.
* Most likely failure path: a NaN or underflow inside one runout on one thread. Test: a
  poisoned terminal in a chosen runout yields `SolveError` naming that runout
  deterministically, with no partial strategy exposed
  (`failed_iteration_never_exposes_partial_strategy` generalised).

## Risks and edge cases

* **The flop gate may not fit 12 GiB at f32 with three arrays.** Step 3 measured 89.8 GB
  at f64 under five arrays; compaction and f32 divide that by roughly ten to fifteen
  depending on the in-range combo count, which lands near the limit. Check: step 5c's
  exact table before step 8 is briefed. If it does not fit, step 10 moves ahead of the
  full gate run, recorded here; the menu, ranges, and targets stay as decided.
* **No CI host has 16 GB today.** Standard private-repository runners are 8 GB. Check:
  open question 8 answered before step 8's `flop-gate` is written; the prerequisite step
  refuses to solve on an undersized host.
* **Isomorphism saves nothing on rainbow flops**; the research note's "roughly halves" is an
  average. Check: estimate with and without merging logged per flop in `flop-reference`.
* **A merge that ignores blockers** produces plausible strategies. Check: the per-combo
  merged-versus-unmerged tests with a blocking hero hand, plus the unmerged exploitability.
* **A wrong chance probability or mask** produces plausible strategies. Check: contract test
  (sum = 1 per compatible pair), river equivalence, and the joint reference comparison.
* **A non-deterministic parallel reduction** hides tiny drifts. Check: hash equality across
  thread counts, first-error-by-index test.
* **The f64 baseline is unavailable on the full flop tree.** Check: baseline comparisons on
  the turn gate and a reduced flop tree; recorded in the README so nobody reads the
  f32-versus-i16 comparison as f64-validated.
* **Compression that changes the algorithm.** A shared scale cannot carry DCFR's two sign
  factors. Check: step 10's update order and the mixed-sign trace test.
* **Per-node `Vec` allocation churn** in a multi-million-node traversal may make flop
  iterations too slow for CI. Check: iteration time logged; if the gate tree cannot converge
  in the 360-minute job, replace per-node returns with the depth-indexed arena already
  counted in `traversal_bytes` (no numeric change; hash regression proves it).
* **Cancellation latency** is one iteration plus, today, a best-response measurement
  (`solver.rs:93-107` measures after observing the cancel). Neither is measured yet. Check:
  5b records iteration, measurement, and cancel latency separately; 5d and 6 remove the
  measurement from the cancel path; phase 7 holds the app to the stated gate.
* **Rare-history conditional values** (phase 3 review): `PostflopStrategy` carries reach and
  runout so a consumer cannot quote a river policy without context.
* **The pinned reference is a suspended project.** Fine for a pinned commit; if a future
  toolchain cannot build it, the river capture breaks first and says so.
* **i16 overflow or scale underflow** during discounting. Check: loud failure test;
  per-node scale recomputed each update.
* **A range that breaks symmetry** silently merged. Check: explicit invariance test per
  generator; merging disabled with a logged reason.

## Open questions

Questions 1 to 7 were answered on 2026-09-06 (Decisions 1 to 8). Two are open.

8. **Flop gate host.** The `flop-gate` job needs a host with at least 16 GB of physical
   memory and a recorded specification. Standard runners for this private repository have
   8 GB, and GitHub's larger runners need an organisation on a Team or Enterprise plan.
   Options: (a) a self-hosted runner in WSL2 on Caleb's 16 GB development machine,
   labelled and used only by the on-demand gate, which also measures the machine the app
   must ship on; (b) make the repository public, which gives the 16 GB standard tier for
   free but is a visibility decision; (c) an organisation on a paid plan with a larger
   runner; (d) a rented Linux machine for each gate run. The main session recommends (a),
   with (b) as the fallback. Paid options need Caleb's approval before anything is
   provisioned.
9. **Product milestones from Astra's review.** Astra proposes three sequencing changes
   to `docs/ROADMAP.md`: one complete play, ask why, review, retry loop as the next product
   milestone after the internal contracts, with a short observed session; an advice
   coverage measurement during normal play before the library scales; and a bounded
   preflop-chart feasibility investigation earlier than phase 11. None changes the phase 4
   scope. They are Caleb's call and are recorded in the roadmap's Decisions as proposals
   until he answers.

## Decisions

All given by Caleb on 2026-09-06 unless dated otherwise, in multiple-choice form.

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
   receives turn and flop contracts together in step 11. The lifecycle contract in step 5d
   is drafted early for the storage work and reviewed by Astra, not shipped as a turn
   feature.
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
10. **Gate raise rule and river all-in (2026-09-06, after the step 1 and 5a reviews):** every
    phase 4 gate tree caps raises at one per street (`max_raises: 1`) and sizes that raise at
    100% of pot, the phase 3 convention. The river menu is `33%,75%` bets plus the raise with
    no explicit all-in token, as phase 3's river had it; a jam appears only when a raise
    clamps to the stack. Flop and turn keep `33%,a` per Decision 1. Chosen over a 60% raise
    (step 1's fixture) and over a 32-raise cap (step 5a's cases), because one raise per
    street is what the plan's feasibility count of five live continuations assumed.
11. **Reference raise cap (2026-09-07):** the pinned wasm-postflop binding has no raise-cap
    input, so with the 100% raise menu its turn trees reach three raises on a street and a
    cap of one cannot be captured directly (run 34079922254 failed at `max_raises: 1`).
    Caleb chose to prune the reference with upstream's `removed_lines` so it solves exactly
    our one-raise tree, over comparing on the deeper 32-raise tree and over changing the
    size menus. The same routine serves the flop comparison in step 8.

## History

Earlier review outcomes and executor reports, kept as written. The state table above is
current; where a paragraph here disagrees with it, the table wins.

### Step 3 review, 2026-09-08 (round one, resolved on the branch)

Step 3 was built on `origin/worktree-agent-a53c5f7bb472f1e9a` at da6477d (six commits on
b82ae27, CI run 34084062082 green, `Turn solve` skipped by design). It was reader-reviewed,
spot-checked, and put through `/code-review` (eight angles). Verdict: the numerical path is
right (chance probability 1/(unseen-4) with per-card masks, river-start game reproduces
`RiverGame` bit for bit, called turn all-in matches a brute-force enumeration, small turn
solve reaches 0.195% of pot) but nine findings blocked acceptance: nested `runout_ranges`
on a flop-start tree; no test with two chance levels; `threads: 0` resolving to one worker
and the query overlap missing from the bound; construction transients outside the estimate;
the 12 GiB default not in configuration; stale README and plan text; cleanups (`STATES`,
`MAX_EXPANSION_DEPTH`, unused accessors); `PostflopGame::new` skipping path validation; the
110-decision comment and the fixture target. A fresh executor closed all nine on the same
branch (head de8219a, CI run 34286324836 green at bfb2162), and its resolution paragraphs
arrive here at merge. The reader fact sheet and `/code-review` of that delta produced round
two (A to I above).

### Step 1 review outcome

Reader fact sheet plus a main-session read of `State::after`, `settled`, `opening_after`,
and the build loop in `crates/tree/src/postflop.rs`. The river-start tree matches
`RiverTree` node for node on the phase 3 fixtures and sixty generated configurations. Two
things fixed in step 3, housekeeping rather than defects: the gate test
`the_gate_menu_counts_its_nodes_per_street` and the anchor test used a 60% raise and a
river all-in token, which Decision 10 rules out; their fixtures moved to a 100% raise and a
`33%,75%` river and the pinned counts were re-derived (flop [10, 50, 270] decisions,
[5, 25, 153] live continuations, 925 nodes, depth 13). The last sentence of
`crates/tree/README.md` ("awaits hosted Rust verification") was stale: CI run 34061497535
verified it. The step 1 executor did not run cargo locally; the step 2 executor did.

### Step 5a review outcome

`cases.json` carries Decision 9's ranges character for character. `max_raises` was 32 in
all three cases; Decision 10 makes it 1 and Decision 11 prunes the reference to match
(the `turn-reference` job costs about three minutes per push, so it stays on every push).
The `flop-subset` job is a 20-second list check, not a solve; Decision 7's on-demand rule
applies from step 8. `capture.py` refuses to run outside Linux, so turn captures come from
CI or WSL2 only.

### Step 1 Noticed list (for step 3)

Per-street counts are not uniform at deep bases (a raise target can clamp to the stack and
merge into the all-in), so the memory estimate must sum the built tree's counters, never
multiply one block's anchors. Chance nodes carry only `next: Street`, no card and no
probability; step 3 supplies both. `max_nodes` bounds the compact tree, not the expanded
runouts. `MAX_DEPTH` is a hard-coded 128. `PostflopTreeConfig` has no defaults.
Contributions are cumulative from the root; a separate per-street `base` drives the
minimum bet and raise multipliers. Called all-ins chain single-child chance nodes to
showdown with no decision and no live continuation. Three rounding helpers (`add_action`,
`pot_after`, `rounded_product`) are copies of the frozen `river.rs` ones.
`PostflopNode::street()` exists beyond the plan's listed API.

### Step 2 Noticed list (for steps 3 and 4)

Leduc's 30 (node, outcome) chance pairs pool to six mask entries. `Traversal` in `cfr.rs`
holds `&mut dyn TerminalEvaluator` over one shared `ShowdownScratch`, so step 4 needs an
evaluator per worker before anything is `Sync`. `precision` accepts only `"f64"` until
step 7. The step 2 executor's "local cargo works" did not hold for the step 3 executor.

### Step 5a Noticed list (for steps 3 and 5b)

With donk sizes unset, the reference still gives OOP its ordinary river bet menu after
calling a turn bet; our tree must match or the histories misalign. The reference merges
isomorphic runouts whenever a suit permutation fixes the four-card board: 12 on the paired
board, 13 on the flush board, 0 on the rainbow one. `compare.py` records both sides' merge
counts and only requires the `possible_cards` sets to agree. `oracle.py` recomputes
showdown values and action EVs, not exploitability. The 49-flop list is a
texture-stratified sample, not frequency-weighted; changing its seed or buckets is a
decision, not a parameter. Export is capped at 64 MiB per case, three or four named runouts.

### Executor reports

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

Step 5a: `tests/reference/turn/` and the `turn-reference` job are done and green on the
approved BTN-versus-BB ranges (all three cases reach the 0.25% target in 200 iterations,
0.186/0.235/0.185% of pot); `turn-solve` is wired behind a gate job and skips until step 5b
lands `crates/postflop/examples/turn_capture.rs`; `tests/reference/flop/select_flops.py` and
its 49-flop `flops.json` are added with a `flop-subset` job. Three things the plan did not
know. The export must be scoped to three or four named runouts per case, because all 48 is
about 12,000 nodes and roughly 90 MB of JSON per case, over the capture's 64 MiB cap. The
reference merges isomorphic runouts whenever any suit permutation fixes the four-card board,
which includes paired boards showing all four suits: 12 merges on the paired board, 13 on the
flush board, 0 on the rainbow one. `compare.py` now asserts that a merged pair's exported
rows are equal under the suit swap, which is 105,391 cells at zero difference on the
committed cases.
And with donk sizes unset, upstream still gives OOP its ordinary river bet menu after it
calls a turn bet, so our tree must do the same or the histories will not line up.
