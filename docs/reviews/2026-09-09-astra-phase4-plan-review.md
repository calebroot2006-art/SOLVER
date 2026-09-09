# Claude's phase 4 plan needs changes

The solver foundation is worth keeping. The active plan has concrete numerical
errors, incomplete acceptance wiring, and resource assumptions that must be fixed
before the affected steps are built. The wider roadmap also needs an earlier test
of whether playing and reviewing hands actually helps Caleb learn.

Reviewed on 2026-09-09: [phase 4 plan](../phase-4/PLAN.md),
[roadmap](../ROADMAP.md), [product brief](../PRODUCT.md), incoming handoff, and
targeted code at `b593f40` on `solver/phase-4`, clean at review start. Step 3 is
unmerged; its memory implementation at `bfb2162` was inspected for context. This
review does not accept that branch or repeat its already recorded repair list.

Astra personally checked the findings and ran small Python reproductions. One
read-only helper reviewed numerical and memory contracts. Source hashes and the
reproduction results are in [evidence.json](../astra/2026-09-09-phase4-plan-review/evidence.json).
Reproduce them with `python docs/astra/2026-09-09-phase4-plan-review/reproduce.py`.
No full solver run or fresh CI acceptance was performed. Prior CI results in the
handoff remain reported history. Claude's plan and implementation were not edited.

## What is good and should stay

* **Correctness is treated as a measurable requirement.** The turn-first sequence,
  known games, river regressions, brute-force all-in checks, and independent
  reference comparisons provide different ways to catch wrong numbers
  (`docs/phase-4/PLAN.md:352-359,498-517`).
* **The reference game must match ours.** Decision 11 prunes the reference tree to
  the approved raise cap instead of accepting a comparison of different games
  (`docs/phase-4/PLAN.md:611-616`). Keep that requirement for the flop.
* **Optimization has explicit safeguards.** Deterministic reduction, refusal before
  oversized allocation, range-symmetry checks, and full unmerged best response are
  sound requirements. Keep them while correcting the implementation instructions.
* **The product has a clear identity and honest advice boundaries.** Play comes
  first, help is requested after the hand, mixed actions are not automatically
  mistakes, and unsupported spots remain approximate or ungraded
  (`docs/PRODUCT.md:37-45`; `docs/ROADMAP.md:22-34,209-227,235-257`).

## Findings that change the implementation plan

### R1. High: compression instructions change the DCFR algorithm

**Location:** `docs/phase-4/PLAN.md:478-484`.

Step 10 stores signed regrets with one scale per node and applies discounting to
that scale alone. DCFR discounts positive and negative regrets differently.
The current implementation does this explicitly in
`crates/postflop/src/cfr.rs:167-174,311-314`. With the approved parameters, iteration
2 maps regrets `[+1,-1]` to approximately `[+0.738796,-0.5]`. One common scale cannot
produce that change in their magnitude ratio. This is an algorithm change, beyond
rounding error. The [DCFR paper](https://arxiv.org/pdf/1809.04040), printed pages
3-4, specifies the two factors.

**Amendment:** Specify the update order: decode, add the regret contribution,
discount by each resulting regret's sign at the existing iteration boundary,
choose the new scale, then encode. A different representation, such as separate
sign scales, needs its own derivation. State exactly when rounding occurs.

**Closure:** Mixed signs, zero crossings, and multi-iteration traces agree with
the f64 algorithm within the stated quantization error. Run known-game and turn
accuracy gates with compressed storage actually selected.

### R2. High: suit merging needs a traversal contract for private hands

**Location:** `docs/phase-4/PLAN.md:457-473`.

The plan multiplies one representative branch's probability by the orbit size
and mentions combo permutation only when answering strategy queries. That is
insufficient for the existing concrete-hand value vectors and masks in
`crates/postflop/src/cfr.rs:357-375`.

For example, on `Ah Kh Qh`, the turn cards `2s,2c,2d` are symmetric for suitably
symmetric ranges. A hero holding `2s3s` blocks representative `2s` but can see
either of the other two cards. Multiplying the representative's zero mask by
three loses two legal outcomes. Range symmetry does not remove a particular
hand's blockers. This counterexample is our inference from the current traversal;
the [Johanson paper](https://johanson.ca/publications/poker/2011-ijcai-abr/2011-ijcai-abr.pdf),
printed pages 4-5, supplies the private-value-vector and canonical-history framework.

**Amendment:** Define representative-to-member permutations, transformed reach
and masks, inverse permutations of returned values, and their composition across
turn and river. Add the affected CFR, best-response, and strategy files to the
step's owned paths; tree construction alone cannot establish this contract.

**Closure:** Small merged and unmerged games agree per combo on chance mass,
terminal values, action EVs, and expanded strategy. Include a hand blocking one
orbit member, weighted asymmetric ranges, and two chance levels. Preserve the
full unmerged exploitability gate as a separate check.

### R3. High: the specified CI machine does not supply the assumed memory

**Location:** `docs/phase-4/PLAN.md:377,436-442,511-512`;
`.github/workflows/ci.yml:309-314`.

The plan assumes standard Linux and Windows runners have four cores and 16 GB,
and places the full flop gate on `ubuntu-latest`. The repository is documented
as private in `CLAUDE.md`. GitHub currently lists standard private-repository
Linux and Windows runners as two CPUs and 8 GB. The public-repository tier has
the assumed 16 GB. See the current
[GitHub runner specification](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Repository visibility and individual historical runner logs were not queried.

**Amendment:** Name an actually available runner with enough RAM for the 12 GiB
solver limit, tool overhead, and measurement. Record physical memory, CPU, image,
and any process/container limit in every benchmark. Fail a prerequisite check
before starting a full solve on an undersized host. Keep ordinary smoke jobs on
standard runners. Choose any paid infrastructure with Caleb before provisioning.

**Closure:** The selected host's recorded resources meet the declared test setup;
the exact gate completes under the memory cap. A later Windows app run must also
measure desktop overhead, which a standalone Linux solver process cannot prove.

### R4. High: the turn gate does not yet join our solve to the reference

**Location:** `docs/phase-4/PLAN.md:379-400`;
`.github/workflows/ci.yml:263-267,305-393`.

Step 5b requires both `turn-solve` and `turn-reference`, but does not assign the
workflow change that connects their outputs. Today the reference job calls
`compare.py` without `--project`; the project job only captures and uploads its
results. The reference-only function explicitly reports `project_capture: absent`
(`tests/reference/turn/compare.py:228-275`). A local fixture also confirmed that
reference-only reporting can return successfully with `reached_target: false`.
That is intentional diagnostic behavior, but it cannot serve as final acceptance.

**Amendment:** Give step 5b ownership of a required comparison job. It consumes
both captures for the same revision and case configuration, enforces convergence,
runs the two-point/per-combo review rule, and retains the report. Missing project
data, skipped captures, mismatched inputs, or stale reviews must prevent acceptance.

**Closure:** Demonstrate that missing data, an above-target solve, a changed case,
and an unexplained frequency difference each fail the final gate. Passing both
capture jobs separately is insufficient.

### R5. Medium: memory reductions depend on unstated storage lifetimes

**Location:** `docs/phase-4/PLAN.md:255-272,402-449,478-486`.

The memory discussion moves from five arrays to three, including one snapshot.
Step 6 explicitly removes only the current-policy array. The existing strategy
stores f64 rows (`crates/postflop/src/strategy.rs:15`), and the step 3 memory
contract covers two snapshots. Steps 7 and 10 do not specify whether snapshots
stay f64, use compact storage, or decode on demand. Full unmerged verification
and overlapping strategy queries also consume memory.

The old approximately 19 GB f32 estimate uses superseded planning anchors. It
does **not** prove the approved gate is impossible after the later menu changes.
Equally, an unmeasured reduction is not evidence that the gate fits.

**Amendment:** Before committing to the full flop sequence, produce a component
table from the exact approved tree and positive range combos: representation,
bytes, lifetime, and allowed overlap for every retained and temporary buffer.
Include snapshots, compression scales, construction, parallel traversal, queries,
and unmerged verification. State how two snapshots become one, if that remains
the design. Estimate the project and reference separately.

**Closure:** Reconcile the table with actual reservations and measured peak
memory, first on the reduced flop and then on the approved gate. If the gate
requires compression earlier, revise dependencies explicitly; do not silently
change Caleb's ranges, menus, or accuracy targets to make a benchmark fit.

### R6. Medium: the f32 failure test cannot exercise its stated failure

**Location:** `docs/phase-4/PLAN.md:420-431`.

`1e-30` does not underflow f32. A Python binary32 conversion returned
`1.0000000031710769e-30`; Rust's
[minimum normal f32 value](https://doc.rust-lang.org/std/primitive.f32.html#associatedconstant.MIN_POSITIVE)
is approximately `1.175e-38`. Further, `finite` accepts finite zero, and
`reach_product` checks multiplication, not a storage cast
(`crates/postflop/src/error.rs:71-118`).

**Amendment:** Define checked storage conversion separately from arithmetic
checks. Distinguish invalid reach loss, f32 cast underflow, overflow/non-finite
values, and expected compression rounding to zero. Define decoding and f64 row
normalization. Difference reports need a stated acceptance/review rule; merely
producing a report is not acceptance.

**Closure:** Exercise positive and negative representable tiny values, actual
cast-underflow values such as `1e-50`, overflow, and normalized decoded strategy
rows. Review frequency and action-EV differences with the measured residual;
do not turn a root residual into a per-decision error guarantee.

### R7. Medium: cancellation and the app boundary are deferred too far

**Location:** `docs/phase-4/PLAN.md:490-495,537-538,582-583`.

The plan says cancellation latency equals one iteration. The current driver
also calls `session.measurement()` after observing cancellation
(`crates/postflop/src/solver.rs:93-107`), so a full best-response calculation can
add delay. The quoted tens of seconds is an estimate, not a measured bound.
Meanwhile, step 11 describes app contracts as open items, even though Decision 5
says turn and flop contracts will be delivered together then.

**Amendment:** Draft the internal request/result and job-lifecycle contract before
the storage refactor: immutable game identity, job and snapshot IDs, memory
reservation, progress freshness, cancellation acknowledgment, resource release,
and late-result rejection. Separate quick acknowledgment from worker completion.
Keep turn internal as Caleb decided. Record iteration, measurement, and cancel
latency separately; specify the app-ready cancellation gate in the responsible
phase if phase 4 retains coarse cancellation.

**Closure:** Cancel during traversal and measurement; start a replacement job;
verify release timing, valid snapshots, and stale-result rejection. Confirm with
Astra that the contract supports browsing a result while solving within the
declared memory budget. These lifecycle checks can live above the numerical core.

### R8. Medium: the plan has conflicting current instructions

**Location:** `docs/phase-4/PLAN.md:39-58,91-103,241-272,548-616`;
`docs/ROADMAP.md:139-156,368-376`.

Progress says Decision 11 is done and reviewed, then labels its work unstarted.
The main steps retain `runout_ranges` after its replacement was recorded.
Answered questions remain under Open questions. Old memory anchors coexist with
new counts; the roadmap still describes the old isomorphism route and a broader
f64 comparison than the phase plan can perform. Step 8 also changes numerical
behavior but is absent from the explicit independent-rerun list near the top.

**Amendment:** Keep one concise current-state table with each step's owner,
dependency, revision, gate, and acceptance state. Move superseded reports into
history. Apply Decisions to the operative steps and reconcile the roadmap now.
Include step 8 in the independent numerical acceptance requirement. Each gate
names its command, artifact, pass condition, and required reviewer.

**Closure:** A fresh executor can identify its base revision, owned paths, exact
inputs, and acceptance command without choosing between contradictory paragraphs.

## Product recommendations for Caleb and Claude

These are proposed sequencing and acceptance improvements, not changes to Caleb's
formats, chart requirement, or release scope.

**Bring forward one complete play-and-learn loop.** The roadmap already permits
engine and app work alongside the solver (`docs/ROADMAP.md:180-181,356-359`), but
the current `app/src/App.tsx` is a placeholder and `crates/engine/src/lib.rs` is
a skeleton. Phase 8 proves that a session can run; phase 9 proves that feedback
is grounded. Neither gate asks whether Caleb understands that feedback or wants
to play another session.

After agreeing the internal contracts, make one limited experience the next
product milestone: play a hand, ask why, inspect the exact decision, review the
session, and retry a related spot. Use accepted solves where applicable and the
existing labeled fallback elsewhere. Propose a short observed session with Caleb:
can he navigate without help, explain the lesson in his own words, and apply it
to a different spot? Record those results alongside correctness checks before
expanding the number of screens and formats.

**Measure how often normal play can actually receive useful advice.** A 25- or
49-flop generated library for one scenario does not establish coverage of random
6-max play at 10-200bb. Matching still requires the right ranges, history, sizes,
and payoff model (`docs/ROADMAP.md:158-166,209-227`). Before scaling the library,
report exact/approximate/ungraded decisions by cause in representative sessions,
plus solve wait time and bot fallback frequency. Pin the workload and propose an
acceptable coverage target for Caleb. Keep a deliberate route into well-covered
practice so the coach is useful while general coverage grows.

**Test the launch-critical preflop assumption earlier.** Our own interactive
charts are required before public launch, yet phase 11 is explicitly the hardest
research problem and comes after tournament implementation
(`docs/ROADMAP.md:287-323`). Move a bounded feasibility investigation earlier:
prove the small reference gate can run, estimate one representative multiway
chart configuration's compute/storage, and state how that evidence scales to
the required formats. This does not authorize paid compute or a smaller product.
It exposes a launch dependency before we invest in all the features waiting on it.

## Recommended revised order

1. Reconcile current state and gates, choose a valid benchmark host, and close the
   existing step 3 findings before accepting its merge and Decision 11 integration.
2. Finish the f64 turn and the mandatory joint reference comparison.
3. Draft the app/job contract and exact storage-lifetime/memory table. Use that
   evidence to confirm the order of compaction, f32, full flop, and compression.
4. Validate the corrected precision path on the turn and reduced flop, then run
   the full approved flop gate. Preserve the stated limit on full-tree f64 evidence.
5. Add suit merging and compression one at a time under the corrected contracts,
   checking each against an accepted baseline before combining them.
6. Run the already permitted engine/app track toward one complete learning loop,
   with coverage and usability evidence. Investigate preflop feasibility before
   scaling tournament and chart-dependent work.

The verdict is **needs changes** for the plan. The findings above specify what
would close that verdict; passing a prose check or the small reproductions does
not establish solver acceptance.
