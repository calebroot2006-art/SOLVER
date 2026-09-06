# River refinement and individual frequency review

These files are factual outputs from source `2930550b5535891b423289d103cae5e69c25acd5`
in [CI run 34053078702](https://github.com/calebroot2006-art/SOLVER/actions/runs/34053078702).
Both solver jobs, both app jobs and the independent WASM job passed. That run's
formatting job failed on whitespace only; its verified patch was applied in the
next milestone. This directory records numerical evidence, not final CI acceptance.

`provenance.json` records every job, artifact digest and file hash. Project captures
are unmodified TOML; reference captures are unmodified JSON. The initial target
captures and explicit 20,000-iteration refinements preserve their distinct stopping
rules. The reference has separate default display and raw f32 refinements. No vendor
source or binary is included. Byte counts and source hashes bind the actual outputs.

The `*-verification.json` files evaluate the captured project policies through the
independently written scalar physical-deal evaluator. They check every available
action EV and every own-reach/opponent-mass value as well as root EV and best
response. The paired presentation report restores the original display rules and checks
exact equality across 69 nodes and 603 residual checkpoints. It checks 4,208
strategy cells, 2,334 shared action EV cells, 3,112 shared EV and equity cells,
8,778 reach cells and 7,211 normalized-weight cells. Raw mode does not change the solved policy.

## Every frequency difference

[per-combo-review.json](per-combo-review.json) includes all 588 rows differing by
more than two percentage points in either the initial or refined comparison. Every
row has its own `review_reasoning`, history, physical cards, actions, both policies,
conditional action gaps, own/opponent reach and single-row root effects, before
and after refinement. The initial comparison is bound to `../82f8f4c/`; its
reference uses display-rounded values. The refined reference uses raw f32 values.
All independent calculations normalize exported probability rows and record the
maximum adjustment. They do not infer per-hand error from the root residual.

| River case | Within tolerance after refinement | Zero reference own reach | Zero opposing mass | Reached in both profiles |
| --- | ---: | ---: | ---: | ---: |
| 20bb dry | 0 | 27 | 6 | 6 |
| 100bb paired | 0 | 194 | 0 | 24 |
| 200bb flush | 27 | 248 | 0 | 56 |

For each of the 469 zero-own-reach reference rows, changing that row alone has no
reference root effect. The project keeps tiny positive average reach. Every such
project row's single-row best-action gain decreased after refinement; the largest
remaining gain is 4.44396e-12 chips. Conditional losses still reach 161.34444 chips.
The evidence explains their diminishing influence on the fixed root result. It
neither calls those conditional choices optimal nor makes them suitable coaching
advice if a player reaches that history.

Six dry-board rows have zero compatible opponent mass in the reference. Conditional
EV is undefined there. Their project counterparts have positive mass and recorded
action values, with single-row root best-action gain below 9.60e-13 chips. Undefined
reference EVs remain null.

For the 86 reached rows, the differing actions have directly measured small gaps:

| Case and action group | Rows | Largest project changed-action gap (chips) | Largest reference gap (chips) |
| --- | ---: | ---: | ---: |
| Dry, fold/call after check/bet/all-in | 6 | 0.000128116 | 0.000326696 |
| Paired, call/raise with K-full hands | 24 | 8.84e-10 | 0 |
| Flush, four check/bet/raise histories | 56 | 0.000013745 | 0.000794232 |

These measurements explain differing approximate mixtures. The 24 paired rows are
indifferent in the independently evaluated reference policy; the other rows have
small nonzero incentives. The larger gaps on unused raise actions are preserved in
each row and are not mistaken for the gap between materially differing actions.
Project conditional best-action gains decreased for every reached row. The
reference's f32/display-to-raw comparison is separately labeled: 18 flush rows have
larger measured conditional gains after refinement, so reference improvement is
not asserted to be monotone or attributed entirely to convergence.

A single-row effect changes only one physical combo's information set, holding all
other policy rows fixed. It equals own reach times compatible opponent mass divided
by root deal count, times the conditional gain. A full-root perturbation regression
and independent final-capture perturbations confirm this calculation. Effects are
not additive when multiple rows or their ancestors change. See the scoped
[numerical review](../../../../../docs/reviews/2026-09-06-astra-phase-3-numerical-review.md).
