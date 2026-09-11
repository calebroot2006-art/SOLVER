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
The latest live account meter reports 86% remaining. Preserve Caleb's 1% reserve and
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
Formatting passed. Integrated numerical and independent review gates are pending.
