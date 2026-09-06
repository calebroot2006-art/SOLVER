---
project: gto-solver-app
type: plan
status: in-progress
date: 2026-09-06
---

# Heads-up river solver

Research: `docs/research/solver-algorithms.md`, `open-source-libraries.md`, and
the phase 2 terminal contract. Caleb authorized full development takeover on
2026-09-06 with a 10% account reserve; stop at 12% remaining and save progress.

## Progress

Phase 2 is accepted at `620ea85`; all five CI jobs passed. The checked tree and
owned river API are integrated. Run `34051757916` at `82f8f4c` passed all workspace
tests and existing independent numerical gates on both platforms. The first
named river captures measured below 0.001% of pot. The real pinned WASM capture
ran successfully, and every public history, action and contribution matched.
Frequency review remains open: 579 rows differ by more than two percentage points.
Some reference EVs are hidden by its display cutoff. A separately labeled full
20,000-iteration capture and presentation-only instrumentation address this gap.
One tree test's Clippy slice-size calculation and the example's hosted formatting
are being corrected. Phase 3 is not yet accepted.

The tree executor's commit is integrated; root now owns all tree and numerical
files. The reference executor retains only its existing capture scripts, tests
and README in `.claude/worktrees/astra-phase3-reference`. Root owns the comparator,
project capture, measured fixtures, manifests, CI and documentation. The expanded
reference boundary is recorded in `reference-contract.md` before instrumentation.

A read-only helper critiques that binding change. A separate read-only helper
checks external reference capture and exact input/output conventions. Astra owns
the plan, core numerical implementation, integration, review, and acceptance.

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
Per-combo frequency explanations and the final complete hosted run remain open.
