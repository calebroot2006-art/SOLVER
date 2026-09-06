---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-06
---

# Heads-up river solver

Research: `docs/research/solver-algorithms.md`, `open-source-libraries.md`, and
the phase 2 terminal contract. Caleb authorized full development takeover on
2026-09-06 with a 10% account reserve; stop at 12% remaining and save progress.

## Progress

Phase 2 is accepted at `620ea85`; all five CI jobs passed. The checked tree and
owned river API are integrated. Both solver jobs, both app jobs and the independent
WASM reference passed in run `34053078702` at `2930550`. Every public history,
action, physical combo and contribution matched. Independent scalar calculations
verify root EV, best response, own/opposing reach and every available action EV.

The complete 20,000-iteration captures on both platforms measured below 0.000047%
of pot. The paired default/raw reference check restores every strategy cell and
residual checkpoint exactly. Individual numerical review covers all 588 differing
rows across initial/refined captures, including the 561 still over two percentage
points. Their measured action gaps, reach and convergence effects are preserved in
`tests/reference/river/measured/2930550/`. Rare-path conditional policies can be
materially suboptimal; the review does not turn root convergence into advice there.

The numerical review is `docs/reviews/2026-09-06-astra-phase-3-numerical-review.md`.
The hosted formatting patch is integrated. Final run `34054309357` at `06dd4f4`
passed all six jobs, including mandatory scalar and paired-presentation checks.
Every final capture reproduces the reviewed numerical fields. Phase 3 is accepted;
`final-evidence.json` records the complete hosted gate, artifact hashes and metrics.

All bounded tree/reference helper commits are integrated; their worktrees are
preserved and no helper has active writes. Root owns the core, integration,
numerical review, plans and acceptance. The latest read-only helper independently
checked every explanation category and full-root perturbations.

## Task

Solve a heads-up, zero-rake river subgame from a fixed five-card board, two weighted
ranges, a root pot, an effective stack, and explicit betting settings. Return
per-combo strategies and measured exploitability within that configured game.
The initial public state is out-of-position to act with no outstanding wager.
This phase does not establish full-ring, preflop, or tournament solver coverage.

## Approach

Add an immutable checked betting tree in `tree`. `RiverGame` owns that tree, the
board and ranges, a single shared showdown table, and finite zero-sum terminal
descriptors. A bound `RiverSolver` owns CFR state and reuses traversal scratch.
`RiverStrategy` retains the exact immutable game binding. Updating and evaluating
these objects takes no caller-supplied game, preventing stale callbacks and
cross-game strategy reuse. No public trait method may opt out of legacy audits.

Extract private traversal entry points from the proven CFR and best-response code.
Keep public phase 1 constructors and methods intact, including game mutation
checks and numerical poisoning. Both paths use the same update and evaluation
equations; a fallible private terminal interface supplies terminal values.

Use all 1326 canonical combo slots initially. Remove board-blocked range weights
and condition the product of both ranges on compatible private deals exactly
once at the root. Scaling each range by its maximum preserves the conditioned
game and prevents small input scales from destroying the normalizer. Reject any
positive mass that is lost to numerical underflow; do not silently certify it.

## Steps and ownership

1. Finish phase 2 acceptance before implementing this phase.
2. Specify and build `crates/tree/src/**`, `crates/tree/tests/**`, and its README.
   A bounded executor may own these files in an isolated worktree after the
   betting contract is written. No root manifests, CI, or solver files are theirs.
3. Astra edits `crates/postflop/src/{game,cfr,strategy,best_response,error}.rs` to
   expose private checked traversal entry points and preserve all legacy behavior.
   Add `crates/postflop/src/river/**` for immutable game, solver, strategy,
   normalization, memory accounting, and progress/cancellation.
4. Add independent small-game history/payoff checks, dense-legacy parity on sparse
   river ranges, blocker normalization fixtures, binding failures, zero-sum checks,
   and convergence gates in `crates/postflop/tests/river*.rs` and `tests/` as needed.
5. Capture external reference outputs with pinned provenance. Keep vendor solver
   source and binaries outside the application and repository. Never link an
   application crate against AGPL code. Record the capture route before running it.
6. Run the full Windows/Linux workflow. Save numerical measurements, reference
   comparisons, resource measurements, and the scoped review under `docs/astra/`
   and `docs/reviews/`. Update module READMEs and `CLAUDE-UPDATE.md`.

## Tests and acceptance

- Every existing Kuhn/Leduc update, policy, convergence, mutation, and poisoned
  solver regression remains mandatory. Refactoring cannot lower its tolerance.
- Tree tests enumerate legal histories, call/fold after an all-in, minimum raises,
  raise caps, uncalled wager returns, deterministic action order, and chip totals.
  Invalid sizes, overflow, excessive branching, and exhausted resource limits fail.
- Independent pairwise evaluation checks profile EV and best response on sparse
  physical ranges. Maximization is per own combo, never per opponent hand.
- Small river trees use a separately implemented legacy adapter to compare every
  current policy, regret, cumulative strategy row, average, EV and best response
  through repeated vanilla, CFR+, and DCFR updates.
- Named river fixtures at 20, 100, and 200 big blinds must measure below 0.5% of
  root pot exploitability. Record actual settings and both reference and project
  residuals. Compare identical action histories and physical combos; require
  frequencies within two percentage points or explain each larger difference
  with measured action EVs and convergence evidence. No aggregate-only waiver.
- Check mismatch rejection, invalid rows, all-in-only trees, empty compatible
  mass, skewed weights, cancellation and resume, cap versus target, and finite
  failures. A cancelled solve must report its last complete measured state.
- Account for tree, strategy, regret, averaging, retained game, snapshots, and
  scratch before allocation; refuse a requested solve exceeding its memory limit.
  Report measured runner timing and storage without claiming consumer benchmarks.
- GitHub Actions runs formatting, Clippy, workspace tests, independent Python
  gates, frontend checks, the native Windows build, and its runtime smoke tests.
  Smart App Control remains enabled; no Rust compiler runs locally.

## Risks and decisions

- Product bet menus are not chosen here. Every size, threshold, raise cap, minimum
  bet, and budget is explicit configuration; numerical fixtures are test inputs.
- The zero-sum root convention assigns half the existing pot to each player as
  sunk contribution. Reported net chips include future commitments and returned
  uncalled bets. Document this origin when comparing reference EVs.
- Heads-up effective stacks prevent future side pots. Rake, unequal
  payoff offsets, tournaments, chance streets, and action translation require
  separately reviewed extensions.
- The old `Game` remains supported for custom toy games. Its callback audit path
  must reject river-bound layouts before inspecting legacy-only storage.
- `bestresponse` supplies the public legacy and river metric entry points. Keep
  the traversal in the shared `postflop` core to preserve its private binding and
  terminal boundary; moving it now would introduce a dependency cycle or duplicate
  those contracts. Both APIs therefore execute the same reviewed calculator.
- External reference availability and license separation must be established
  before declaring the phase complete. A missing reference is an open gate.
- Cancellation occurs between full alternating iterations so resume cannot expose
  half an update. Keep terminal errors contextual and poison failed numerical
  updates. Memory limits are enforced before allocations under our control.

## Open questions

No new product decision is required for this explicit-config river backend.
No phase 3 acceptance item remains open. The next roadmap phase extends the
solver to turn and flop; its implementation and plan have not started.
