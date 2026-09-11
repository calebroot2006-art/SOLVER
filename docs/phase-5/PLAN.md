---
project: gto-solver-app
type: plan
status: reviewed-with-dependent-gates
date: 2026-09-10
---

# Phase 5: solved-spot format and library generator

Research: `docs/research/how-to-build-a-solver.md`. The planner's 2026-09-09
proposal is amended after Astra's review at `d9c979d`:
`docs/reviews/2026-09-10-step6-review/phase5-plan-review.md`.
Caleb's product answers remain open. The v1 schema is independently reviewed and
the pure quantization kernel is implemented and verified; see
`docs/astra/phase5-format/PLAN.md`. The bounded builder is in progress separately.

## Progress and dependencies

After review of this amendment, format types, codecs, charts and synthetic tests
can proceed independently. Capture waits for an accepted API that binds a source
policy, game, attempt and completed iteration. The SQLite index waits for
Caleb's dependency decision. The production library waits for his scenario,
coverage, host and coverage target, and accepted flop solves. An earlier turn/
river library is a separate possible milestone; the roadmap's final 25-flop
gate remains required unless Caleb changes it.

This phase owns `crates/spots/**`, its data/schema documents and `spots/**`.
It does not edit `crates/postflop`, `crates/tree`, `crates/cards`, `app/`
or `docs/phase-4/`. Request any missing solver accessor through its owner.
Use existing pinned workspace dependencies; adding SQLite remains a separate
dependency/lockfile review.

## Task and representation

Specify a versioned solved-spot and range-chart format, add a SQLite library
index, and build `spotgen` to generate, export and verify a selected library.
The binary representation is authoritative; JSON exports the same quantized
values. Both must round trip exactly after capture.

A complete flop policy is too large for the first library. The original estimate
is roughly 19 KB per decision node, about 38 GB for two million nodes.
Start-street coverage was estimated at 2–6 MB per spot and about 150 MB for
25 flops. Adding every next-street runout was estimated at 0.5–1.5 GB per spot.
Recompute those figures from the final schema, which includes missing values,
reach, provenance and allocation overhead. They are planning estimates, not
measured output sizes or an approved coverage choice.

Support `StartStreet` and `ActionDepth(u16)` coverage, with explicit uncovered
lookup results. A partial cut does not have a measured exploitability of its own.
Store source-solve accuracy separately from payload quantization error.
Phase 5 writes identity runout mappings; validated suit permutations permit
later storage of accepted phase 4 merging results.

## Steps

**1. Types, resource limits and schema.** Files: `crates/spots/src/format.rs`,
`src/lib.rs`, `docs/phase-5/spot-format.md`.

Define `Spot { header, nodes }`. Header includes format version, spot/scenario
IDs, positions, range labels and canonical ranges, board/start street, chips per
big blind, pot/stack, the full supported tree specification, chip-EV/no-rake
payoff model, units, coverage and `uncovered_policy: ungraded`.

Source metadata includes game and attempt identity, policy iteration, optional
measurement with its own measured iteration and best-response values, root
values, stop reason, target, variant, precision and elapsed time. Provenance
includes solver revision, crate/schema versions, generation time, host and
configuration identity. Missing accuracy is absent, never zero. Derive
staleness from policy and measurement iterations.

Each decision record includes node/compact IDs, history, board/runout, street,
player, ordered actions, contributions, EV scale and mapping. Combo records
include canonical combo ID, integer probabilities, optional integer action EVs
and reach. Define how reach quantization affects applicability.

Validate finite values, action counts, exact integer row sums of 65535,
duplicate histories/IDs/combos, live-combo blockers, units, permutations and
mapping targets. Original ranges may contain board-blocked hands; those are
legal inputs, unlike a blocked combo stored as live. Lookup uses history,
board/runout, player, combo and ordered actions, not node ID alone.
Unknown layout versions are refused.

Define explicit `ResourceLimits` before codecs: aggregate decoded/capture
bytes, encoded bytes, node/combo/action counts, strings and nesting. Account for
vector capacities and metadata with checked arithmetic before allocation.
Apply these limits to binary, its JSON header, JSON import, index rebuild and
CLI input. A count limit alone is not an allocation budget.

Gate: format validation and adversarial allocation tests in `cargo test -p spots`,
schema prose check, and independent schema review.

**2. Bound capture adapter.** File: `src/capture.rs`. Depends on 1 and an
accepted solver binding contract. Public node/range/policy/value accessors supply
the row data; separate strategy, report and input arguments cannot establish
provenance. Request a bounded API addition if binding cannot be verified.
Reject a report, policy or configuration from another game or attempt.

Capture only completed policy iterations and selected coverage. Reserve the
aggregate in-memory Spot budget, including overlap with its source snapshot,
query reports and encoder buffers. A streaming path instead retains one bounded
record at a time; it cannot also claim a complete uncharged nodes Vec.

Quantize probabilities by largest remainder with stable action-order ties,
exact sum 65535 and per-action error at most 1/65535 against the validated
source row. Use signed EV integers in [-32767, 32767]. Store the smallest finite
positive f32 scale at least `max_abs_ev / 32767`; all-zero EVs use scale 1.
Round to nearest with ties away from zero. Verify error against decoding with
the stored scale, at most half that scale; never clamp. Missing EV stays
missing. Nonzero scale or reach cast underflow refuses; expose the measured
reach quantization error separately.

Tests cover a small river-start owned game with 20-combo ranges, decoded rows
against source values, all-zero/missing EVs, endpoints, tied remainders, many
actions, tiny reach and later-runout blockers. Test cancellation before any
measurement, with stale measurement and with fresh measurement, plus mismatched
provenance. Only a fresh source measurement is compared with remeasurement of
that source policy; it does not certify the partial quantized payload.

Gate: main session independently recaptures and checks the same source policy.

**3. Binary codec.** File: `src/binary.rs`. Depends on 1.
Specify little-endian `GTOS` framing byte by byte: version, bounded JSON header,
length-delimited node section and CRC32 trailer. Test CRC32 against
`123456789 -> 0xCBF43926`. Outer limits remain 1 MiB header, four million nodes,
1326 combos per node and 4 GiB file; stricter aggregate ResourceLimits can refuse
earlier. They apply before trusting any declared length.

Seeded generators produce 200 valid spots for exact round trips. At least
1,000 flipped, truncated and inflated-length variants must refuse without panic
or allocation above the budget. Include tiny inputs declaring huge collections,
arithmetic overflow, duplicate IDs and trailing data. Gate: `check`.

**4. JSON import/export.** File: `src/json.rs`. Depends on 1.
Use the existing pinned serde/serde_json, reject unknown fields, and enforce
bytes, nesting, strings, collections and aggregate allocation before a complete
Spot exists. Test oversized/deep JSON and the same 200 exact round trips as
binary. Export is bounded too. Gate: `check`.

**5. Range charts.** Files: `src/chart.rs`, `spots/charts/README.md` and a
clearly labelled illustrative example. Independent of capture/codecs; serialize
shared lib edits. Include version, ID, scenario, position, stack depth, action
labels/ranges and provenance. Parse ranges through `Range::parse`; per-combo
action weights sum to at most 1 + 1e-9. Test range-byte limits, unknown tokens,
duplicates and excessive sums. The example is not a verified opening chart.
Gate: `check`.

**6. Recoverable library index.** File: `src/index.rs`. Depends on 1 and 3,
plus Caleb's approval of pinned rusqlite/bundled SQLite and lockfile review.
Index game/scenario/board/positions, coverage, units, policy iteration, nullable
source accuracy and measured iteration, stop reason, versions, relative path,
size/checksum and generation time. Failed generation attempts have a separate
status/error record; they are not solved entries.

Validate bounded relative paths on lookup, insert, rebuild and verification.
Write a unique temporary file in the destination directory, finish validation
and flushing, publish the complete file, then commit the index transaction.
Specify no-clobber/duplicate-ID behavior and directory durability per supported
OS. Temporary files are never indexed. Rebuild can recover a complete unindexed
file within import budgets.

Tests interrupt before/after publication and index commit; cover missing/corrupt
files, duplicates, path escape and rebuild equivalence. Gate: `check`.

**7. Generator CLI.** Files: `src/bin/spotgen.rs`, `spots/scenarios/README.md`,
`spots/.gitignore`, `crates/spots/README.md`. Depends on 2, 3, 4 and 6.
Provide generate/export/verify commands with bounded scenario/config/list input,
coverage, output directory and revision. Logs record progress with optional
measurement and measured iteration, stop reason, errors, timing and output
bytes. Generation aborts on failure unless continue-on-error is explicitly
selected; that mode records failures separately. Capped/cancelled outputs retain
their real status and cannot enter accepted coverage.

End-to-end tests generate three boards of a small river-start test scenario,
verify the directory and compare JSON rows with the same owned solver API.
Run locally when available and in CI; no local ignore substitutes for evidence.

**8. Production library and coverage gate.** Depends on the previous steps,
accepted flop solves, and product answers below.
Generate the selected 25-flop library, retain logs/index, and verify output
size against the schema accounting. Accepted entries require TargetReached
with a fresh source measurement at the selected target; capped/cancelled entries
remain separately labelled.

The accepted roadmap also requires representative-session coverage. Use a
deterministic trace for Caleb's scenario before the live app exists. Report
decisions graded exactly, approximately or not at all, by cause, plus solve
waits and bot fallbacks. Name the trace and compare its report with Caleb's
coverage target. Successful export alone does not accept useful coverage.

## Verification and open decisions

Run relevant spots tests, workspace Clippy with warnings denied, formatting,
prose checks and CI. Keep generated libraries outside source control unless an
explicit small fixture is selected. Main session reviews the schema, allocation
paths, source binding, publication recovery and exported numerical rows.

Caleb's questions remain:

1. Which scenario (positions, ranges, stack, pot, menus)? Should an earlier
   turn/river library precede the required 25-flop production gate?
2. Start-street coverage or action-depth coverage, with which depth?
3. Generate in CI on the required runner, or locally under WSL2?
4. Approve pinned rusqlite with bundled SQLite and reviewed lockfile changes?
   serde_json is already pinned; it is not a new dependency request.
5. What target accepts the representative-session coverage report?

No product answers have been supplied. Astra's schema/resource/accuracy
requirements above resolve review findings without inventing those answers.
