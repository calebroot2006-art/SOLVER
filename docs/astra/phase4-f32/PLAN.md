---
type: plan
status: waiting-for-step6-acceptance
date: 2026-09-10
---

# Phase 4 step 7: checked f32 storage

Base `c2eddfe`, isolated branch `solver/astra-phase4-f32`. Planning is permitted
while final step 6 integration CI runs. Storage implementation begins only after
step 6 acceptance and merge. F64 remains the default; i16 remains unsupported.
Caleb's Decision 14 thresholds and runner installation remain separate decisions.

## Reviewed facts and implementation choices

The flat CFR buffers and PolicySource already support per-node derivation.
Traversal, chance reach, terminals and best response use f64. The current owned
PostflopStrategy stores normalized f64 probabilities and returns f64 borrows.
The projected f32 memory table assumes that the retained snapshot also halves.
Keeping that snapshot in f64 would leave the wide-flop bound around 13.685 GB,
above 12 GiB. Narrowing already normalized probabilities would instead change
the policy after its direct-from-sums accuracy measurement.

Astra selects raw f32 strategy sums for an f32 solver's frozen snapshot.
Live measurement and snapshot queries normalize those same stored values with
the same f64 algorithm. F64 snapshots retain their existing representation.
Imported probabilities remain f64 and are never silently quantized. Their
destination reservation therefore stays f64 even in a game configured for f32.

## Storage and arithmetic contract

Use private typed storage/traversal dispatch, rather than converting both whole
buffers on each iteration. Keep public legacy Cfr and river behavior in f64.
The owned postflop backend supports both widths without exposing an f32 object
through a method that promises an f64 slice. Regrets and sums share one width.

For each update, widen the stored value, calculate the complete regret addition
or average-strategy increment in f64, then checked-store once. Validate finite
arithmetic before applying CFR+'s zero floor; a NaN cannot be hidden by max(0).
For CFR+, floor a completed regret update before converting it to f32, so a
discarded tiny negative regret cannot cause a storage-underflow refusal.
Preserve f64 behavior and alternating player order.

DCFR discount remains after both player walks. Decode the updated stored value,
select its sign-dependent factor, multiply in f64 and checked-store again.
Do not fuse discount with the earlier addition: the second player observes the
first player's update before discount. Preserve whole-sum discounting.

F32 stores refuse nonfinite values, magnitude above f32::MAX, and nonzero values
whose cast is zero or subnormal. Exact zero and normal values are accepted.
Name iteration, node, player, regrets/sums and the failed conversion reason.
Any failure after mutation poisons future iterations, snapshots and diagnostics;
the completed-iteration counter cannot advance. Keep the existing checked
reach/weighted products and both flat-buffer length guards.

Normalize only bounded node/action rows into the existing f64 workspace pool.
All reaches, values, sums used for normalization, reductions, evaluator scratch
and BR calculations remain f64. Chance outcomes retain deterministic order.

## Snapshot and public query contract

An f32 snapshot copies its raw sums, with no full-size f64 intermediate. It owns
its game and lease. Its shared PolicySource decoder is the one used for live
measurement, including uniform fallback for an all-zero action row.

Owned postflop row/node-row queries return checked, leased f64 rows, or use
explicit caller-provided output slices. Invalid/blocked rows retain their named
absence; memory refusal must remain distinguishable. Any optional f64 borrow
accessor names that storage requirement explicitly. Remove the unconditional
whole-buffer f64-borrow promise from the owned postflop API; provide entry count
and bounded iteration/copy access instead. Update all callers and examples.
Legacy callback Strategy and river APIs remain unchanged.

Accumulator diagnostics use a typed read-only storage view or checked leased
decode. No hidden full-policy cache and no uncharged returned Vec. Preserve
CurrentPolicyRow's reservation-before-allocation and payload-before-lease order.
The frozen snapshot does not implement the future global registry or source
binding contract merely by retaining the game.

## Accounting and capture

Retain precision in the owned game. Its actual storage plan prices regrets,
sums and native snapshot at their selected width. Topology, construction,
terminal scratch, ranges and traversal buffers stay at their real widths.
Charge new decoder/container metadata and every construction overlap.
Separate f64 import destination bytes from native snapshot bytes. Recheck
from_rows/from_values/uniform spare capacity and all refusal cleanup paths.

The turn example accepts explicit `--precision f64|f32`, records selected storage
and resolved workers, and exports decoded policy/EV values at completed
iterations. The no-argument behavior remains f64. No target or game input changes.

## Acceptance

1. Preserve f64 captures and fixed-iteration traces, apart from explicitly added
   execution metadata. River accepted fields and turn policies/values remain
   unchanged after dispatch is introduced.
2. Test normal tiny signed values, minimum-normal boundaries, zero/subnormal
   casts, overflow/nonfinite input, CFR+ flooring, mixed-sign DCFR and poisoning.
3. Compare direct f32-sum measurement to frozen-snapshot measurement and queried
   rows. They must describe the same decoded policy. Test all-zero-row fallback.
4. Fixed-iteration f32 policies/values are bit-identical at 1/2/4 workers, including
   nested flop chances. Run capacity/refusal/drop tests and allocation probes.
5. Produce same-revision f64/f32 turn captures on both OSes, both reaching target.
   A true project-to-project baseline mode validates each project's evidence;
   neither input is converted into the external reference schema.
6. Report each row's max/mean absolute frequency and action-EV differences and
   both residuals. Rows above two percentage points require current, evidence-bound
   review reasoning. An unexplained row refuses. Root residuals cannot become
   per-decision accuracy claims. Preserve the ordinary reference gate.

Main reviews exact store points, decoder consistency, imported storage charges,
capture comparisons and error evidence. Required tests, Clippy, formatting and
CI precede acceptance. Step 8's host/flop gate remains separate; a projected
memory reduction alone does not prove host fit or flop correctness.

## Baseline comparison contract

`--baseline` and `--reference` are mutually exclusive. Baseline mode requires
`--project`, `--expected-revision` and a distinct baseline-review record when
any row needs reasoning. Both inputs are project TOML with bounded reads.
Require identical inputs, expected revision and algorithm/run settings apart
from storage width; baseline is f64, candidate is f32. Record capture hashes,
variant/exponents, resolved workers and platform. Independent target stopping
may produce different completed iteration counts.

Extract reusable project validation from the existing gate. Validate both
projects' dimensions, histories, cards, actions, contributions, probabilities,
fold/value origin, root metrics and BR interval, path reach, compatible mass
and decision/conditional EV availability. Preserve the existing f64 allowances:
relative 1e-10 for reach/mass, no absolute floor, and 1e-9 chips for numerical
agreement. F32 storage still exports f64 decoded policy and measured values.
Extract the single-project called-all-in check without changing the external
reference's separate interval/availability semantics.

Match every captured decision/hand directly. Report max/mean absolute frequency
and action-EV differences, null EV comparisons with their availability causes,
and reconstructed reach on both sides. Both solves must reach the same target.
Keep residuals in their named chip/pot-percent units beside iteration counts.

Rows strictly above 0.02 need reasoning bound to both full capture hashes, rule
hash, complete row identity and current evidence. A transplanted sentence or
old oracle array does not count. Recompute relevant action values under each
current policy. Preserve reported-chance dependencies for omitted river betting.
Apply switching-loss and action-gap predicates to both sides; low reach on one
side cannot excuse a materially reached row on the other. Keep the existing
Decision 14 candidates and any C-budget disposition explicit. No root residual
is a local action-error allowance.

First refactor with unchanged external gate output and all 184 tests passing.
Then test both sides' malformed evidence, revision/input/precision mismatches,
unconverged solves, per-row means, missing values, exactly 0.02 versus greater,
missing or stale reasoning, and a downstream mutation below the frequency
report threshold. Main reruns real f64/f32 reports after storage implementation.

## Progress and ownership

The read-only storage review identified the snapshot/import seams above. The
baseline-comparison proposal is incorporated above. No step 7
code is implemented. Assign isolated Rust and Python executors only after step 6
passes its final gate; Astra retains integration, CI and numerical acceptance.
