---
type: contract
status: reviewed-design-implementation-in-progress
date: 2026-09-10
---

# Solve job lifecycle contract

Phase 4 step 5d, amended after Astra's review at `d9c979d`.
This contract covers the owned solver primitives and the application driver that
must exist before step 11 accepts the turn and flop contracts together.
Decision 5 still excludes a turn-only app feature. The table at the end
distinguishes implemented primitives from driver work; this document does not
certify unimplemented behavior.

## Identities and estimate keys

The existing `JobId` is a pair: a process-unique `u64` solver number and a
`u32` generation. Generation zero means no driver invocation has started.
Every validated start or resume increments the generation, including a start
that subsequently fails. Invalid configuration refuses before issuing an attempt.
Both counters use checked exhaustion; neither saturates nor wraps.

Progress and `SolveReport::job()` carry the producing attempt. Cancellation and
execution failure close acceptance immediately; a subsequent start or public
manual iteration also makes old reports ineligible. `accept(report)` checks
the report's embedded identity. Callback and legacy river sessions currently
return no job identity; the application driver must wrap every backend in its
own attempt-bound envelopes. Error, progress, result and query envelopes all
carry that identity and are checked again at delivery.

A future `GameId` identifies canonical game inputs, not an allocation estimate.
The encoding must include board order, exact range weights in canonical combo
order, complete betting rules and tree configuration. Specify and test the
versioned encoding and hash before implementing a cache or persisted ID; the
current crate implements neither. A digest never substitutes for canonical
input equality on a cache hit. There is no ad hoc hash fallback.

An estimate cache additionally keys resolved worker count, storage plan, memory
limit, implementation version and platform layout assumptions. Target and
iteration cap remain run parameters. A cached estimate cannot be reused after
any size input changes.

`SnapshotId = (attempt, snapshot_sequence)`, with a checked monotonic sequence.
The completed iteration is metadata, since two replacements can occur at the
same iteration. Retired IDs return `SnapshotGone`.

## Admission, replacement and release

The future driver owns at most one running attempt and one total application
budget. That budget covers retained games, snapshots, reports, preparation and
new jobs across different games. The crate's existing per-game `Budget` and
`Lease` are necessary local accounting; they do not implement this total limit.

Request validation returns observable preparing, refused or accepted state.
Acceptance requires successful reservation before worker start and includes the
attempt, estimate and reserved bytes. Invalid input and byte shortages preserve
their named errors. A replacement blocked by the current worker returns a
busy/releasing state, not a fabricated `MemoryLimit` error.

A replacement may prepare bounded validation metadata while the old attempt is
stopping. It cannot reserve or construct an expanded replacement game until
worker release, for either the same game or a different game. Worker release
means accumulators, scratch, traversal buffers and worker threads are gone.
Retained results and their shared game remain separately charged until the last
browser or query owner releases them. Result disposal is therefore distinct
from worker release.

Do not promise a preparation latency from one fixture. Record request,
acknowledgement, worker start and release with monotonic clocks, and support
cancellation during preparation. The host gate measures those stages.

## Progress and measurements

Use pushed progress coalesced to the newest state, with a read-only status query
for attachment and recovery. A slow consumer cannot grow an unbounded queue or
delay solving. Events and status replies carry attempt and monotonically
increasing sequence; discard duplicates and older sequences. A gap causes a
status refresh. Terminal state stays queryable when its event was missed.

A measurement is optional and carries the iteration it covers. Measure only on
`check_every` and the iteration cap. `log_every_secs` controls progress cadence
without initiating measurement. Derive staleness from measured and completed
iterations; before the first measurement display "not measured".

The existing `Progress` and `SolveReport` contain optional exploitability,
`measured_at`, derived staleness and elapsed time. The future driver serializes
those as one coherent measurement record. Iteration time, measurement time,
cancel acknowledgement and worker release latency are separate diagnostics,
measured by the component that observes each stage. The capture example's
diagnostics do not imply those fields already exist in every solver report.
Wall-clock timestamps serve display; durations use monotonic time.

## Cancellation and terminal publication

The numerical driver checks cancellation before and after complete iterations,
after an already running measurement, and after a progress callback.
Cancellation observed after an iteration prevents a new measurement.
An in-flight measurement may finish and its valid value may be retained;
cancellation observed before terminal publication wins over target or cap.

A cancelled report states the last completed iteration and carries the last
measurement this invocation took, if any. That measurement may be stale.
No partially updated strategy may be exposed. The low-level session can resume
under a new generation; a replacement through the application driver must also
satisfy its release barrier.

The application driver serializes cancellation and terminal publication. Once a
completion has been published, cancellation returns the completed status.
Requests racing after the numerical driver's final observation are resolved at
that delivery boundary, not by pretending the worker can observe a flag forever.

The phase 7 target is acknowledgement within one second of request and worker
release within one iteration after acknowledgement. Phase 4 reports measured
latency; it does not guarantee that target. Faster cancellation inside an
iteration requires rollback or disposal of partial accumulators plus a valid
snapshot of the last complete iteration. Poisoning alone does not recover it.

## Snapshots and queries

A query reads a snapshot taken at a completed iteration boundary, never live
accumulators. The step 6 low-level snapshot is f64; f32 and i16 remain later
storage work. Snapshot creation, retained capacity, query workspaces and returned
reports all reserve before allocation. A diagnostic current-policy row also
retains its reservation until dropped; it does not certify convergence.

The driver registry permits one browsing snapshot per running job. The byte
budget alone does not enforce that count. Replacement is serialized: retire
the old ID, wait for outstanding query pins to release or reserve their overlap,
then create the replacement. If creation fails, status reports no current
snapshot and the old ID stays retired. Test this failure explicitly.

A finished comparison may retain two explicitly selected results, each charged
and shown in the admission estimate. A finished result may also be compared
with the running job's browsing snapshot if all allocations fit. This permits
no second concurrent solve. Final snapshot creation is fallible and occurs
before the worker is disposed; the budget must cover that overlap.

Queries carry snapshot identity, node and optional combo. Answers carry street,
board, runout, values, own reach, compatible opponent mass and the normalized
policy. Suit-merged queries later identify their representative and permutation.
Missing values remain missing; zero reach does not by itself erase a conditional
node value. The consumer rejects stale query envelopes at delivery.

## Implementation and acceptance

| Obligation | Current scope | Gate |
|---|---|---|
| Measurement schedule, optional measurement, cancellation boundaries | Implemented in the numerical driver | Step 6 tests, including cancellation during step, measurement and callback |
| Fresh attempt IDs, embedded report tags, closed-attempt rejection | Owned street solver primitive implemented | Step 6 success/failure/cancel/resume/manual-step/exhaustion tests |
| Complete per-game allocation accounting and leased reports | Step 6 correction pass | Capacity/refusal/drop tests, allocation counters, memory row accounting |
| Total driver budget and replacement/release across games | Required driver work | Before step 11 acceptance |
| Snapshot registry, pins, invalidation and count | Required driver work | Before step 11 acceptance |
| Attempt-bound delivery, sequences, recoverable terminal status | Required driver state model | Before step 11 acceptance; transport and rendering in phase 7 |
| Host acknowledgement/release targets | Unverified | Required host measurements before app acceptance |

Driver tests cover replacement on the same and different games while retaining
a result, including refusal when the remaining budget is insufficient.
Test cancellation during preparation/measurement and late responses from
successful, failed, cancelled and superseded attempts. Also test concurrent
snapshot replacement and queries, allocation failure after retirement, and
release after the last query pin.
Step 11 cannot accept those obligations from the low-level budget tests alone.
