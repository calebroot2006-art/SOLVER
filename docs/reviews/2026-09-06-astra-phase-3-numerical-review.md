# Phase 3 numerical review

The owned river backend passes the numerical checks on the recorded 20bb, 100bb
and 200bb games. Every material frequency difference has a measured, individual
explanation in the [refinement record](../../tests/reference/river/measured/2930550/README.md).
Phase 3 is accepted at `06dd4f4df09516e4ddfb8c53e0eec67dd4c5997a`; all six jobs
passed in [run 34054309357](https://github.com/calebroot2006-art/SOLVER/actions/runs/34054309357).
[Final evidence](../astra/phase-3/final-evidence.json) records every step, artifact
digest, independent verification report and final measurement.

## Scope and implementation

Astra implemented the shared traversal and owned river core personally, integrated
the bounded tree and reference executors, read their diffs, and reviewed the
measurements. An independent read-only reviewer checked numerical boundaries and
per-combo explanations. All helper source commits are integrated.

`RiverGame` retains a checked tree, fixed five-card board, weighted ranges and
zero-rake terminal payoffs. `RiverSolver` and `RiverStrategy` keep that immutable
identity. Existing callback-based toy games still undergo binding audits. Legacy
callers cannot use the private owned-game path to bypass those checks.

Both original ranges are scaled by their largest live weight after board removal.
The compatible private-deal product is conditioned once at the root. Updates and
queries reject lost positive products, invalid values and over-budget allocations.
A numerical update failure poisons subsequent solver reads and updates. The shared
memory reservation counts concurrent queries and retained strategies; the 16-thread
regression confirms that reservations cannot exceed one shared limit.

Cancellation occurs between full alternating updates and returns a fresh measured
average. Snapshot identity, imported-vector capacities, cancellation/resume,
underflow, legal all-in responses, minimum raises, refunds and invalid trees have
explicit tests. The public `bestresponse` crate delegates to the same private
numerical calculator to preserve binding and avoid a crate dependency cycle.

## Independent evidence

All old Kuhn/Leduc/OpenSpiel checks remain enabled. The sparse river adapter checks
current policy, regrets, cumulative rows, averages, EV and best response through
32 updates for vanilla CFR, CFR+ and DCFR against a separately constructed legacy
game. Direct physical-deal tests independently fix each response before considering
the opponent's private hand. Root blocker mass and scaled tiny ranges have their
own checks.

The scalar Python evaluator expands the fixture rank classes independently, ranks
seven-card hands through five-card subsets, enumerates compatible private deals,
and chooses each response after summing opposing hands. Its information-boundary
regression rejects the result a response would obtain by seeing the opponent's
cards. It evaluates normalized exported policies; the maximum probability adjustment
is recorded, and weighted range syntax is explicitly outside this fixture tool.

Across both platforms, all diagnostic, target and full-budget project captures
match its root EVs, best responses, reach fields and all available action EVs to
1e-9 absolute tolerance. The largest observed action EV difference is below 2e-13
chips. After 20,000 iterations, Windows and Linux frequency differences are at
most 4.4408921e-16. This is measured agreement for these inputs, not a claim of
bitwise reproducibility across every platform or game.

The independent reference builds pinned upstream WASM and engine revisions in an
external temporary checkout. The application does not link vendor code. Exact
source hashes limit optional instrumentation to the presentation wrapper's one
round function and two reach truncation closures. Default/raw 20,000-iteration
captures restore exactly to the original displayed outputs, including every
strategy and residual checkpoint. Historical compiler and dependency resolution
are recorded with the actual artifact hashes.

## Convergence and resources

Each fixture uses root pot 10, chips per big blind 1, 50% bets, 100% pot raises,
minimum bet 1, zero all-in/merge thresholds and a nonbinding raise cap 32. Boards,
ranges and all other settings are stored with every capture. These are fixture
inputs, not product defaults.

| Case | Project initial iterations | Project initial residual (% pot) | Project 20,000 residual (% pot) | Native reference 20,000 residual (% pot) |
| --- | ---: | ---: | ---: | ---: |
| 20bb dry | 300 | 0.000715640365 | 0.0000462145415 | 0.0000405311584 |
| 100bb paired | 500 | 0.000665289524 | 0.00000732371377 | 0.0000214576721 |
| 200bb flush | 1,900 | 0.000962275567 | 0.0000278453555 | 0.000301003456 |

All are below the 0.5% phase gate. Independently evaluating the normalized raw
reference policy gives 0.0000405475821%, 0.0000214181469% and 0.000302099805%.
Those values remain distinct from the native f32 residual. The largest reference
probability normalization change is 6.073211e-8. None is a per-hand error bound.

The full-budget project captures took 212.01/392.08/396.78 seconds on Linux and
149.84/282.81/283.93 seconds on Windows. Their conservative working-set bounds were
2,140,672/3,018,400/3,018,400 bytes; reservations at capture were
1,111,744/1,625,696/1,625,696 bytes. These are implementation accounting and runner
timings. They are not measured process RSS or desktop memory, nor a fair speed
comparison with the reference's sparse f32 implementation.

## Frequency review and practical limits

The complete comparison matches all histories, legal actions, physical hand sets,
contributions and fold winners. Diagnostics establish the root EV origin, and
raised-history fold cells establish the prior-contribution offset. Reference fold
zero becomes `-root_pot/2-actor_contribution` in project net chips.

The review covers 588 distinct rows that exceed two percentage points in either
capture. Twenty-seven now fall within tolerance. The remaining 561 comprise 469
zero-reference-own-reach rows, six zero-opponent-mass rows and 86 reached rows.
Every row retains its own explanation and numeric evidence. The linked README
organizes them by case and action group; its tables do not replace the row record.

At rare histories, small root impact coexists with materially poor conditional
choices. The largest project conditional best-action gain in the reviewed off-path
rows is 161.34444 chips. Its presence is explicit. Root convergence is appropriate
to the configured game and cannot certify coaching advice at that history. A
future consumer must preserve reach, state and coverage context or solve an
appropriate new subgame before drawing conclusions.

For reached rows, the changed-action gaps directly explain approximate mixtures:
up to 0.000128116/0.000326696 chips in dry, 8.84e-10/0 in paired, and
0.000013745/0.000794232 in flush (project/reference). These are observed conditional
incentives, not a new acceptance tolerance, a numerical-error estimate or proof of
exact equality. Reference refinement is not uniformly monotone.

Independent full-root perturbations at the final capture confirmed local effects:

- Dry `check/bet:5/allin:20`, `7c7d`, replace its row by its best action:
  measured gain 1.881582303298046e-7 versus predicted 1.8815823032188514e-7 chips.
- Flush `check/bet:5/raise:25`, `9c9s`, replace its row by the reference mix:
  measured change -2.5527180369522284e-10 versus predicted -2.5527172945994183e-10 chips.

These change one row at a time. Their effects cannot be added to certify a joint
deviation, because an ancestor change alters descendant reach. Average-policy
captures also do not prove that every residual mix comes exclusively from early
averaging; the supported statement is that its measured root influence decreases.

## Hosted acceptance

Run `34053078702` at `2930550` passed both solver jobs, both app jobs and the pinned
WASM job. Rust formatting failed on whitespace in two files; the exact hosted patch
was downloaded, hashed, reviewed and applied. No local Rust compiler ran; Smart App
Control remains enabled. Run `34054309357` at `06dd4f4` passed all six jobs,
including mandatory scalar verification on both platforms and mandatory paired
presentation verification. Final artifact ZIP hashes were independently checked.
All captured case fields match the reviewed `2930550` records exactly when only
elapsed-time fields are omitted. Solver and tree source files are unchanged
between those two commits, and final project capture revisions bind to `06dd4f4`.

The phase is accepted for this documented river scope. The phase 4 turn/flop
extension has not started. The app remains the starter shell, without playable
poker, coach or solver controls. The saved acceptance commit changes documentation
and evidence only; it does not change the verified implementation.
