---
type: plan
status: in-progress
date: 2026-09-10
---

# Corrections after Astra's step 6 review

## Progress and authorization

Caleb said "keep going" after the five reviews were saved at `d9c979d`.
Astra is continuing with isolated implementation corrections. Fable's original
checkout remains at `e338d7a` and main at `2f81339`; neither is an edit target.
The latest live account meter reports 80% remaining. Preserve Caleb's 1% reserve and
save verified milestones; do not spend usage merely to reach the reserve.

## Sequence and ownership

1. A bounded executor fixes R1/R2/R4/R7 in `astra-step6-memory`, based on
   `4dac7af`: construction accounting, retained import capacity, boxed scratch,
   and the sums guard. It owns `streets/memory.rs`, `streets/strategy.rs`,
   `river/strategy.rs`, `cfr.rs`, memory examples, README, and affected integration
   tests. It does not edit either solver module or the generic driver.
2. Astra fixes R5/R6 in `astra-step6-corrections`, also based on `4dac7af`:
   cancellation observation, attempt identity and their meaningful unit tests.
   Astra owns the generic driver, street/river solver modules, and lifecycle doc.
3. A separate bounded executor fixes B1/B2/B3 in `astra-turn-gate-corrections`,
   based on `2f81339`, owning the turn comparison/oracle/review tooling and tests.
   The candidate Decision 14 thresholds remain unchanged.
4. Astra inspects and integrates each correction, then closes R3's diagnostic
   allocation ownership after the memory changes are integrated. No concurrent
   changes to the same file. Add the missing called-all-in numerical gate after
   the comparator corrections, using independently checked evaluation.
5. Amend the reviewed lifecycle and phase plans, preserving Caleb's unresolved
   decisions. Continue independent plan work only when its contracts are ready.
6. Run focused probes, then the relevant complete numerical and CI gates.
   Compare captures with the unchanged baseline and inspect the integrated diff.
   Save and push the correction branch and write a precise handoff. Step 6 stays
   unmerged into main until its required corrections and verification are complete.

## Evidence and risks

The durable findings and reproductions are on `docs/astra-step6-review` at
`d9c979d`, under `docs/reviews/2026-09-10-step6-review/`. The base includes six
deliberately failing Rust probes plus an allocation example; turn-gate mutations
are saved in that review. Turn/river solved fields were unchanged at `e338d7a`.
Do not weaken tests or regenerate records to accept changed numerical behavior.

Memory estimates must cover overlapping allocations and owned outputs, including
sparse ranges where large CFR buffers cannot conceal missing terms. Cancellation
must publish only completed iterations and must not start a new measurement after
observation. Successful, failed, and cancelled prior attempts must not be current.
The comparator must validate evidence before classifying differences and must
recompute or completely bind oracle dependencies.

The runner installation and Decision 14/phase 5/6 product choices remain Caleb's.
Local Rust compilation works without changing security settings. Required CI and
independent review still apply; an executor's passing tests are input to Astra's
acceptance, not a substitute for it.

## Correction checkpoint

The first lifecycle correction passes 11 focused Rust tests. The driver polls
after a completed step before scheduling measurement, after an in-flight
measurement, and after the progress callback before publishing its stop reason.
Cancellation wins over target and cap when observed there. A validated street
solver start advances a checked generation; reports carry an immutable attempt
tag, progress includes it, and cancellation, failure, or manual advancement
closes acceptance. Counter exhaustion refuses without wrapping. The global
replacement driver and snapshot registry remain separate step 11 obligations.

Command: `cargo test -p postflop --lib solver:: -- --skip astra_review_scratch
--skip astra_review_import --skip astra_review_current`. The three exclusions are
the open memory probes, owned by the active correction pass; none is waived.
Formatting passed. The later integrated checks below supersede those exclusions.

## Integrated checkpoint

All seven original Rust findings now have corrections. Main inspected and
integrated memory fixes from 1e8e6f4, the river poll adjustment from 5e3165b, and
the import cleanup correction from ed1e852. R3 returns a read-only CurrentPolicyRow
with a lease on both owned solver APIs. Independent review approved the lifecycle
and row ownership changes. Its additional import drop-order finding was reproduced
and fixed: the allocation hook sees all 546,800 input bytes still reserved when
the selected payload starts deallocation after destination refusal.

Main ran the 64 postflop library tests, seven river integration tests, fifteen
street integration tests and five terminal-sweep tests successfully. The workspace
run initially hit Windows application control on one toy executable; the same
unmodified binary and the remaining toy/tree suites passed on a normal targeted
retry, without security changes. Workspace Clippy passed with warnings denied.
The import unit test passed again after its final ownership correction.

Main ran all four allocation modes. Sparse flop peaked at 263,339,447 against
339,122,382 shared plus construction. Dense turn peaked at 2,630,636 against
10,934,691; dense flop refused after 67,480 incremental bytes. Failed import
retained its charge through input cleanup. These are requested allocations, not RSS.

Turn evidence changes from 4aec013 and 54e0a99 are integrated. Main found the tiny
mass bypass in the first correction: a true normalized reach of 0.5 could become
zero under its absolute tolerance. The follow-up removes the absolute floors and
normalizes classification from reconstructed root mass. Main's 166 Python tests
passed. Main's complete Linux and Windows gate reruns also passed, with zero
failures, missing rows or stale rows. Each recomputed 817 C rows; the maximum
oracle discrepancy was 6.856737400084967e-13 chips. Thresholds and committed
records are unchanged. The source-hashed summaries are saved beside this plan.

The amended 5d contract and phase 5/6 plans received independent review. The
contract's global driver and snapshot registry remain required before step 11;
phase 5 retains the final 25-flop gate. Product/dependency answers remain open.
New contract/plans pass the prose checker. Historical phase 4 plan and README
sentence-length flags remain documented source text; authored prose is checked.

Next: finish the independent called-all-in numerical gate, preserving both
project representations (chance plus runouts, or turn showdown). It enumerates
every pair-legal river and checks both captures' parent EVs. Then inspect final
diffs, run CI and compare newly generated captures with the unchanged baseline.
Step 6 is not merged into main, and no f32 implementation is claimed yet.
