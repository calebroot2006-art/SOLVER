# Step 6 correctness review: needs changes

Reviewed by Astra on 2026-09-10, completed after Caleb resumed the saved checkpoint.
Reviewed implementation: `e338d7a853f10bf819bf31728e23ac4f1fac802b`, against
`14a02fd`. Main remains at `2f813392d99ce701abd09a6618f1feb6e00e55e2`.
Neither original checkout was edited. Step 6 remains unmerged.

The complete step 6 diff was reviewed. The defects below prevent acceptance.
The overdue 5d, 5b, and phase 5/6 assessments are separate reports in this directory.
Locations refer to the implementation at `e338d7a`, before probe additions.

## Confirmed findings

### R1, high: construction exceeds the admitted memory bound

`crates/postflop/src/streets/memory.rs:794` omits the temporary `NodeBuild`
array and its child/probability/mask allocations. `streets/game.rs:654` reserves
one build record per expanded node. `game.rs:708` allocates the flat layout while
those build records and their buffers remain alive.

The review example uses the approved flop gate menu, board `9c 5d 2h`, and one
compatible combo per range (`AcAd`, `KcKd`). The public constructor admits the
game at exactly its estimated limit. A counting allocator observed:

| Quantity | Bytes |
|---|---:|
| Complete admitted working-set bound | 227,544,422 |
| Shared reservation | 178,329,552 |
| Declared construction transients | 8,552,918 |
| Additional live allocation peak during construction | 263,339,447 |
| Additional retained allocation after construction | 110,810,362 |

There are 1,792,006 expanded nodes. Construction alone exceeds the complete
bound by 35,795,025 bytes. The counter excludes all pre-existing allocations,
including the input tree, so this is a lower bound on job allocations. It counts
requested allocation sizes, not RSS or allocator overhead.

Correction: include every overlapping build allocation, or construct the flat
layout without retaining a second topology. Validate dense and sparse ranges;
large CFR arrays must not conceal missing construction terms. Closure: the
saved allocation probe must fit the admitted bound or receive a named refusal
before the excessive allocation. Recompute the memory table and gate figures.

### R2, high: imported flat strategies retain uncharged capacity

`crates/postflop/src/streets/strategy.rs:194` charges `values().len()` instead
of the owned vector's capacity. `from_values` retains that vector unchanged.
A valid uniform policy with excess capacity was admitted under an 8,275,265-byte
game limit while retaining 8,275,272 bytes in that vector alone. The snapshot
reservation increased by only 41,064 bytes.

Correction: account for retained capacity before accepting ownership, or reduce
the allocation to its validated length with any temporary peak also covered.
Closure: a valid-length, excess-capacity import must refuse or charge its actual
retained allocation. The saved test exercises this public API without corrupting
internal state. Also inspect `from_rows`: flattening allocates before the budget
reservation, and the old river capacity test now resizes the row into an invalid
shape, so it no longer tests capacity accounting.

### R3, medium: owned diagnostic rows bypass the budget

`crates/postflop/src/streets/solver.rs:266` and
`crates/postflop/src/river/solver.rs:86` now return newly allocated `Vec<f64>`
rows through `Cfr::current_row`; neither takes a lease. The previous API borrowed
the stored row. Holding 32,326 rows retained 8,275,456 payload bytes under an
8,275,265-byte game limit, with a reservation increase of zero.

Correction: use a returned object with a lease, or explicitly caller-owned
output storage with a documented ownership boundary. Closure: retained output
must be accounted for and release its reservation on drop; repeated requests
must not silently exceed the declared limit. The saved probe demonstrates the
postflop path; the river wrapper uses the same unreserved allocation path.

### R4, medium: terminal scratch omits boxed payloads

`crates/postflop/src/streets/memory.rs:787` uses
`size_of::<TerminalWorkspace>() + 128`. The workspace in
`streets/terminal.rs` holds two `Box<[f64; 1326]>` values. `size_of` includes
their pointers, not their 21,216 payload bytes. On this Windows build the row
reserves 85,824 bytes; the inline workspace plus heap payloads occupy 106,912
bytes, before wrapper overhead. The reservation is short by 21,088 bytes.

Correction: add both heap payloads and account for the worker/query wrappers.
Closure: compare the reservation to owned allocation sizes, then update worker,
query, row-sum, and table expectations. This omission affects every worker and
each query workspace, regardless of whether other estimate rows have slack.

### R5, medium: cancellation can start another measurement

`crates/postflop/src/solver.rs:129` polls cancellation before `step`, then
`:144` uses that saved answer to decide whether to measure. If cancellation
arrives during a scheduled iteration, the driver starts a new best-response
walk after the request. The deterministic session probe sets the flag inside
`step`; the driver starts one measurement and only then returns `Cancelled`.

Correction: observe cancellation again at the completed-iteration boundary
before starting a measurement. Specify precedence when cancellation and target
completion coincide. Closure: cancellation during a scheduled iteration must
not start a new measurement; cancellation during an already running measurement
must have separately stated latency. The existing capture probe uses
`check_every = u64::MAX`, so it cannot catch this case.

### R6, medium: successive solve attempts share an identity

`crates/postflop/src/streets/solver.rs:200` accepts any matching pair, but
`solve_with_cancel` increments the generation only after cancellation (`:310`).
Two successful invocations, capped at iterations 1 and 2, both used the same
job and generation. The second invocation accepted the first report as current.

This contradicts the draft's generation-on-start rule and the public attempt
identity wording. Correction: define and implement attempt boundaries in the
driver, bind results to the issued attempt, and invalidate superseded attempts.
Closure: delayed results from successful, failed, and cancelled prior attempts
are rejected after a replacement starts. Also specify counter exhaustion;
saturating generation counters eventually stop invalidating old results.

### R7, low: missing sums guard is confirmed, but requires private-state corruption

`crates/postflop/src/cfr.rs:475` splits the sums slice without the length check
applied to regrets. A probe that truncates the private sums buffer panics at
the second `split_at_mut` instead of returning `InvalidGame`.

The supported construction path allocates both arrays at the same length
(`cfr.rs:77`), exposes no mutable buffer accessor, and splits both arrays by
the same lengths. No public input producing unequal lengths was found. Treat
this as defensive hardening, not evidence that ordinary solves panic or race.
Correction: check both slices, preferably through one checked splitting helper.
Closure: the intentionally malformed private-state probe returns the named
error. The probe explicitly records its unsupported precondition.

## What was inspected and verified

Astra personally inspected the CFR update and split paths, layout flattening,
compaction maps, terminal scatter/gather, best-response row sourcing, strategy
queries/imports, memory estimates, driver, lifecycle API, and changed callers.
The review includes all 33 changed files, their callers, tests, examples, and
documentation. It covers the three named risks: worker slices, terminal
projection, and allocation accounting.

The worker range check enforces increasing, disjoint node ranges; expansion is
depth first and row offsets are monotone. Mutable slices remain disjoint through
safe Rust slicing. The terminal clears the full opponent vector before every
scatter, uses increasing combo-ID maps, and gathers through the same maps.
No changed terminal summation result or overlapping worker row was demonstrated.
Flat row offsets follow node order; the parent policy is derived before that
node's regret update, and positive normalization uses the previous arithmetic.
The changed report adapters preserve combo-ID lookup through the compact maps.
River capacity-test weakening is covered by R2. Existing memory row tests prove
that formulas agree with reservations, which does not prove those formulas price
every allocation. R1 and R4 demonstrate that distinction.

These observations do not override the confirmed memory and lifecycle findings.

| Check | Result |
|---|---|
| Personally queried CI run `34524553796` | All 13 jobs passed at the exact reviewed head |
| `cargo test -p postflop --lib --offline`, untouched probe checkout | 54 passed, exit 0 |
| `cargo test -p postflop --lib astra_review --offline -- --nocapture --test-threads=1` | Six expected contract assertions failed; shell exit 1; defects reproduced |
| `cargo run -p postflop --example astra_memory_probe --profile test --offline` | Construction bound assertion failed, child exit 101 |
| River integration tests | 7 passed before the earlier stop request |
| `cargo test -p postflop --test streets --offline`, resumed run | All 15 passed, exit 0; includes river projection, 1/2/4-worker turn, nested flop, and independent called-all-in enumeration |
| `cargo test -p toygames --offline`, resumed run | All 17 integration tests passed, exit 0; Kuhn, Leduc, failure paths and independent scalar/history oracle |
| Before/after turn capture comparison, both OSes | 2,501,439 fields each; no solved field changed |
| Raw/refined river vs accepted record, both OSes | No solved field changed; memory fields +136 bytes |
| Current turn joint gate, both OSes | Accepted, zero missing/stale rows; gate limitations documented in the 5b review |
| Fresh oracle generation on Linux capture | 817 C rows reproduced; worst difference 6.856737400084967e-13 chips; 700 independent showdown paths, 117 continuation-dependent |

The local Rust compiler worked despite the older repository note about Smart
App Control. No security setting was changed. Probe commands ran locally on
Windows, with Cargo 1.98.1, against `e338d7a` plus review-only tests. None of the
six failing probes changes production logic or weakens an existing assertion.

## Reporting corrections and closure order

The incoming handoff calls table estimates "measured" memory. The table computes
allocation estimates; it is not a peak-memory measurement. R1 disproves one admitted
bound even before allocator overhead or RSS. Reprice the table after the fixes,
then perform the required host measurement. The ordering conclusion still stands:
the current f64 flop estimate already exceeds the default limit, and these missing
terms do not make it smaller. Step 7 is still required, but its predicted fit must
be checked against corrected estimates and the real host.

Two documentation errors need correcting with that table: the README says a live
compacted flop snapshot adds 17.8 GB, whereas the adjacent totals differ by
6,671,737,976 bytes; and the `solver_bytes` and `working_set_bound_bytes` field
comments still describe a stored current-policy array or two snapshots. These
are reporting errors, not new numerical defects.

Fable's next implementation pass should close R1, R2, and R4 first, then R3's
ownership boundary, R5/R6 lifecycle behavior, and R7's defensive guard. Include
the lifecycle contract amendments in `job-contract-review.md`. Review the new
revision and rerun the affected gates; do not regenerate a numerical baseline to
hide a moved policy. The twelve earlier quality findings remain in Fable's plan
and were not erased or treated as numerical acceptance.

## Saved reproductions and evidence

`reproductions.patch` applies to `e338d7a` and adds the six tests plus the
allocation example. The separate probe worktree is
`.claude/worktrees/astra-step6-probes`, branch
`solver/astra-step6-review-probes`. The review records are on
`docs/astra-step6-review` in `.claude/worktrees/astra-step6-review`.

Turn artifacts used from run `34524553796`: `10171677831` (Linux), `10171751898`
(Windows), `10171129136` (reference). Step 5b baselines from run `34510476253`:
`10166294613` (Linux), `10166187322` (Windows). River artifacts from the step 6
run: `10171900435` (Linux), `10171902367` (Windows). ZIPs and extracted captures
remain under ignored `target/review-evidence/`. The original comparison artifact
`10171817891` was downloaded but was not used as proof; Astra reran the comparison.
The step 6 artifacts expire upstream on 2026-09-17; local copies remain.

`check_captures.py`, `capture-check-results.json`, `fresh-oracle-results.json`,
and `step5b-acceptance-review.md` record commands, hashes, results, and their limits.
Fable owns implementation fixes. Decision 14 thresholds, runner installation, and
the remaining phase 5/6 product answers remain Caleb's decisions.
