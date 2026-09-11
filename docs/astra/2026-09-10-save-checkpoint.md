# Save checkpoint before clearing the session

Caleb requested: "save everything so I can clear". Development is paused.
The live account meter reported 39% used, 61% remaining at this save.
This stop supersedes the earlier instruction to continue until 1% remained.
Resume only when Caleb requests it.

## Accepted evidence and pending integration

The main checkout `solver/phase-4` is clean at `2f81339`. Fable's original
step 6 worktree `agent-aeabce3b33204de8a` is clean at `e338d7a` and unmerged.
Both are preserved. Five independent reviews are saved and pushed on
`docs/astra-step6-review` at `d9c979d`.

The correction integration worktree is `.claude/worktrees/astra-step6-corrections`,
branch `solver/astra-step6-corrections`, clean and pushed at `c2eddfe`.
Its execution record is `docs/astra/step6-corrections/PLAN.md`.
All seven original Rust findings have reviewed corrections. This includes the
short strategy-sum guard, bounded diagnostic rows, complete construction and
scratch charges, imported-capacity/drop accounting, cancellation observations
and fresh immutable attempt identity. Main ran numerical, capacity, lifecycle,
allocator and cleanup checks. An additional import deallocation-order defect
was independently reproduced and corrected before integration.

Earlier checkpoint `69dc3ff` passed all 13 CI jobs in run `34557551003`.
Final integration run `34560100450` is still running at this save. Eight visible
jobs have passed, four solve jobs are running, and the dependent comparison job
has not appeared yet. No observed failure is waived. Inspect its final state
before accepting or merging Step 6. Do not push another correction revision
merely to update this note and cancel that run.

Main compared newly generated `69dc3ff` captures against `e338d7a`: all 2,501,439
turn fields per OS agree except 56 permitted metadata, memory and timing fields.
Normal and refined river captures preserve every solved field. Full evidence
hashes and compact exact-comparison results are committed beside the correction
plan. Large downloaded captures and full comparisons remain in ignored
`target/correction-evidence` in that worktree; old captures remain under
`astra-step6-review/target/review-evidence`. Do not remove those worktrees.

The final turn gate passes both new captures with zero failures, missing rows or
stale rows, and 817 current independent oracle walks per OS. Maximum ordinary
oracle discrepancy is 6.856737400084967e-13 chips. Called turn all-ins now enumerate
577,025 compatible pairs and 25,389,100 distinct pair-rivers per OS; 5,546 project
EVs differ by at most 1.7053025658242404e-13 chips. Reference values have separate
proved arithmetic/display intervals and availability checks. Wide intervals at
rounded-away reach remain reported. Ordinary river betting omitted from exports
still uses reported chance values; this is not a complete independent turn solve.
All 184 Python tests pass. No numerical threshold or review record was weakened.

The correction checkpoint also includes the independently reviewed phase 5 v1
Spot schema and pure quantization kernel (nine tests), plus the phase 6 validated
payout/ICM refusal stub (five tests). Integrated formatting and workspace Clippy
pass. These are bounded foundations, not completion of phases 5 or 6.

## Saved independent work

| Worktree suffix | Branch | State at save |
| --- | --- | --- |
| astra-phase4-f32 | solver/astra-phase4-f32 | Plan only; six open contract clarifications appended to its PLAN.md. No f32 implementation. |
| astra-phase5-builder | spots/astra-phase5-builder | Executor saving the compiling bounded builder and 24 passing tests as an unaccepted checkpoint. Read builder.md and final branch head. |
| astra-phase6-fixtures | engine/astra-phase6-fixtures | a57142d; eleven pinned PHH data files, full MIT license, inventory and offline verifier. Main's local inventory check passes. |
| astra-phase5-format | spots/astra-phase5-format | e7b5632; schema and quantization integration, already merged into c2eddfe. |
| astra-phase5-quantization | spots/astra-phase5-quantization | 71aec4a; pure kernel, already integrated via c450806. |
| astra-phase6-payoff | engine/astra-phase6-payoff | 47ac9ed; payout validation and explicit ICM refusal, already merged into c2eddfe. |
| astra-turn-gate-corrections | solver/astra-turn-gate-corrections | 942c8a2; all-in correction, integrated at af3b63e. |
| astra-step6-memory | solver/astra-step6-memory | ed1e852; memory and cleanup fixes, already integrated. |
| astra-step6-probes | solver/astra-step6-review-probes | 4dac7af; original reproductions, already in correction ancestry. |

The PHH corpus is pinned to `e47fbd5816372360bade4de5d712346fe1bb70f6` in
uoftcprg/phh-dataset. Its eleven files contain 8,605 bytes, 159 actions, five
players each and four explicit show actions. Complete known cards and integer
stack records pass inventory validation. Engine replay, action legality, pot
awards, independently reproduced finishing stacks and observer privacy remain
unimplemented. The upstream engine was neither copied nor executed.

The Spot builder is private, bounded storage for borrowed typed DTOs, with an
explicit shared live/retained budget and structural validation. It has no codec
or source certification. The executor owns its final save note. Main has not
performed the required full implementation review. Check allocation growth,
overlap and drop ordering, range parsing scratch, duplicate complete keys,
metric operation order, unavailable EVs, and aggregate limits before acceptance.

## Next session

1. Read the root CLAUDE-UPDATE.md and this checkpoint, then inspect branch heads
   and dirty state. Check final CI `34560100450` without rerunning unchanged work.
2. If final CI passes, record Step 6 acceptance and merge the corrected integration
   into solver/phase-4, preserving Fable's original worktree. Push the milestone.
3. Resolve the six f32 plan review items before assigning isolated Rust storage
   and Python baseline executors. Live f32 measurement and native snapshots must
   decode identical raw sums; imports remain lossless f64. Step 7 remains required
   for the wide-flop memory budget. Projection is not the later host/flop gate.
4. Independently review and test the saved Spot builder, then integrate it.
   Schema/kernel alone do not complete phase 5 step 1; codecs and source binding
   still need their own checked implementation.
5. Finish the PHH artifact acceptance note and integrate the data-only fixtures.
   Do not label inventory checks as engine replay.

Caleb still owns Decision 14 thresholds, runner installation, production scenario,
coverage/depth, host/coverage target, rusqlite and arena dependency approvals,
and unresolved engine rules for straddles, dead blinds, rake, clock and short
forced posts/ante eligibility. No default answer was invented. The final phase 5
gate still requires the 25-flop library. The global driver, total application
budget and snapshot registry/pins remain required before phase 4 step 11.

## Local verification notes

Rust 1.98.1 works. An earlier Windows application-control refusal passed on an
ordinary retry of the same binary; no security setting changed. Sandbox Python
temporary-directory creation caused Black to hang; the same formatting check
passed promptly with approved execution outside the sandbox. Create the intended
temporary root before Resolve-Path if overriding TEMP. Preserve test logs and
source hashes already recorded rather than repeating unchanged suites.
