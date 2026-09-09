---
type: contract
status: draft
date: 2026-09-09
---

# Solve job lifecycle contract

Phase 4 step 5d (Astra's finding R7). This is the internal contract between the solver
crates and whatever drives them, first the capture examples and later the Tauri commands
Astra wires. It is drafted before step 6 so the storage refactor implements it, and it is
handed to Astra with the flop contracts in step 11 (Decision 5: no turn-only app feature).
Names are Rust identifiers in `crates/postflop`; the wire form for the app is decided with
Astra in phase 7 and mirrors these fields one for one.

## Identities

* `GameId`: a 32-byte BLAKE-style hash (the exact function is chosen in step 6, from the
  pinned dependency list or a hand-written FNV-1a if no hash crate is added) over the
  canonical bytes of: board, both ranges as canonical strings, the full
  `PostflopTreeConfig`, the precision, and the memory limit. Two requests with the same
  `GameId` describe the same game and may share a cached estimate. The id never includes
  thread count, target, or iteration cap, which are run parameters, not game identity.
* `JobId`: `u64`, unique for the process lifetime, issued by the driver, never reused.
* `Generation`: `u32`, incremented every time a job on a `GameId` is started or cancelled.
  A result carries the generation it was produced under.
* `SnapshotId`: `(JobId, iteration)`. A snapshot is a compact copy of the average strategy
  taken at an iteration boundary.

## Request and acknowledgement

`SolveRequest { game: GameInputs, run: SolveConfig, memory_limit_bytes, threads }`.

The driver answers before any worker starts, within the time it takes to build the
compact tree and run the estimate, which is under a second for the turn and a few seconds
for a flop tree:

* `Accepted { job, game_id, generation, estimate: PostflopMemory, reserved_bytes }`, or
* `Refused { game_id, reason }` where `reason` is `MemoryLimit { required, limit }`,
  `Config(String)`, or `InvalidGame(String)`, the same variants `SolveError` has today.

Acceptance means the budget reservation succeeded (`Budget`/`Lease` in
`crates/postflop/src/memory.rs`). Worker start, iteration, and completion are separate
events; a consumer must never treat `Accepted` as "solving".

## Progress

`ProgressEvent { job, generation, sequence: u64, iteration, exploitability:
Option<Exploitability>, elapsed, timestamp, stale_measurement: bool }`.

* `sequence` increases by one per event for the job; a consumer that sees a gap knows it
  missed events and a lower sequence than the last seen is discarded.
* `exploitability` is `None` until the first measurement (`check_every` iterations or
  `log_every_secs`); after that it repeats the last measurement with
  `stale_measurement = true` on events between measurements, so the number shown is
  always labelled with whether it is current.
* Iteration time, measurement time, and cancellation latency are reported separately in
  the final report (`SolveReport` gains `timings: { mean_iteration, last_measurement,
  cancel_ack, cancel_release }`, all `Duration`). Phase 4 step 5b records all four for the
  turn gate; they are the numbers phase 7 designs the progress screen around.

## Cancellation

Three recorded moments, each a timestamp on the job:

1. `requested`: the driver set the cancel flag.
2. `acknowledged`: the worker observed the flag. Today that is between iterations
   (`drive` in `crates/postflop/src/solver.rs` checks `should_cancel` once per loop), so
   acknowledgement waits for the current iteration to finish. Step 6 keeps that
   granularity for phase 4; the app-ready gate below says what phase 7 needs.
3. `released`: every buffer the job held is freed and the `Lease` returned to the
   `Budget`.

Rules:

* Cancel does not run a best-response measurement. `drive` returns the last measurement
  with its iteration, marked stale when older than the final iteration (step 6 changes
  `solver.rs:93-107`, which today measures after observing the cancel).
* A replacement job on any game may reserve only after `released`. Reserving earlier is a
  `MemoryLimit` refusal, not a wait.
* A completion, measurement, or progress event whose `(job, generation)` does not match
  the driver's current pair is rejected and logged, never delivered. This is what makes a
  late result from a cancelled worker harmless.
* `StopReason::Cancelled` reports are still valid reports: the strategy at the cancelled
  iteration is exposed with its stale measurement, never a partial iteration (the NaN
  poisoning discipline in `cfr.rs` guarantees no half-updated row is readable).

App-ready cancellation gate, for phase 7 to hold the app to: `acknowledged` within one
second of `requested`, `released` within one iteration after that. If phase 4's turn
timings show an iteration longer than a few seconds on the flop, phase 7 adds an
in-iteration cancel check at chance-node boundaries; that is a numerical no-op because it
only decides whether to finish the iteration.

## Snapshots and browsing during a solve

* A query never reads the accumulators during an iteration. Browsing reads a snapshot.
* `Snapshot { id: SnapshotId, precision, bytes }` is taken at an iteration boundary on
  request, from the strategy sums (normalised per state row at read time, uniform when a
  row is all zero, the same rule as step 7's decoding). It is stored compactly (f32 or the
  i16 form of step 10) and charged against the job's budget; taking one that does not fit
  is a `MemoryLimit` refusal.
* At most one snapshot is alive per job by default. Taking a new one frees the old one
  first, so a consumer holding an old `SnapshotId` gets `SnapshotGone` on its next query.
* The final report's strategy is a snapshot too, taken after the last iteration; the job
  releases its accumulators once that snapshot exists, which is when `released` fires.
* The memory table of step 5c lists the snapshot row with its precision and lifetime; the
  design target there is zero snapshots retained by the solver itself, with the
  best-response walk normalising sums as it reads them.

## Strategy queries

`StrategyQuery { snapshot: SnapshotId, node: NodeId, combo: Option<Combo> }` returns
`PostflopDecisionValues` (street, board, runout, values, own reach, opponent mass) plus the
normalised row. The reach context travels with every answer so a consumer cannot quote a
rare-history river policy without it (phase 3 review). Under suit merging (step 9) a query
for a member runout returns the representative's row at the permuted combo index and says
so in a `merged_from: Option<Card>` field.

## What phase 4 measures and what it promises

| Quantity | Measured where | Promise in phase 4 |
|---|---|---|
| Acknowledgement latency | step 5b turn capture, cancelled at a known iteration | reported, not bounded |
| Release latency | same | reported |
| Mean iteration time | step 5b, step 8 | reported per host |
| Measurement time | step 5b, step 8 | reported |
| Snapshot bytes | step 5c table | equals the table's row |
| Stale-result rejection | step 6 lifecycle tests | tested |

## Closure checks (step 6)

Tests above the numerical core, in `crates/postflop/src/streets/solver.rs` tests:
cancel during traversal and during measurement; start a replacement job on the same
`GameId` and on a different one; assert `released` before the replacement's `Accepted`;
assert a late completion with the old generation is rejected; assert a snapshot taken
mid-solve equals the average strategy at that iteration; assert two concurrent snapshot
requests leave exactly one alive. Astra confirms in review that browsing a result while
solving fits the declared memory budget.

## Open for Astra

* Whether the app wants progress pushed (events) or polled; the contract supports both,
  since `sequence` makes polling safe.
* Whether one snapshot per job is enough for the solution browser's compare view, or
  whether phase 7 needs two, which the budget then has to charge.
