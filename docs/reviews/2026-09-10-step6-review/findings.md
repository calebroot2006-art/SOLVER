# Step 6 checkpoint: needs changes; review unfinished

Saved by Astra on 2026-09-10 after Caleb requested "save and stop".
Reviewed implementation: `e338d7a853f10bf819bf31728e23ac4f1fac802b`, against
`14a02fd`. Main remains at `2f813392d99ce701abd09a6618f1feb6e00e55e2`.
Neither original checkout was edited. Step 6 remains unmerged.

The defects below are sufficient to withhold acceptance. This checkpoint does
not claim that the complete diff, 5d, 5b, or phase 5/6 plans have been accepted.
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
The full diff review remains unfinished, including parts of the test/doc diff.

The worker range check enforces increasing, disjoint node ranges; expansion is
depth first and row offsets are monotone. Mutable slices remain disjoint through
safe Rust slicing. The terminal clears the full opponent vector before every
scatter, uses increasing combo-ID maps, and gathers through the same maps.
No changed terminal summation result or overlapping worker row was demonstrated.
These observations do not override the confirmed memory and lifecycle findings.

| Check | Result |
|---|---|
| Personally queried CI run `34524553796` | All 13 jobs passed at the exact reviewed head |
| `cargo test -p postflop --lib --offline`, untouched probe checkout | 54 passed, exit 0 |
| `cargo test -p postflop --lib astra_review --offline -- --nocapture --test-threads=1` | Six expected contract assertions failed; shell exit 1; defects reproduced |
| `cargo run -p postflop --example astra_memory_probe --profile test --offline` | Construction bound assertion failed, child exit 101 |
| Larger `postflop` and `toygames` suite | Interrupted on Caleb's stop request; 54 library and 7 river tests passed before interruption; streets suite incomplete, toy suites not reached |

The local Rust compiler worked despite the older repository note about Smart
App Control. No security setting was changed. Probe commands ran locally on
Windows, with Cargo 1.98.1, against `e338d7a` plus review-only tests. None of the
six failing probes changes production logic or weakens an existing assertion.

## Saved reproductions and continuation

`reproductions.patch` applies to `e338d7a` and adds the six tests plus the
allocation example. The separate probe worktree is
`.claude/worktrees/astra-step6-probes`, branch
`solver/astra-step6-review-probes`. The review records are on
`docs/astra-step6-review` in `.claude/worktrees/astra-step6-review`.

Downloaded, not yet inspected or rerun: artifacts `10171817891` (turn comparison),
`10171900435` (Linux river), and `10171902367` (Windows river), all from run
`34524553796`. ZIPs remain under the review worktree's ignored
`target/review-evidence/`. Do not claim Astra independently reproduced Fable's
2,501,439-field comparisons. The downloaded artifacts expire upstream on
2026-09-17; the local copies remain.

Next authorized session: finish the diff and numerical comparisons; complete
the 5d review and 5b acceptance; personally assess the helper's saved phase 5/6
findings. Fable owns implementation fixes. Re-review the changed revision and
rerun affected gates before any merge. Decision 14 thresholds, runner install,
and product answers remain Caleb's.
