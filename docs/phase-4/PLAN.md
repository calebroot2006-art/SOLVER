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

## Progress (updated 2026-09-10, Astra review complete, corrections in progress)

Read this first when picking the work up. The state table says where every step is, on
which revision, and what accepts it. "Next actions" is the order of work. "History" keeps
the earlier review outcomes and executor reports as written, because they hold facts the
steps below depend on.

### State table

Base revision for new work: `solver/phase-4` at 14a02fd (the step 5b merge of 2026-09-10
plus the plan reconciliation), clean; CI run 34513847858 on that commit is green on all 13
jobs, including the turn solve and the turn comparison gate.

| Step | State | Revision and branch | Gate | Acceptance |
|---|---|---|---|---|
| 1 `PostflopTree` | Merged | 7f19a4d on `solver/phase-4` | `check` | Accepted 2026-09-07 |
| 2 Shared plumbing | Merged | 281c661 on `solver/phase-4` | `check`, river hashes | Accepted 2026-09-07 |
| 5a Turn reference tooling | Merged | 0a076b6 on `solver/phase-4` | `turn-reference` | Accepted 2026-09-07 |
| 3 `PostflopGame`, turn, f64 | Merged | f3f55b7 on `solver/phase-4` (branch head 7522b91, CI run 34393031096 green) | `check` | Accepted 2026-09-09 after rounds two and three; river capture from run 34388450561 matches the accepted record |
| Decision 11 reference pruning | Merged | merge commit after f3f55b7 on `solver/phase-4` (branch head 4147355, CI run 34285342323 green) | `turn-reference` | Accepted 2026-09-08 |
| 4 Rayon over runouts | Merged | branch head f88de8b, CI run 34428676356 green on both OSes | `check` | Accepted 2026-09-09: bit-identical policies at 1, 2, 4 workers (turn fixture, 2.69x at 4 workers on the runner), nested flop-start split, river capture from run 34426744713 matches the accepted record |
| 5b Turn gate capture and joint comparison | Merged | merge 114b815 (branch head d0830ca, CI run 34510476253 green on all 13 jobs) | `turn-solve`, `turn-reference`, `turn-compare` | Accepted 2026-09-10: main session reran `compare.py` with the committed review on both OS captures, the river field comparison, and the capture comparison across runs; three executor rounds, eight review findings closed. Thresholds are the Decision 14 candidate, unconfirmed |
| 5c Storage lifetime and memory table | Merged | branch head 6777635, CI run 34434849281 green | `check`; `memory_table` example | Accepted 2026-09-10: main session reran the example locally and every README number reproduced; two review rounds, ten findings closed. Conclusion: the flop gate needs step 6 and step 7 together; i16 is headroom |
| 5d Job lifecycle contract | Reviewed and amended 2026-09-10 | `docs/phase-4/job-contract.md` | prose check; implementation gates below | Review at d9c979d; step 6 primitives and required step 11 driver work are distinguished |
| 5e Self-hosted flop gate runner | Runbook written 2026-09-09 (`docs/ci/self-hosted-runner.md`); install by Caleb pending | Decision 12 | runner online, trivial dispatch | Main session reads the dispatch log |
| 6 Flat layout and compaction | Corrections verified locally; final integration CI pending; **not merged** | Original branch e338d7a preserved; corrections on `solver/astra-step6-corrections` | `check`, `turn-solve`, corrected acceptance gate | Seven Rust findings closed; 69dc3ff green on all 13 jobs; new OS captures preserve solved fields; final all-in gate independently rerun |
| 7 f32 storage | Not started | after 6 | `turn-solve` f64 and f32 | Main session reruns the difference report |
| 8 Flop start street and gate | Not started | after 6, 7, 5e | `flop-smoke`, `flop-gate`, `flop-reference`, `flop-compare` | Main session reruns the gate's exploitability and the comparison |
| 9 Suit isomorphism | Not started | cards part after 3; integration after 8 | `check`, `flop-gate` merged versus unmerged | Main session reruns the merged-versus-unmerged per-combo test and the unmerged exploitability |
| 10 16-bit compression | Not started | after 7; integrated after 9 is accepted | `turn-solve` three precisions, `flop-gate` | Main session reruns the compressed-error check |
| 11 Driver closure, docs and handoff | Not started | after all | driver behavioral tests, `check`, `format`, prose check | Astra review |

The step 3 and Decision 11 worktrees and branches were removed after the merges; the
phase 0 worktree `agent-a5c19d7e711c4fc07` (763faba) was removed with them.

### Next actions, in order

1. Complete and verify the isolated step 6 corrections and turn-gate corrections in
   `docs/astra/step6-corrections/PLAN.md`. Astra completed all five requested reviews
   at d9c979d, under `docs/reviews/2026-09-10-step6-review/`. Step 6 remains unmerged
   until its corrections pass independent review and the numerical/CI gates.
2. Caleb confirms or changes the Decision 14 thresholds; the record already carries the
   threshold-free totals that make the choice concrete.
3. Amend the reviewed phase 5 and 6 plans before their executors start. Caleb's runner
   installation and unanswered product choices remain open. Other authorized work
   continues only through its stated dependency gates.

### Astra correction pass, 2026-09-10

The saved mid-flight section below records the incoming state, not current acceptance.
Astra confirmed the sums guard defect and six other step 6 defects, plus three turn
acceptance defects. The original before/after captures still agree on solved fields;
the acceptance tooling nevertheless required root-value validation, fresh oracle
recomputation, and reconstructed reach/EV-availability checks.

The correction branch separates per-game solver primitives from the required application
driver. Its amended lifecycle contract records pushed/coalesced progress with status
recovery, one running-job browsing snapshot, and two separately budgeted finished results
for comparison. Global replacement ordering, query pins, snapshot invalidation and the
total application budget remain explicit implementation gates before step 11 acceptance.
Neither available budget slack nor a per-game drop test proves those rules.

Corrected allocation estimates for the widest flop ranges are 13,685,414,694 bytes
mid-solve at f64 and 20,357,152,670 with a browsing snapshot. Projected f32 is
7,013,677,014 and 10,349,546,150 respectively. These supersede the undercounted estimates
in the historical step 6 report. Step 7 remains required; no f32 implementation or host
peak-memory measurement is claimed by this arithmetic.

### Saved mid-flight, 2026-09-10 evening (account hit its limit)

**Step 6 is built, pushed and green, and is not merged.** Branch
`worktree-agent-aeabce3b33204de8a`, head e338d7a, worktree
`.claude/worktrees/agent-aeabce3b33204de8a`, base 14a02fd. CI run 34524553796 is green on
all 13 jobs. The branch's own account is under History as "Step 6". The review handoff,
with what was verified and what still needs reading, is
`docs/reviews/2026-09-10-phase4-step6-handoff.md`.

**What the main session verified.** The turn captures of the step 6 run and the step 5b run
agree on every one of 2,501,439 fields per operating system except timings, the revision,
the progress interval and the memory bounds: no policy or value moved. `compare.py --review`
rerun on both step 6 captures is accepted with zero rows missing review and zero stale, so
every recorded strategy matches to 1e-12. The river captures match
`tests/reference/river/measured/2930550/` on every solved field. Measured bounds at one
worker in f64: turn gate 75,768,147 against 5c's predicted 80,344,515, flop gate
13,533,132,510 against 14,095,816,234, both under, and 5c's ordering conclusion stands with
the flop gate 618.20 MiB over the 12 GiB default mid-solve.

**What was not done.** `/code-review worktree-agent-aeabce3b33204de8a high` completed two of
its eight angles before the account limit stopped the rest. Nobody has read this diff for
correctness end to end. The three places that carry the risk are `chance_in_parallel`'s
slicing of the flat buffers in `cfr.rs`, the scatter and gather at the terminal boundary in
`streets/terminal.rs`, and `estimate`/`rows_under` in `streets/memory.rs`.

**The twelve findings that did land, all quality rather than correctness.** Reuse: the
regret-matching chunk loop is written four times (`Cfr::current_row`, `Cfr::normalised`,
`Traversal::policy_row`, `PolicySource::row`); the compaction projection is re-implemented
four times against `game.live` instead of living on `Inner` beside `state_of`;
`PostflopStrategy::row` re-implements the `BLOCKED` sentinel check `state_of` already does;
`opposing_mass` is written three times with two different error kinds; the generic slice
cutting was pasted into `chance_in_parallel` twice, and **the length guard is applied to the
regrets array but not to the sums array**, which would panic inside `split_at_mut` instead
of returning the named error (the one finding here worth treating as a possible defect); the
gate tree config and ranges are spelled out in both `tests/streets.rs` and
`examples/memory_table.rs`. Simplification: `Parts.mask_pool_bytes` is written and never
read; `Progress` and `SolveReport` each carry the same three fields derived from one
optional measurement, and can represent `measured_at: Some` with `exploitability: None`;
`PolicyRow` is a hand-rolled `Cow`; `Expansion::intern` takes three arguments all derivable
from the payoff, with four tag constants mirroring the enum; `chance_in_parallel` packs a
five-tuple where a small struct would read.

**Open from the executor.** The river record's `working_set_bound_bytes` and
`reserved_bytes` are 136 bytes above the accepted record, up from 24, with 112 of it from
`size_of::<TraversalLayout>()` growing; no solved field moves and nothing in CI compares
those fields. `PostflopStrategy::from_rows` now expects compacted-width rows. Peak resident
memory is still unmeasured against the table, which is 5c's outstanding acceptance step and
needs the flop gate runner. Three changes against the drafted 5d contract are listed in the
step 6 paragraph under History.

### Step 5b

Merged 2026-09-10 at 114b815 from `worktree-agent-a9ca9d74a78331558` (head d0830ca, CI
run 34510476253 green on all 13 jobs). Three executor rounds; the branch's own account is
under History as "Step 5b", "Step 5b round two" and "Step 5b round three". Review handoff:
`docs/reviews/2026-09-10-phase4-step5b-merge.md`.

**What the turn gate says.** Both sides converge under the 0.25% target: project 0.208 /
0.166 / 0.208% of pot at 150 / 200 / 150 iterations, reference 0.159 / 0.178 / 0.185%.
Root EVs agree to 0.00337 / 0.00091 / 0.00067 chips on an 11-chip pot. Of 43,542 / 52,779 /
42,417 differing rows, the rule sorts A/B/C as 10,998/32,193/351, 17,029/35,561/189,
9,913/32,227/277; the C rows cost 4.10e-05 / 1.35e-05 / 1.32e-05 of the pot against the
5.0e-03 budget, and the threshold-free reach-weighted loss over every differing row is
1.08e-03 / 4.96e-04 / 8.36e-04, so a rule with no indifference threshold at all would sit
inside the budget. Every C row carries `oracle.py`'s recomputation, worst disagreement
6.86e-13 chips. Linux and Windows captures are bit-identical in every policy and value.

**Main-session verification.** `compare.py --review` rerun on run 34510476253's captures,
both OSes, accepted with the counts above; river captures identical to `measured/2930550`
except elapsed, revision and the +24 bytes known since step 3; turn captures across runs
34478062038 and 34510476253 differ only in timings, revision and the 85,376-byte node
report row (2,501,439 fields per OS). `/code-review high`: eight findings, all closed and
re-verified (see the round-three paragraph under History).

**Memory.** `node_values` has its own row (`NODE_REPORT`, 85,376 bytes) counted beside the
decision report; every bound in the 5c table grows by exactly that (turn gate 471,031,163;
flop gate compacted f64 14,095,816,234, f32 7,424,078,554). 5c's ordering conclusion is
unchanged. Both standard runners report four CPUs and about 16 GB, not the two and 8 GB
under "Facts the later steps depend on".

**Open.** Decision 14 thresholds (below) are unconfirmed. The capture is 65,101,178 bytes
against the 64 MiB ceiling, 3% headroom, and the size guard runs before the file is
written. The committed review is generated from the Linux capture and checked on both.

### Step 3 rounds two and three (resolved, merged)

Round two (A to I, from the reader fact sheet and the eight-angle `/code-review` of
da6477d..bfb2162) closed at 940ec23; round three (three findings from the `/code-review`
of de8219a..940ec23: the `pairs` doc claiming one shared budget, the memory doc naming
two columns where the walk holds one, and `validate_traversal` accepting a column source
with no scoped pairs) closed at 7522b91. The branch's own account is under History.

Not now, for step 6: the river/streets duplication (`scaled`, root normaliser,
`evaluate_terminal`, `Payoff`, memory formulas, `from_rows`, `decision_values`,
`PostflopSolver` versus `RiverSolver`), `Payoff::Showdown([u, u])`'s symmetric pair,
`showdown_tables` charging 2,352 ordered runouts where the build interns 1,176, and the
`Layout` callback path's validation transients (compatibility table, terminal kernels,
zero-sum matrix), which the river estimate does not charge because the river solve does
not use that path.

Also for step 6, from the step 5b review (2026-09-10): `node_values` and `decision_values`
share `path_reaches` but still duplicate the mass, walk and underflow block; `path_reaches`
recomputes reach from the root per query and builds both players' masks even for
`decision_values` (query time only, not the solve); `drive` in `solver.rs` uses one
measurement for both the progress log and the stop test, which `turn_capture` works around
with an 86,400-second progress interval (step 6 owns the cancel path: stop test on
`check_every` only, log keeps its timer); after a called all-in on the turn our tree deals
48 river runouts into showdowns where the reference ends at a turn showdown, and
`compare.py` excludes those nodes by shape without a numeric check, so if step 6 ends the
hand at a turn showdown the special case and the 48 nodes go together; the turn capture
sits 3% under `compare.py`'s 64 MiB ceiling.

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
  tree asks for 89,793,081,162 bytes at f64 under today's five-array accounting and is
  refused under 12 GiB.
* **Local Rust is best-effort.** Smart App Control blocks `cargo.exe` on this machine
  while `rustc.exe` runs. CI is the compiler for every gate. Python tooling runs locally,
  so the main session can rerun `compare.py` and `oracle.py` on downloaded artifacts.
* **CI hosts.** This repository is private. GitHub's standard runners for private
  repositories are 2 CPUs and 8 GB (Linux and Windows); the 4-CPU, 16 GB tier applies to
  public repositories only, and larger runners need an organisation on a Team or
  Enterprise plan (GitHub runner reference, read 2026-09-09). Every job in `ci.yml` runs on
  standard runners today. The turn gate fits them (the turn tree is about 250 MB per f64
  array at 1,326 states). The flop gate does not; Decision 12 puts it on a self-hosted WSL2 runner.
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

### Step 4

Built on `worktree-agent-ad40c29522beb0497`. At a chance node with more than one
outcome and a mask pool, the CFR walk and the best-response walk map outcomes over a
`rayon` pool. Each outcome is walked by the same code, collected in outcome order and
reduced in outcome order. The first `Err` by outcome index is what is returned.
Accumulators split with `split_at_mut` along `outcome_range(chance, k)`, and a split
that does not match the tree is refused. Nested deals nest the split. `SharedTerminal`
takes `&self` so each worker takes the showdown workspace its own pool index names.
`threads: 1` builds no pool and runs the previous code path. `PostflopSolver::workers()`
reports the pool size, which is the same `resolve_workers` answer the estimate charged.
`rayon` is pinned `=1.12.0`, MIT OR Apache-2.0, recorded in `crates/postflop/README.md`.

Two facts for the later steps. The traversal-buffer term now uses the widest deal rather
than the widest bet menu, **above one worker only**. A parallel chance node holds one
value vector per outcome while a serial one holds one at a time. At `threads: 1` every
estimate is unchanged. And the gate flop tree's refusal is **89,793,081,162 bytes**
under 12 GiB, not the 89,793,081,162 recorded under "Facts the later steps depend on":
that figure went stale during step 3's rounds two and three. Measured on `11904b4` with
step 4 reverted, and identical with step 4 applied.

Small turn fixture unchanged: 0.195407% of pot, `nash_conv` 0.039081 chips, 50
iterations, `TargetReached`, and bit-identical at 1, 2 and 4 workers.

### Step 5b

Built on `worktree-agent-a9ca9d74a78331558`, pushed as the same branch name. First CI run
[34432298497](https://github.com/calebroot2006-art/SOLVER/actions/runs/34432298497) on
`301d6e4`: every job green except `turn-compare`, which failed on one rule and one only.
`crates/postflop/examples/turn_capture.rs` solves the three gate cases in f64, reading the
memory limit, worker count and precision from `config/solver.toml` and the target, cap and
check interval from each case. `compare.py` gained the joint mode and every refusal the step
lists; `test_compare.py` proves each of them (109 tests). `serde_json` is a new
**dev-dependency** of `crates/postflop`, MIT OR Apache-2.0, licence recorded in `Cargo.toml`
beside `rayon`'s. It is **not** yet recorded in `crates/postflop/README.md`, which step 5c
owned in parallel; that line still needs adding.

**Convergence, both sides under the 0.25% target.** Linux 136 / 162 / 136 iterations at
0.24427 / 0.23872 / 0.23808% of pot; Windows 135 / 162 / 136 at 0.24698 / 0.23872 /
0.23808%. The reference reached 0.15869 / 0.17830 / 0.18510% at 150 / 200 / 150. Root
expected values agree between the two solvers to 0.00067 to 0.00337 chips in a pot of 11.

**Memory.** Estimate 377,933,341 bytes at four workers. Peak RSS 342,822,912 bytes on Linux
(`/usr/bin/time -v`) and 298,213,376 bytes on Windows: 91% and 79% of the estimate, so the
bound held and is not wildly loose. The capture is 62.9 MB against the 64 MiB `compare.py`
reads, 6% of headroom.

**Timings, four workers on both hosts, per case range.** Linux: mean iteration 1.712 to
1.741 s; best-response measurement 8.80 to 8.90 s; average-strategy snapshot 0.043 to
0.067 s; cancellation 4.956 to 5.012 s from the flag, of which 3.299 to 3.343 s is the
measurement `drive` takes after observing the cancel, leaving 1.649 to 1.670 s of in-flight
iteration. Windows: 1.879 to 1.919 s; 10.03 to 10.09 s; 0.071 s; 5.480 to 5.526 s, of which
3.704 to 3.727 s measurement, leaving 1.753 to 1.822 s. The residue matches that host's mean
iteration time on every case, which is what says the two-probe decomposition is measuring
what it claims. The best-response number is far above one iteration because
`PostflopStrategy::exploitability` walks on one thread whatever the worker count is.

**Hosts.** Both standard runners reported **four** CPUs and about 16 GB, not the two CPUs
and 8 GB recorded under "Facts the later steps depend on". Worth rechecking before step 8
sizes anything around the two-CPU figure.

**Linux and Windows agree bit for bit.** On the two cases that stopped at the same
iteration, every exported policy cell matched exactly: 243,670 cells on the paired board and
178,320 on the flush board, largest difference 0.0. That is step 4's determinism confirmed
on a full turn tree rather than the small fixture.

**The third case did not stop at the same iteration, and that was a defect in this step's
own capture.** `drive` uses one measurement for both the progress callback and the stop
test, so `config/solver.toml`'s ten-second `log_every_secs` made the stopping iteration a
function of how fast the machine is: 136 on Linux, 135 on Windows, same commit, same
workers, same inputs. A gate capture that cannot be reproduced is not evidence, so
`turn_capture` now sets the progress interval far above any solve and lets `check_every`
decide; it records what it used as `progress_interval_seconds`. Convergence still reaches
the job log every `check_every` iterations, and the solve does less work, because the timed
schedule was spending roughly 34 extra best-response measurements per case. This overrides
the brief's instruction to log every ten seconds. The other fix, letting the stop test
consider only `check_every` measurements while the log keeps its timer, is in
`crates/postflop/src/solver.rs` and was not this step's to make.

**Two tree conventions the comparison now reconciles, as checked equivalences.** Our
`Action` names a wager by the actor's total commitment since the root; the reference names
the chips going in on the current street. They coincide on a river-rooted tree, which is why
phase 3 never had to choose. Restating ours makes 320 labels match. Every menu and every
contribution then agrees exactly.

Separately, after a called all-in on the turn our tree still deals the river into showdowns,
while the reference ends the hand at a turn showdown. No policy row lives there. Each such
line does cost 48 expanded nodes on our side against one on theirs, which step 6 may want
back.

**Two things are open and neither is mine to settle.**

1. **The two-point per-row rule does not survive a 0.25% target.** 141,567 rows on Linux and
   141,599 on Windows exceed two percentage points, out of 233,567 compared, with single
   differences up to 0.9999, while the two solves agree on the game value to 0.003 chips.
   The differing rows are near-indifferent: at the heaviest ones the two actions sit within
   a few hundredths of a chip, so the mix between them is barely determined. The river's
   588-row review was possible because the accepted river record is refined to 4.6e-05% of
   pot, four orders of magnitude tighter than this gate's target. Writing 141,567 reasoning
   entries is not a review. `turn-compare` is red until the gate's rule is decided: a refined
   pass on both sides, a reach-weighted rule, or agreement on the game value plus the rows
   that are actually reached. Everything else in the joint mode passed, including both
   convergence checks, the revision check, the input check and every history.
2. **`oracle.py` and `review_combos.py` cannot be run on a project turn capture, and they
   fail silently.** They read per-hand values at chance nodes and turn showdowns;
   `decision_values` refuses any node that is not a decision and nothing else on
   `PostflopStrategy` returns per-hand values at an arbitrary node. `oracle.py` turns a
   missing value into `0.0`, so the walk returns a wrong number rather than an error: root
   row `2c2d` of the rainbow case gives 2.23 for check where the capture's own
   `decision_values` say 14.96 and the reference oracle says 14.70. Closing it needs a
   `PostflopStrategy::node_values(node)` accessor in step 3's code.

No `measured/<sha>/` record was committed. The captures are 62.9 MB each and the comparison
reports 263 MB each, so the river's habit of committing the raw files does not carry over.
Nor is there much point pinning a record of a red gate to a commit the rule decision will
supersede. Everything is in the run's artifacts: `turn-project-ubuntu-latest` (10135337779),
`turn-project-windows-latest` (10135401999), `turn-wasm-reference` (10134970691) and
`turn-comparison` (10135449079), all on seven-day retention, which is itself a problem for a
permanent record.
### Step 5c

Built on `worktree-agent-ac40b78d67c3be6ce`, green on both OSes at 50676e0 in CI run
34431642933 and again on the branch head after the round-two review fixes, whose head and
run id are in the executor report. `PostflopMemory::rows_under` reports one row per buffer with its
representation, bytes, lifetime and overlap. The rows the bound counts sum to
`working_set_bound_bytes`, and `MemoryReservation` names, for every `Budget` site, the rows
it draws from, checked against the bytes the budget actually holds during a solve. The two
fixture sums reproduce (31,790,761 with construction 4,205,121, and 422,706,474) and are
pinned in `crates/postflop/tests/streets.rs`. `PostflopMemory::for_tree` prices a tree
`PostflopGame::new` refuses, which is the only way the flop gate can be priced at all. The
table is printed by `crates/postflop/examples/memory_table.rs` and recorded in
`crates/postflop/README.md`. No charged number moved: the aggregate fields are the same
terms regrouped.

Sums at one worker, in bytes, f64 / f32 / i16. Turn gate (9,003 expanded nodes, 3,178
decisions, 11,363,820 entries per stored array), today's three arrays and two snapshots:
470,945,787 / 243,669,387 / 130,094,747; after step 6 (two arrays, no retained snapshot,
live combos): 80,259,139 / 48,001,659 / 31,898,343. Flop gate (1,792,006 nodes, 637,500
decisions, 2,222,795,016 entries), today: 89,793,081,162 / 45,337,180,842 / 23,121,980,682;
after step 6: 14,095,730,858 / 7,423,993,178 / 4,093,224,338; after step 6 with one browsing
snapshot alive: 20,810,476,978 / 10,802,870,458 / 5,806,717,198. `f32` and `i16` are
arithmetic over the same entry counts, as is any row charging fewer than 1326 states.

**Ordering.** The turn gate fits the 12 GiB default today at every width. The flop gate fits
at none of them: 7.0x the limit at f64, 3.5x at f32, 1.8x at i16. Step 6 is required and is
not sufficient. Compacted, with the policy derived and no snapshot retained, the flop gate
still needs 14,095,730,858 bytes at f64, which is 1.13 GiB over the default, though it would
fit the 16 GiB ceiling, which is the machine and not the configured limit. Step 7's f32
closes it at 7,423,993,178 bytes, 5.09 GiB spare, and 1.94 GiB spare with a browsing
snapshot alive. So the flop gate needs 6 then 7, and step 10's i16 is headroom rather than a
prerequisite. One reading to settle: this step's contingency says step 10 moves ahead of step
8 "if the table shows f32 with three arrays over 12 GiB on the gate tree". Literally, f32
with today's three arrays and no compaction is 45,337,180,842 bytes, over. Read as the layout
steps 6 and 7 actually produce, it is 7,423,993,178 bytes, under. Under the second reading
the dependency lines stand as written; the main session decides which reading governs.

**What the plan did not know.** The gate menu at the reference capture's chip scale (pot 11,
stack 195, minimum bet 1) builds the same tree and the same bound as the phase 3 scale (55,
975, 10) the tree test uses, so one table describes both. Live combos of the Decision 9
ranges after board removal are 34.5% to 37.5% of 1326 across the six flops priced, worth
about 2.7x, and on the three turn boards they are exactly the reference's
`private_hand_counts` (469/470, 468/473, 445/448 from the `turn-wasm-reference` artifact of
run 34401787355), which cross-checks our ranges against the reference's input. The estimate
itself depends only on the tree, the board length and the worker count, not on the ranges or
which board: a board changes only the compacted projection. The two retained snapshots are
the largest single term today, 35.65 GB of the flop gate's 89.79 GB, so 5d's "at most one
snapshot alive" is a memory decision and not only a lifecycle one. Everything that is not a
stored entry array costs 651 MB on the flop gate (topology 496 MB, showdown tables 154 MB),
which is 8.8% of the post-step-6 f32 working set, so step 6's flattening of topology matters
much less than the entry width does. An extra worker costs 2.0 MB on the flop gate, which no
ordering decision depends on. The reference's own estimates for the same three turn cases are
24,670,040, 18,654,832 and 17,239,588 bytes; they are its accounting of its own solver and
merge isomorphic runouts, so they are not comparable term by term.

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
| `flop-gate` (step 8, on demand) | `flop_capture` on the self-hosted WSL2 runner (Decision 12) | `flop-project` | Prerequisite host check passes; under 0.5% of pot; peak RSS under the limit plus headroom | Main session reruns exploitability from the capture with the best-response tool |
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
Contents: the amended contract defines attempt identity, optional measurements, observable
preparation, progress/status recovery, cancellation publication, total application budget,
worker release, and snapshot/query ownership. Game identity is separate from the worker-
and storage-dependent estimate key. Canonical encoding and hashing must be specified and
tested before implementing a cache or persistent ID. Snapshots use an attempt and sequence;
iteration is metadata. One browsing snapshot and two explicit finished compare results
are count limits enforced by a registry as well as byte reservations.
Gate: prose check clean; Astra's review recorded in `docs/reviews/`. Step 6 closes the
numerical cancellation and attempt primitives. Before step 11 acceptance, the driver must
pass the contract's cross-game replacement, retained-result, snapshot-pin, allocation-
failure and delayed-delivery tests. The one-second phase 7 acknowledgement target remains
unverified until required-host measurement; capture timings do not establish it.

**5e. Self-hosted flop gate runner.** Depends on nothing in code; Decision 12. Main
session writes the runbook; Caleb installs; the step 8 executor wires the job.
Files: `docs/ci/self-hosted-runner.md` (new), `.github/workflows/ci.yml` (step 8 adds the
job). The runbook covers: WSL2 with Ubuntu, the pinned Rust toolchain, Python and Node for
the reference tooling, the GitHub Actions runner installed as a service under a dedicated
user with the labels `self-hosted`, `linux`, `x64`, `flop-gate`; the runner token entered
from GitHub's settings page and never written to the repository; `wsl --shutdown` and the
`.wslconfig` memory setting so the VM sees at least 16 GB and a swap of zero, which makes
the peak-memory measurement honest; how to confirm the runner is online; how to remove it.
Security notes for Astra: the job runs only on `workflow_dispatch` or a commit-message tag,
the repository is private, pull requests from forks cannot reach the runner, and the runner
user has no access outside its work folder. The prerequisite step in step 8 records the
VM's physical memory and refuses under `memory_limit_mib` plus 1 GiB.
Gate: the runner appears online in the repository's runner list; a dispatch of a trivial
job on the `flop-gate` label completes and prints the recorded memory and CPU.

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
best-response walk normalises strategy sums per node as it reads them. Low-level snapshots
are f64 and budgeted; the step 11 registry must enforce browsing count and invalidation.
`drive` stops measuring on cancel as 5d specifies. The
memory maths in `crates/postflop/README.md` is rewritten from 5c's table.
Invariant: numerically identical results for the river and turn (same arithmetic order per
node).
Tests: river and turn measured hashes unchanged; a range with 37 combos produces
`states == 37`; the table sum still equals the estimate and every reservation; cancel
during traversal prevents a new measurement, and cancellation during a measurement or
callback wins over target/cap publication. Every validated start/resume issues a checked
attempt identity; reports from successful, failed, cancelled and superseded attempts are
tested for stale rejection. Dropped local owners release reservations. The global driver
release barrier and snapshot registry have their own step 11 gates in the 5d contract;
these per-game tests do not claim them. No phase 4 cancellation latency bound is promised.
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
under 10 minutes and prints the estimate. A `flop-gate` job runs on the self-hosted WSL2 runner
(`runs-on: [self-hosted, linux, x64, flop-gate]`, Decisions 7 and 12), on demand, with a
360-minute timeout. Its first step
records physical memory, CPU count, image, and any cgroup or container memory limit, and
fails before the solve when physical memory is below `memory_limit_mib` plus 1 GiB of tool
and OS headroom (the WSL2 VM gets 14 GB of the machine's 16, see
`docs/ci/self-hosted-runner.md`). It then solves the gate tree in f32 and records exploitability, estimate,
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
Before accepting the contracts, implement and verify the driver obligations in
`job-contract.md`: one running attempt, a total application budget across games, worker
release before replacement construction, attempt-bound delivery/status sequencing, and
a snapshot registry with retirement, pins and allocation-failure behavior. Test same-game
and different-game replacement with a retained result and concurrent snapshot queries.
The internal state model is phase 4 work; app transport and rendering are phase 7 work.
Files: `crates/postflop/README.md`, `crates/tree/README.md`, `docs/phase-4/tree-contract.md`
and `docs/phase-4/reference-contract.md` (new, mirroring phase 3), `docs/phase-4/job-contract.md`
(final form), `docs/ROADMAP.md` progress, `ASTRA-UPDATE.md` handing Astra the typed contracts
the app needs together (Decision 5): turn and flop solve request, job lifecycle per 5d,
runout-aware strategy query with reach context, memory estimate display, progress with
runout count.
Gate: driver behavioral tests and `check`; `format`; `slopcheck.py` clean; Astra's review.

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
  (standard 8 GB runners) and `flop-gate` (the self-hosted WSL2 runner of Decision 12, with its resources
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
* **No hosted CI runner has 16 GB.** Standard private-repository runners are 8 GB. Check:
  the self-hosted runner of Decision 12 is online before step 8's `flop-gate` is written;
  the prerequisite step refuses to solve on an undersized host.
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
* **Cancellation latency** can include the remainder of an iteration or a measurement
  already in flight. An observed request starts no new measurement. The turn fixtures
  record iteration, measurement and cancellation timings separately; they do not prove
  the phase 7 host target. Verify driver publication/release ordering and measure the
  required host before app acceptance.
* **Rare-history conditional values** (phase 3 review): `PostflopStrategy` carries reach and
  runout so a consumer cannot quote a river policy without context.
* **The pinned reference is a suspended project.** Fine for a pinned commit; if a future
  toolchain cannot build it, the river capture breaks first and says so.
* **i16 overflow or scale underflow** during discounting. Check: loud failure test;
  per-node scale recomputed each update.
* **A range that breaks symmetry** silently merged. Check: explicit invariance test per
  generator; merging disabled with a logged reason.

## Open questions

Questions 1 to 7 were answered on 2026-09-06 (Decisions 1 to 8); questions 8 and 9 on
2026-09-09 (Decisions 12 and 13). None is open.

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
12. **Flop gate host (2026-09-09):** a self-hosted GitHub Actions runner in WSL2 on Caleb's
    16 GB development machine, labelled `flop-gate`, used only by the on-demand `flop-gate`
    job (Decision 7). Chosen over making the repository public, a paid organisation plan,
    and a rented machine. It also measures the machine the app must ship on. Step 5e adds
    the runbook and the workflow wiring; Caleb installs the runner from the runbook.
13. **Product milestones (2026-09-09):** Caleb accepted all three of Astra's proposals.
    Recorded in `docs/ROADMAP.md` Decisions: one complete play-and-learn loop as a
    milestone between phases 7 and 8; an advice coverage measurement before the library
    scales; a bounded preflop feasibility investigation now, before the tournament work.
14. **Turn comparison thresholds (candidate, 2026-09-10, not yet confirmed by Caleb):**
    the rule in `tests/reference/turn/review_rules.json`: a differing row is indifferent
    when adopting the other side's mix costs at most 1% of pot on both sides' own action
    EVs; unreached when an EV is absent or its reach is under 1e-6 (bound recorded);
    otherwise a real gap, and the real gaps' reach-weighted losses must sum to under 0.5%
    of pot per case. The C rows carry an oracle recomputation held to 1e-9 chips. The
    review noted 1% per row is four times the 0.25% convergence target and nothing caps the
    indifferent rows in aggregate; the record reports the A and all-row totals (1.08e-03,
    4.96e-04, 8.36e-04 of pot today) so Caleb can decide with the number. Until confirmed,
    nothing outside the rules file cites these as decided.

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

### Step 3 executor reports (branch text, merged 2026-09-09)

The step 3 branch's own Progress paragraphs, kept as written. Its "cases are back at 32"
paragraph was superseded by Decision 11 before the merge.

The step 5a numbers are unchanged by the step 3 housekeeping: all three cases still reach
the 0.25% target in 200 iterations at 0.186/0.235/0.185% of pot, because `cases.json` had to
stay at `max_raises: 32` (see the step 3 housekeeping paragraph below).

Step 3 housekeeping: the tree fixtures now carry decision 10's raise rule. The gate menu test
uses `33%,a` bets on the flop and turn, `33%,75%` on the river, and a `100%,a` raise menu on
every street. Its flop tree measures [10, 50, 270] decision nodes, [5, 25, 153] live
continuations, 925 nodes and depth 13. The old 60% raise and river all-in token gave
[10, 50, 384], [5, 25, 209], 1,267 nodes and depth 14. Its turn tree measures [0, 10, 66],
[0, 5, 41] and 214 nodes, against [0, 10, 80], [0, 5, 45] and 256. The planning-anchor test
now measures fourteen decision nodes rather than sixteen and is renamed accordingly. It keeps
the plan's nine live continuations: river [0, 0, 14]/[0, 0, 9] and 39 nodes, turn
[0, 14, 110]/[0, 9, 65] and 346 nodes, flop [14, 110, 598]/[9, 65, 305] and 1,985 nodes. The
stale "awaits hosted Rust verification" sentence is gone from `crates/tree/README.md`.

What the plan did not know: **decision 10's `max_raises: 1` cannot reach
`tests/reference/turn/cases.json`.** The pinned binding takes no raise cap, so the input's
`max_raises` is only a bound `capture.py` checks the export against. With the committed
menus the reference reaches three raises on a street: `bet:4`, `raise:23`, `raise:80`,
`allin:195`. Run 34079922254 confirmed it, because the capture failed its
`max_raises: 1` check on exactly those lines. The cases are back at 32 and the limit is now written down in
`tests/reference/turn/README.md`. Lowering it needs a decision. Change the reference's size
menus so one raise exhausts the stack, drive upstream's `removed_lines` argument to delete
every second-raise line, or accept that the reference comparison runs on a deeper tree than
the gate trees. One improvement was kept: `capture.py` counted wagers across the whole
history, which charges a river bet against the turn's raises, and it now restarts the tally
at every deal.

Step 3: done and CI-verified on run 34082674786 (head ad2b5e6), green on both platforms.
`crates/postflop/src/streets/` adds `PostflopGame`, `PostflopOptions`, `PostflopNodeView`,
`PostflopSolver`, `PostflopStrategy`, `PostflopDecisionValues` and `PostflopMemory` beside the
untouched `river/`, with a `streets` submodule in `crates/bestresponse`. The load-bearing check
is river equivalence: on the three phase 3 fixtures a river-start `PostflopGame` gives the same
regrets, strategy sums, current rows, average rows and exploitability as `RiverGame`, compared
with `assert_eq!` on f64 rather than a tolerance. A called turn all-in agrees with the phase 3
showdown sweep applied once per runout and with a brute-force seven-card enumeration, to under
1e-9 chips. The small turn fixture reached 0.195407% of pot against a 0.5% target in 50
iterations, stop reason `TargetReached`, `nash_conv` 0.039081 chips; its estimate is 49 boards,
48 tables, 537 nodes and a 26,726,760-byte bound, and every `Budget` reservation matched a
component of that estimate exactly. The gate flop tree asks for 89,782,558,132 bytes and is
refused under the 12 GiB default, which is the memory finding at the top of this plan measured
rather than projected.

Four things step 3 learned that the plan did not know. The chance masks and the showdown
tables intern on different keys. A mask depends only on the dealt card and a table only on the
completed board's card set, so a flop tree needs 1,176 tables rather than the 2,352 ordered
runouts the estimate charges for. The estimate needs the per-street sum of action counts, not
just the per-street node counts, because action menus differ inside one street block; it reads
both in one pass over the compact tree. `SolveConfig.threads` and `PostflopOptions.threads` are
recorded and charged for but nothing is parallel yet, so the estimate resolves zero to one
worker rather than to the core count, and step 4 must revisit that when it wires the pool.
And cargo is blocked on this machine after all: Smart App Control refuses `cargo.exe` while
`rustc.exe`, `rustfmt.exe`, `clippy-driver.exe` and `rustup.exe` run, so the step 2 executor's
"local cargo works" does not hold here and CI compiled everything.

Step 3 findings resolved (this branch, on top of da6477d). The nine review findings
are closed. The numerical path is untouched: no arithmetic, ordering or payoff
changed, so every accuracy value step 3 reported still stands.

1. **Runout ranges are disjoint by construction.** `runout_ranges` is gone. The
   expansion records each node's subtree end, and `PostflopGame::subtree(node)` plus
   `outcome_range(chance, outcome)` read the ranges off the chance node itself. The
   contract is per chance node and holds at every level: one node's outcome ranges
   are non-empty, contiguous, in outcome order, and they partition that node's own
   subtree less its root. A flop tree's turn deal and each of its river deals
   therefore partition their own parent's range, not one shared flat list. Step 4
   splits accumulators along `outcome_range` and nests naturally. Checked at both
   levels on the flop-start fixture, and asserted absent on a river-start tree.
2. **Two chance levels are tested.** A new flop-start jam-only fixture expands to
   9,610 nodes over 2,402 boards and 2,352 showdown tables. It constructs and
   iterates, and its called flop all-in matches a seven-card enumeration of all
   36 x 45 x 44 ordered runouts to under 1e-9 chips.
3. **`threads: 0` means one worker per core.** `streets::resolve_workers` answers
   that once through `available_parallelism`, and both the estimate and the solver
   read it, so the charged and the allocated workspaces always match. The traversal
   is still serial on `scratch[0]`, said in `solver.rs`. The bound now charges
   `workers + 1` traversal buffers and scratches, because a strategy query can run
   while an iteration holds its own; the reservation test pins the formula term by
   term and again at `threads: 0`.
4. **Construction transients are charged.** `PostflopMemory.construction_bytes`
   covers the per-board deal table, both interning maps and the validation walk,
   with the math beside the code, and it sits inside `working_set_bound_bytes`. The
   refusal now covers everything a build would have allocated, not only what
   survives it.
5. **The memory limit is configuration.** `config/solver.toml` gains
   `memory_limit_mib = 12288`, parsed with decision 4's 12 GiB default and 16 GiB
   ceiling, and `PostflopOptions::from_config` feeds it to a game. The doc comment
   claiming the limit already came from the file is now true.
6. **Docs.** `crates/tree/README.md` carries the 14-decision anchor (18 against 14,
   two ninths conservative), `crates/bestresponse/README.md` documents the `streets`
   module, and the garbled sentence about run 34079922254 is rewritten below.
7. **Cleanups.** One `STATES` and one `PRIVATE_CARDS` in `streets/mod.rs`,
   `MAX_EXPANSION_DEPTH` removed (the tree's own 128-edge limit bounds the
   recursion), `options()` and `workers()` dropped; `compact_id()` and
   `contributions()` stay for step 5b.
8. **Path validation runs over the expanded tree.** `game::validate_traversal` is
   `Layout::validate_paths` split out: it takes a `TraversalLayout`, a
   compatible-pair predicate and an optional terminal-column source, and checks
   reachability, one unit of chance mass per live pair, and zero-sum terminals.
   `PostflopGame::new` calls it and `validation()` reports what it covered. Each
   check is size gated, because the pair walk is quadratic in the live combos and
   reading a terminal's utilities costs one evaluation per live state: a gate tree
   reports zeroes rather than spending minutes. Callback games take the same path
   with every pair in scope. The turn and flop-start fixtures assert the walk
   covered every node, every deal and every terminal.
9. **Small.** The turn anchor's 110 river decisions are derived in a comment (five
   full blocks of 14 plus four of 10, where both river raise targets clamp into the
   all-in), and the small turn solve now targets decision 3's 0.25%.

Verified on run 34286324836 (head bfb2162), green on both platforms: clippy at
`-D warnings`, the whole test suite, and the turn reference. The measured numbers
are unchanged where they should be and moved only where finding 4 said they would.
The small turn solve still reaches 0.195407% of pot with `nash_conv` 0.039081 in 50
iterations, now against the 0.25% target. Its estimate is the same 49 boards, 48
tables and 537 nodes, with 3,132,273 bytes of construction transients taking the
bound from 26,726,760 to 30,717,913. The gate flop tree now asks for 89,793,081,162
bytes rather than 89,782,558,132 and is still refused under the 12 GiB default. The
flop-start fixture's bound is 421,351,578 bytes, and the seven postflop street tests
run in 51 seconds.

Still open from the step 3 review, all step 6: the river/streets duplication,
`Payoff::Showdown`'s symmetric pair, and the estimate charging 2,352 ordered
showdown tables where only 1,176 card sets exist.

**Step 3 round two.**

Findings A to I are closed on the same branch. The structural half of
`validate_traversal` now runs on every tree: a new `PairScope::NoPairs` turns off only the
two quadratic checks, and with no scoped pairs the walk carries empty live-flag vectors so
it stays linear in the nodes (A). `PostflopValidation` reports `nodes`, `chance_nodes` and
`terminals` for the whole tree and zero `pairs` and `zero_sum_terminals` when those checks
were gated, instead of a pair count nothing walked (C); a full-range turn fixture, 1,128
live combos per player against the 512 x 512 budget, asserts exactly that. The zero-sum
pass keeps its pair matrix, because pairing player zero's column for one opponent state
with player one's would otherwise cost one terminal evaluation per pair rather than per
state; it is charged instead, and the reason is written next to both the walk and the
estimate (B). `validation_bytes` now also charges the showdown scratch, the stack's
doubling, and `max(52, max_actions)` per level (E, H), and a unit test asserts the whole
formula term by term (G), as do exact node, chance, terminal and pair counts in the two
all-in fixtures. The 16 GiB ceiling is `config::MEMORY_LIMIT_CEILING_MIB` with a derived
byte form, used by both game constructors (F). `PostflopColumns` prefills NaN and checks
every column finite, matching `cfr.rs` (I). `crates/postflop/README.md` and the estimate's
own docs now say the `workers + 1` term covers one concurrent strategy query and that a
second one is refused by the budget rather than allocated (D).

Only the charged transients moved. Turn fixture: construction 3,132,273 to 4,205,121
bytes, bound 30,717,913 to 31,790,761. Flop-start fixture: bound 421,351,578 to
422,706,474. Gate flop tree: 89,793,081,162 to 89,793,081,162 bytes, still refused under
the 12 GiB default. Each delta is `(max_depth + 1) * 52 * 2,712` for the doubled stack
plus 85,680 for the scratch. Nothing else moved: the small turn fixture still reports
0.195407% of pot, `nash_conv` 0.039081 chips, 50 iterations, stop reason target reached,
and the river accounting is untouched.

### Decision 11 executor report (branch text, merged 2026-09-09)

Decision 11: done and CI-verified on run 34284399884 (head 7e07ae5; the fully green run at 4147355 is 34285342323), `turn-reference` green with
`cases.json` at `max_raises: 1`. `tests/reference/turn/raise_cap.py` replays upstream's action
tree in Python (`push_actions` and `BuildTreeInfo::create_next`, amounts, clamps, all-in
threshold, sort and dedup) and derives the lines for `init`'s `removed_lines` argument. That
argument deletes an action and its whole subtree, and chance actions are omitted from a line, so
a street change is implicit in the token sequence. All three cases derive the same 18 lines, because they differ only in their
board: the turn chain is `B4-R23-R80` and the deepest river line is
`B4-R23-C-B19-R114-A172`, whose third wager clamps to the stack and is therefore an `A` token,
not an `R`. The lines are derived at capture time rather than committed, recorded in each case's
`removed_lines` beside `input` rather than inside it, and re-derived by `capture.py` whenever a
capture is validated. The reference now solves exactly Decision 10's tree: the turn offers
`check`/`bet:4`/`allin:195` then `fold`/`call`/`raise:23` then `fold`/`call`, and the river
`check`/`bet:4`/`bet:8` then one raise, with a jam only where a raise clamps. New reference
numbers, all three reaching the 0.25% target: dry rainbow 150 iterations at 0.1587% of pot,
paired 200 at 0.1783%, flush 150 at 0.1851%, exporting 426/561/426 nodes with 5 chance nodes
each. Isomorphic merges are unchanged at 0/12/13 of 48, and the suit-swap check now compares
56,615 cells for `2c`/`2d` and 58,305 for `4d`/`4h` at zero difference, against up to 0.599
without the swap. Four things step 5b needs. `max_raises` now accepts 0 to 32 and is a setting
that changes the tree, not only a bound on the export. A capture carries a `removed_lines`
field, so a reader of the artifact can see what was pruned, but `input` is untouched and
`turn_capture.rs` still echoes it verbatim. With donk sizes unset the reference still gives OOP
its ordinary river menu after calling a turn bet (`bet:6`/`bet:14` in a 19-chip pot), confirmed
on this capture, so our tree must match. And the export is smaller than step 5a measured: 21
turn nodes and 135 per runout, so all 48 runouts would be 6,501 nodes and 45 to 50 MB per case,
still over both ceilings. Local guards: 86 Python tests and 14 Node tests, plus `black` and
`ruff check` clean on `tests/reference/turn/`.

### Step 4 executor report (branch text, merged 2026-09-09)

Built on `worktree-agent-ad40c29522beb0497`. At a chance node with more than one
outcome and a mask pool, the CFR walk and the best-response walk map outcomes over a
`rayon` pool. Each outcome is walked by the same code, collected in outcome order and
reduced in outcome order. The first `Err` by outcome index is what is returned.
Accumulators split with `split_at_mut` along `outcome_range(chance, k)`, and a split
that does not match the tree is refused. Nested deals nest the split. `SharedTerminal`
takes `&self` so each worker takes the showdown workspace its own pool index names.
`threads: 1` builds no pool and runs the previous code path. `PostflopSolver::workers()`
reports the pool size, which is the same `resolve_workers` answer the estimate charged.
`rayon` is pinned `=1.12.0`, MIT OR Apache-2.0, recorded in `crates/postflop/README.md`.

Two facts for the later steps. The traversal-buffer term now uses the widest deal rather
than the widest bet menu, **above one worker only**. A parallel chance node holds one
value vector per outcome while a serial one holds one at a time. At `threads: 1` every
estimate is unchanged. And the gate flop tree's refusal is **89,793,081,162 bytes**
under 12 GiB, not the 89,791,021,146 recorded under "Facts the later steps depend on":
that figure went stale during step 3's rounds two and three. Measured on `11904b4` with
step 4 reverted, and identical with step 4 applied.

Small turn fixture unchanged: 0.195407% of pot, `nash_conv` 0.039081 chips, 50
iterations, `TargetReached`, and bit-identical at 1, 2 and 4 workers.

### Step 5c executor report (branch text, merged 2026-09-10)

Built on `worktree-agent-ac40b78d67c3be6ce`, green on both OSes at 50676e0 in CI run
34431642933 and again on the branch head after the round-two review fixes, whose head and
run id are in the executor report. `PostflopMemory::rows_under` reports one row per buffer with its
representation, bytes, lifetime and overlap. The rows the bound counts sum to
`working_set_bound_bytes`, and `MemoryReservation` names, for every `Budget` site, the rows
it draws from, checked against the bytes the budget actually holds during a solve. The two
fixture sums reproduce (31,790,761 with construction 4,205,121, and 422,706,474) and are
pinned in `crates/postflop/tests/streets.rs`. `PostflopMemory::for_tree` prices a tree
`PostflopGame::new` refuses, which is the only way the flop gate can be priced at all. The
table is printed by `crates/postflop/examples/memory_table.rs` and recorded in
`crates/postflop/README.md`. No charged number moved: the aggregate fields are the same
terms regrouped.

Sums at one worker, in bytes, f64 / f32 / i16. Turn gate (9,003 expanded nodes, 3,178
decisions, 11,363,820 entries per stored array), today's three arrays and two snapshots:
470,945,787 / 243,669,387 / 130,094,747; after step 6 (two arrays, no retained snapshot,
live combos): 80,259,139 / 48,001,659 / 31,898,343. Flop gate (1,792,006 nodes, 637,500
decisions, 2,222,795,016 entries), today: 89,793,081,162 / 45,337,180,842 / 23,121,980,682;
after step 6: 14,095,730,858 / 7,423,993,178 / 4,093,224,338; after step 6 with one browsing
snapshot alive: 20,810,476,978 / 10,802,870,458 / 5,806,717,198. `f32` and `i16` are
arithmetic over the same entry counts, as is any row charging fewer than 1326 states.

**Ordering.** The turn gate fits the 12 GiB default today at every width. The flop gate fits
at none of them: 7.0x the limit at f64, 3.5x at f32, 1.8x at i16. Step 6 is required and is
not sufficient. Compacted, with the policy derived and no snapshot retained, the flop gate
still needs 14,095,730,858 bytes at f64, which is 1.13 GiB over the default, though it would
fit the 16 GiB ceiling, which is the machine and not the configured limit. Step 7's f32
closes it at 7,423,993,178 bytes, 5.09 GiB spare, and 1.94 GiB spare with a browsing
snapshot alive. So the flop gate needs 6 then 7, and step 10's i16 is headroom rather than a
prerequisite. One reading to settle: this step's contingency says step 10 moves ahead of step
8 "if the table shows f32 with three arrays over 12 GiB on the gate tree". Literally, f32
with today's three arrays and no compaction is 45,337,180,842 bytes, over. Read as the layout
steps 6 and 7 actually produce, it is 7,423,993,178 bytes, under. Under the second reading
the dependency lines stand as written; the main session decides which reading governs.

**What the plan did not know.** The gate menu at the reference capture's chip scale (pot 11,
stack 195, minimum bet 1) builds the same tree and the same bound as the phase 3 scale (55,
975, 10) the tree test uses, so one table describes both. Live combos of the Decision 9
ranges after board removal are 34.5% to 37.5% of 1326 across the six flops priced, worth
about 2.7x, and on the three turn boards they are exactly the reference's
`private_hand_counts` (469/470, 468/473, 445/448 from the `turn-wasm-reference` artifact of
run 34401787355), which cross-checks our ranges against the reference's input. The estimate
itself depends only on the tree, the board length and the worker count, not on the ranges or
which board: a board changes only the compacted projection. The two retained snapshots are
the largest single term today, 35.65 GB of the flop gate's 89.79 GB, so 5d's "at most one
snapshot alive" is a memory decision and not only a lifecycle one. Everything that is not a
stored entry array costs 651 MB on the flop gate (topology 496 MB, showdown tables 154 MB),
which is 8.8% of the post-step-6 f32 working set, so step 6's flattening of topology matters
much less than the entry width does. An extra worker costs 2.0 MB on the flop gate, which no
ordering decision depends on. The reference's own estimates for the same three turn cases are
24,670,040, 18,654,832 and 17,239,588 bytes; they are its accounting of its own solver and
merge isomorphic runouts, so they are not comparable term by term.

### Step 5b round two

Two commits on `worktree-agent-a9ca9d74a78331558`. `643d803` adds
`PostflopStrategy::node_values`, the capture rows it feeds, and the oracle's refusal.
`2f41c8e` replaces the per-row review with a rule. Run
[34474380677](https://github.com/calebroot2006-art/SOLVER/actions/runs/34474380677) at
`643d803` was green on every job except `turn-compare`, which had no committed review to
read yet; the run on this branch head is the one that judges the gate, and its id is in the
executor report.

**The solver gap is closed.** `node_values(node)` answers at any node, for both players,
sharing `decision_values`' path walk. It reports a value wherever one exists rather than
only where the policy arrives, because a walker that stops at a chance node gets there down
branches the hand takes with probability zero and multiplies by that probability itself;
`reach` beside the value is what says the hand never arrives. `None` means no weight in the
range, a blocked hand, or no compatible opponent left, never zero for missing. It reserves
two decision reports, one above what `working_set_bound_bytes` charges for, and four unit
tests in the crate cover the policy average against `decision_values` (26 rows, within
1e-9), a chance node and both terminal kinds, an unknown node, an unreached hand, and the
reservation. `decision_values`' own numbers are untouched; the river record check and the
existing tests are what say so.

**The named row.** Root row `2c2d` of `turn_100bb_dry_rainbow`, actions check / bet:4 /
allin:195. The oracle on the `643d803` capture gives 14.9576 / 14.6632 / 11.4971, matching
the capture's own `decision_values` to 7.1e-15. Before the accessor it gave 2.2255 / 1.0665
/ 5.1029, because a missing continuation value read as 0.0. On a 160-iteration capture of a
three-hand fixture, all 679 decision rows agree to 4.3e-14, so the agreement is not one
lucky row.

**The rule-based review.** `review_rule.py` sorts every differing row from the numbers both
captures measured; `review_rules.json` holds the three thresholds and nothing else cites
them. Per case, A / B / C and the C reach-weighted loss as a fraction of pot: rainbow
10,998 / 32,193 / 351 at 4.10e-05; paired 17,029 / 35,561 / 189 at 1.35e-05; flush 9,913 /
32,227 / 277 at 1.32e-05. The budget is 5.0e-03. The B rows' own bounds sum to 9.6e-05,
5.0e-05 and 1.0e-04. Linux and Windows give identical counts and sums, and both reach
0.20821 / 0.16611 / 0.20755% of pot at 150 / 200 / 150 iterations. Root EVs agree with the
reference to 0.00337, 0.00091 and 0.00067 chips in a pot of 11.

**Open, and for Caleb.** The three thresholds are the Decision 14 candidate and are
unconfirmed; they live only in `review_rules.json`, and the committed record carries that
file's hash so it cannot outlive them. The B category excuses three quarters of the
differing rows, most of them because one side reports no EV at all; its bound is recorded
per case but nothing fails if it grows.

**Two facts for later steps.** Both standard runners reported four CPUs and about 16 GB,
not the two CPUs and 8 GB under "Facts the later steps depend on". And the capture is now
65,101,178 bytes against the 64 MiB `compare.py` reads: 3% of headroom, down from 6%, so a
fourth case or a fourth exported runout does not fit until step 6 or 7 shrinks it.
`serde_json` is now recorded as a dev-dependency in `crates/postflop/README.md`. The
measured record is `tests/reference/turn/measured/643d803/`, which says how it is
regenerated.

### Step 5b round three

The main session's `/code-review` returned eight findings; all eight are closed on the
branch. Nothing about the solve changed, so every number in the round-two paragraph still
holds except the memory bound.

**The node report is a row of the memory table.** `node_values` reserved two decision
reports through a `Budget` site no `MemoryReservation` named, while the bound charges one
decision report, so a caller who sized `memory_limit_bytes` to the estimate was refused on
the first node query after a solve: in `turn_capture` that is `reported_hands`, after the
solve, with no capture written. The report is now its own row at 85,376 bytes, counted
beside the decision report rather than aliasing it, because one is sized by the widest menu
and the other by the player count. Every bound grows by exactly that: the turn gate to
471,031,163, the flop gate to 89,793,166,538, the pinned fixtures to 31,876,137 and
422,791,850, and the compacted f64 flop gate to 14,095,816,234, still 1.13 GiB over the
12 GiB default with f32 at 7,424,078,554 and 5.09 GiB spare. **Step 5c's conclusion about
the order of the flop work is unchanged**, and `crates/postflop/README.md` is regenerated
from the example.

**Every real-gap row now carries an independent recomputation.** The rule reads the action
EVs each capture reports for itself, so a convention both sides shared would have put every
row in A unnoticed. `review_combos.py` walks each of the 817 rows again with `oracle.py`,
in 1.8 seconds, and records the result; `compare.py` fails a row that carries none, or one
further than `oracle_agreement_chips` (1e-9, in the rules file) from the EVs the capture
being judged reports. Measured worst case 6.86e-13 chips. Seven hundred of the 817 rows are
walked end to end from terminal values; 117 cross a deal and lean on the chance-node values
`node_values` made available.

**Two totals for Decision 14, reported and gated on nothing.** Per case, the reach-weighted
switch loss over the A rows is 9.45e-04, 4.33e-04 and 7.19e-04 of the pot, and over every
differing row, whatever its category, 1.08e-03, 4.96e-04 and 8.36e-04. A rule with no
indifference threshold at all would therefore still sit inside the 5.0e-03 budget. Caleb
decides whether to cap it.

Also: the staleness check no longer counts the action list as evidence, since it survives
any re-solve, and a row recording only its actions is refused; a zero action gap now bounds
a row at zero instead of leaving it unbounded (no recorded count moves, `unbounded_unreached_rows`
is zero on all three cases); `load_rules` reads through `capture.read_json`;
`measured_record.py` reads each 65 MB capture once; `compare.py` refuses an existing
`--output` by name before parsing anything; and the README says that the committed record
is generated from the Linux capture and checked against both, so a Windows-only stale
failure is a determinism failure to investigate rather than a review to regenerate.

### Step 6

Built on `worktree-agent-aeabce3b33204de8a` from 14a02fd. Three commits: ffbf617 (the
cancel path and the progress/stop split), 03cb619 (the flat layout, in-range compaction and
the estimate that follows), and one more for the shared range helpers, the compaction test,
the three clippy fixes, the README and this paragraph. CI run 34521410713 at 03cb619 was
green on eleven of the thirteen jobs, including `Turn solve` on both operating systems and
`Turn project versus reference`; both `Solver` jobs failed on `cargo clippy` alone, on three
lints (two `needless_range_loop`, one `items_after_test_module`) that the third commit
fixes, and every test and the river capture in those jobs still ran and passed. The branch
head and its own run id are in the executor report.

**The invariant.** Every measured value is bit for bit what 14a02fd produced. The evidence
is a temporary test file (`crates/postflop/tests/zz_baseline.rs`, deleted before the
branch was pushed) that hashes each policy on (node id, combo id, action index), which is the same
key before and after compaction, and prints the exploitability bit patterns. On the small
turn fixture at one and at two workers, on the two-chance-level flop fixture and on both the
`RiverGame` and the river-start `PostflopGame`, every hash and every bit was unchanged:
turn policy `0xc6c5e36bcfd872ff`, `nash_conv` `0x3fa4027cc9e87662`, both best-response
values, the root `node_values` report `0x4b497d1a39823ebf`; flop `0x8b9896cb3e0fa982`; river
`0xaf50c5d13e16a867` from both games. In CI the river record's solved fields and the
`turn-compare` stale check on the committed review are what enforce this.

**What moved.** The walk carries one entry per live combo instead of 1326. A combo with no
weight, or one the board prefix blocks, has a zero live mask at every terminal, so its
regrets and strategy sums never leave zero; dropping its row is a projection, not a change
of answer. The terminal boundary scatters the compacted opponent reach into a full-width
vector and gathers the live entries back, because `ShowdownTable` and `evaluate_fold` are
written against combo IDs, and the zeros scatter leaves are the reach those combos already
carried, so every sum is the same sum in the same order. Topology is struct-of-arrays with
one shared edge array; the payoff is an index into a handful of interned records; regrets
and strategy sums are one flat buffer each, sliced by a per-node `u64` offset. The current
policy is derived from the regrets where a walk reads a node, which is before that walk
touches the node's regrets, so it is the row the stored array held. A measurement retains no
average either: regret matching over the strategy sums is the average strategy row by row,
and the best-response walk normalises them as it reads them.

**The numbers, at one worker and f64, over the widest board of each gate set.** Turn gate,
mid-solve: 75,768,147 bytes against step 5c's predicted 80,344,515, which is 4,576,368
under. Flop gate, mid-solve: 13,533,132,510 against 14,095,816,234, which is 562,683,724
under. Three terms explain the gap, and none of them is the storage change: the topology
fell from 495,519,164 to 78,853,380 bytes on the flop because a node no longer owns three
`Vec` headers and a 56-byte inline payoff; the showdown tables halved, from 154,140,672 to
77,070,336, because they are charged per completed board rather than per ordered runout,
which is how the build has always interned them; and the chance row rose, from 1,851,024 to
21,504,060, because the probability and mask arrays run parallel to the whole edge array.
The bound the budget actually enforces charges one browsing snapshot on top: 108,025,923 on
the turn and 20,204,870,486 on the flop. 5c's ordering conclusion stands unchanged. The flop
gate is 618.20 MiB over the 12 GiB default while it solves, inside the 16 GiB ceiling, and
step 7's `f32` closes it at 6,861,394,830 mid-solve and 10,197,263,966 with a snapshot
alive. Step 10's `i16` is headroom, not a prerequisite.

**Cancel and progress.** `drive` measures on `check_every` and at the cap and nowhere else,
so the iteration a solve stops on no longer depends on how fast the host ran;
`log_every_secs` drives the callback alone, and an event between measurements repeats the
last one with `Progress::stale` set. A cancel takes no measurement: `SolveReport` carries
`exploitability: Option<Exploitability>`, `measured_at` and `stale_measurement`, and reports
no measurement at all when the cancel arrived before the first one. Measured on the small
turn fixture at one worker: one iteration 0.2929 s, one measurement 0.5831 s, a cancel at a
loop top 13 microseconds, a cancel mid-iteration 0.2604 s, which is the remainder of the
iteration in flight and nothing else. `turn_capture` drops its 86,400-second progress
interval and takes `config/solver.toml`'s `log_every_secs`, which is 10; it records a
checkpoint only for a fresh measurement, so its recorded schedule is the case file's.

**Contract changes against the 5d draft.** Three, all reported rather than assumed.
`SolveReport` gains the measurement's optionality, iteration and staleness instead of the
draft's `timings` block, because `turn_capture` already measures and records those four
timings itself. `MemoryReservation::Snapshot` is charged once rather than twice, which is
the draft's "at most one alive per job" made enforceable: the budget refuses a second
retained average under a limit sized to the bound. And `JobId` is the pair, not two types:
`PostflopSolver::job` issues a process-unique job number with a generation the cancel path
moves on, and `accept` refuses a result carrying the old pair. `GameId` and `SnapshotId` are
not implemented; nothing in phase 4 has a second process or a second game to confuse.

**Cleanups done.** `scaled`, the pair-underflow check and the root normaliser are one
`src/ranges.rs` shared by the river and the street-aware modules. `Payoff::Showdown` is one
record in both, not a symmetric pair. `showdown_tables` charges 1,176 on a flop tree rather
than 2,352. `node_values` and `decision_values` share `opposing_mass` and the
divide-and-check block as well as `path_reaches`, and `path_reaches` builds only the live
masks its caller walks with. `from_rows` in both modules delegates to one flattening
constructor.

**Cleanups deferred, and why.** `evaluate_terminal` and the `Payoff` enum stay one per
module: the two now differ in shape, since the street-aware one scatters and gathers and the
river one does not, so merging them would put the compaction branch inside the river's hot
path for no gain. `RiverSolver` and `PostflopSolver` stay separate for the same reason.
`river/memory.rs`'s formulas are unchanged and now over-charge the topology term, which is
deliberate and commented: it is a bound either way, and the accepted river record pins the
number it produces. `decision_values` stays duplicated because the river's has no board,
runout or compaction to carry. `path_reaches` still recomputes reach from the root per
query; caching it needs mutable state on a shared immutable strategy, which is a design
question for the browsing API rather than a tidy-up.

**Open.** The river record's `working_set_bound_bytes` and `reserved_bytes` are now 136
bytes above the accepted record rather than the 24 the step 2 review recorded: 24 from the
`mask_pool` Vec header charged since step 2, and 112 more because
`size_of::<TraversalLayout>()` grew when the topology became struct-of-arrays. No solved
field moves, and nothing in CI compares those two fields, so this is a note for the next
reconciliation of the record rather than a failure. Peak RSS is still unmeasured against the
table on any host; that is 5c's acceptance step and it needs the flop gate runner.
