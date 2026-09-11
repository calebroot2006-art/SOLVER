# Step 5d lifecycle review: needs changes

Reviewed by Astra on 2026-09-10. Draft: `docs/phase-4/job-contract.md` at
`2f81339`; implementation: `e338d7a`. This report supplies the overdue design
decisions and the changes Fable must make to the contract. It does not amend
Fable's files or certify the incoming lifecycle implementation.

## Decisions within Astra's remit

Use pushed progress events, coalesced to the newest state, with a read-only status
query for initial attachment and recovery. A slow window must not hold an
unbounded event queue or delay a solver. Every event and status reply carries the
same attempt identity and an increasing sequence. A gap prompts a status refresh;
duplicates and older sequences are ignored. Terminal state must remain queryable
after an event is missed. Wall-clock timestamps are for display; elapsed time and
latency use a monotonic clock.

One browsing snapshot per running job is the default. It is enough to inspect a
solve while it advances. A compare view may retain two explicitly selected
finished results, with separate reservations and an estimate shown before the
second is loaded. It must also be possible to compare a finished result with the
one browsing snapshot if their combined allocations fit. This decision grants
no second concurrent solve and no uncharged copy. Snapshot replacement must be
serialized through a registry; a retired ID returns `SnapshotGone`. An in-flight
query pins what it reads until it finishes, so replacement either waits for that
pin to release or charges the overlap before allocating.

Measurements occur on `check_every` and the iteration cap. Progress cadence never
starts a measurement. Keep an optional measurement and its iteration in both
progress and final reports. Display "not measured" before the first measurement
and the measured iteration when the value is old. Derive staleness from the two
iterations; do not allow contradictory independently supplied fields. Keep
timings as separate diagnostics: optionality does not replace the timing data
the progress and cancellation interface needs.

Cancellation observed at the completed-iteration boundary takes precedence over
starting another measurement. An already running measurement may finish, but a
request observed before terminal publication produces a cancelled result carrying
that measurement, if valid. Once completion has been published, cancellation
returns the completed status. These points define the race; avoid promises that
depend on detecting the exact instant a flag changed.

## Contract findings and closure

**J1, high: replacement and release are not implemented by the current budget.**
The cancellation rules at draft lines 77–91 require release before any replacement,
including on another game. `PostflopSolver::solve_with_cancel` leaves its CFR
buffers and lease alive; `PostflopGame` owns a separate budget per game. The tests
exercise dropping a solver and trying another reservation. They do not establish
a driver-wide release barrier or a limit across distinct games.

Define a driver with at most one running attempt and a total application budget
covering retained games, snapshots, reports, and new jobs. A replacement request
can prepare bounded validation metadata, but cannot reserve or construct the new
expanded game before the old worker releases. A refusal caused by the lifecycle
barrier should name a busy/releasing state, rather than inventing a byte shortage.
Keep byte shortages as `MemoryLimit { required, limit }`.

The draft also calls release "every buffer the job held is freed", then retains a
final snapshot. Define worker release separately from result disposal: accumulators,
worker scratch, traversal buffers, and worker threads are gone at worker release;
the result and its shared game remain charged until the last result/query owner
drops them. Closing the browser releases those remaining reservations. Test both
same-game and different-game replacement with a retained result, including refusal
when the retained result leaves too little space.

**J2, high: snapshot lifetime and invalidation are only prose.** The budget counts
one snapshot in its estimate; it does not enforce an object count of one. Available
slack can admit more. `average_strategy` returns an owned f64 policy, without a
`SnapshotId`, registry, generation, or automatic replacement. A snapshot cannot be
freed while a query still borrows or owns it. The draft's free-first replacement
must account for that lifetime, and for failure after retiring the old snapshot.
Test concurrent replacement/query requests, failure to allocate the replacement,
old-ID queries, final snapshot creation, and release after the last pin. Do not
describe step 6's f64 snapshot as the future f32/i16 format.

**J3, medium: identities do not bind results to an attempt.** R6 in `findings.md`
reproduces reuse after a successful solve. `JobId` being a pair type is acceptable,
but the fields need one meaning across the producer and consumer. Issue a fresh
attempt identity on every accepted start/resume; carry it inside progress, report,
snapshot, and error envelopes, rather than trusting a caller to supply an arbitrary
pair to `accept`. Invalidate superseded attempts and reject late results at the
driver boundary. Specify checked counter exhaustion; saturation and wraparound
are incompatible with the promise never to reuse an identity.

Use `(attempt, snapshot_sequence)` as the snapshot identity and store iteration as
metadata. The draft's `(JobId, iteration)` can reuse an ID when a result is replaced
twice without advancing the iteration. Test successful, cancelled, failed, and
superseded attempts, including a delayed response from each.

**J4, medium: the estimate cache key omits a size input.** Draft lines 18–22 exclude
threads from `GameId` while allowing the ID to key a cached estimate. The estimate
depends on resolved worker count. Keep game identity separate from an estimate
key that includes resolved workers, storage plan, implementation/version, and the
platform assumptions used by `size_of`. Verify canonical input equality on a
cache hit. The phrase "BLAKE-style" followed by an FNV fallback is not a hash
specification; choose a documented algorithm and encoding before persistence or
cross-process use. An internal hash is not proof of input equality.

**J5, medium: cancellation promises exceed the implemented boundary.** R5 confirms
that a request during a scheduled iteration can start a new measurement. Fix that
before accepting step 6. The draft's future chance-node cancellation is also not
automatically a numerical no-op. Returning after some regret/sum updates requires
rollback or disposal of the partial solver and a snapshot from the last completed
iteration. Poisoning prevents reads after failure; it does not recover that state.
Preserve iteration-boundary cancellation for this phase. Before adopting a faster
boundary, prove snapshot validity and no partial-result publication, then measure
acknowledgement and release on the required host.

**J6, medium: timing and acknowledgement claims need an owner and evidence.** A
capture's timing fields are not `SolveReport` diagnostics for arbitrary callers.
The draft's subsecond/few-second preparation claims are not established for every
admitted tree. Define bounded request validation, then observable preparation and
worker-start states; report measurements for the required fixtures. The one-second
app cancellation target remains a phase 7 target, not a phase 4 guarantee. Include
cancellation during preparation and during an already running measurement, not
only the capture's unscheduled iteration probe.

## Acceptance boundary for the amended plan

| Obligation | Current implementation | Required milestone |
|---|---|---|
| Optional measurement, measured iteration, progress independent of measurement | Present | Retain in step 6 |
| No new measurement after observed cancellation | R5 fails | Fix and re-review step 6 |
| Unique attempt identity with stale rejection | R6 fails; caller-supplied check only | Fix primitive in step 6; bind envelopes in driver |
| Complete memory charges and allocation bounds | R1–R4 fail | Fix and re-review step 6 |
| Global replacement ordering, worker release and final result ownership | No driver | Explicit driver work before step 11 contract acceptance |
| Snapshot registry, invalidation, concurrent queries | Not implemented | Before step 11 contract acceptance |
| Event/status transport and browser rendering | Not implemented | Phase 7; consume the same tested state model |
| App latency target | Unverified | Measure on required host before app acceptance |

Fable must amend both the draft and the step 6/11 closure lists to reflect this
boundary. Deferring transport is reasonable; treating unimplemented lifecycle
rules as passing tests is not. No turn-only feature is authorized. Step 6 remains
unmerged until its corrections are reviewed; this report does not approve a
blanket deferral of its existing requirements.
