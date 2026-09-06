---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-05
---

# Development takeover and phase 1 delivery

## Progress

**Complete.** Caleb asked Astra to finish this milestone and hand development back
to Claude. The takeover branch `solver/astra-takeover` contains the integrated
phase 0/1 implementation. All five jobs pass at
`0d4f338d5e1b62bd8af25ce3580a6f7c3c252a26` in
[run 34015308353](https://github.com/calebroot2006-art/SOLVER/actions/runs/34015308353).
The final save adds documentation and preserved evidence to that tested code.

Both solver jobs pass the core and independent history tests, accuracy budgets,
11 verifier mutation tests, 18 shared-state replays, and 54 policy snapshots.
Six original OpenSpiel captures and all 80 checkpoints remain. The independent
update/policy checks resolve the late Leduc rounding-sensitive trajectory finding;
the [implementation review](../../reviews/2026-09-05-astra-phase-0-1-implementation.md)
records the diagnosis, tolerances, numerical limits, and closure evidence.

The Windows release renders correctly, denies an ungranted core command, and blocks
the external request with an enforced CSP event. The standard-user launcher passes
cleanup. Astra reviewed the final screenshot and independently hashed the downloaded
artifact. [Final CI evidence](final-ci-evidence.json) preserves the jobs and hashes;
[numerical evidence](final-numerical-evidence.json) preserves the detailed reference
comparison. Both frontend jobs pass; all lockfiles stay unchanged.

The three agents completed their assignments and have no active writes. Fable's
original worktrees and the agent worktrees are preserved with local diagnostics.
Their implementation is already integrated; do not reapply it. Claude resumes
development leadership from `CLAUDE-UPDATE.md`, with Astra's standing ownership
unchanged. Phase 2 has not begun. Smart App Control stays enabled; GitHub Actions
remains the approved Rust build environment.

## Task

Complete the remaining bootstrap corrections and deliver the CFR core, Kuhn/Leduc
games, independent oracles, reference fixtures, and measured phase 1 validation.
Keep the existing product scope and researched algorithms. Do not present the scaffold
as the completed poker product or begin unrelated later-phase features.

## Delegation and file ownership

| Work | Owner and worktree | Allowed writes |
|---|---|---|
| Integration, final review, CI evidence, shared docs and workspace lockfiles | Astra, main checkout | Root files, config, shared docs; reviewed integration changes |
| Scaffold security corrections | `takeover_bootstrap_check`, `.claude/worktrees/astra-scaffold` | `app/**`, `.github/workflows/ci.yml`, `docs/reviews/2026-09-05-astra-scaffold-hardening.md` |
| CFR and payoff implementation | CFR agent, `.claude/worktrees/phase-1-cfr` | `crates/payoff/**`, `crates/postflop/**` |
| Toy games, independent oracle, OpenSpiel capture, integration tests | Oracle agent, `.claude/worktrees/astra-oracles` | `tests/**`, its ignored `.venv/**` |

Agents do not edit each other's files or root `PLAN.md`, change global security
settings, push branches, or merge changes. They may commit their owned files after
checks. Astra inspects each diff before integrating and controls remote CI runs.
No nested delegation. Agents report exact commands, observed results, and limitations.

## Shared numerical contract

Root `PLAN.md` revision 4 supplies the algorithm and probability rules. Freeze the
public Rust API between the CFR and oracle agents before writing integration tests:

- Use its `Game`, `NodeKind`, `NodeId`, `Real`, `Variant`, and `Exploitability` shapes.
- `Cfr::new(game, variant)` validates the game and returns a result. Expose iteration,
  one iteration, and the average strategy through the planned `Solver` trait.
  `average_strategy` returns a result so a mismatched game or poisoned iteration fails.
- Strategies use state-major flattened rows per public node, with actions contiguous
  for each private state. Provide checked construction from rows, a uniform strategy,
  and read access to a node's row. No public unchecked mutation is required.
- Expected value, best response, and exploitability report errors for invalid or
  nonfinite inputs. Use chips per hand and the explicit percent-of-root-pot conversion.
- CFR and oracle agents send each other their agreed exact signatures before depending
  on them. Changes to this contract are communicated to Astra and the other agent.
- Phase 1 is serial. Thread settings 0 (automatic) and 1 select one thread; reject
  larger values until the implementation supports them.

## Incoming handoff preserved

Astra read Fable's late-evening phase 0 update, SHA-256
`e17c00baee33c7165f14299b475870824e7bd403f4170d7bf6c6f324d6d21a86`.
It reported the bootstrap CI runs, requested scaffold transfer and a targeted document
re-review, and named `.claude/worktrees/phase-1-cfr` as the pending numerical branch.
Inspection found that branch still at the bootstrap baseline with no numerical code.
The older executor worktree is at `763faba`, not the main checkout's `d256637`.
All existing worktrees are preserved during integration. A future Fable session should
use this task's final handoff rather than restart the completed bootstrap.

## Steps and acceptance

1. Review Fable's revisions and correct remaining stale or contradictory guidance.
2. Remove unused scaffold permissions, explicitly select capabilities, strengthen
   regression checks, and correct future command-registration instructions.
3. Capture pinned OpenSpiel reference outputs before asserting numerical fixture
   budgets. Inspect game parameters, averaging/update semantics, and reference residuals.
4. Implement phase 1 in isolated branches and compare against independent references.
5. Review and integrate the changes, generate exact lockfiles, and run meaningful
   formatting, lint, numerical, frontend, and Windows native build checks.
6. Record the exact CI commit and results. Keep any unperformed desktop runtime check
   explicitly open. Update the permanent role note, project progress, review findings,
   and `CLAUDE-UPDATE.md` for a future Fable session.

## Quality gates

Verify signed DCFR regrets and averaging traces, chance masks and root normalization,
uniform Kuhn/Leduc constants, known Kuhn equilibrium, reference curves, fixed-budget
convergence, and nonfinite/invalid input failures. Test code must independently exercise
the behavior rather than duplicate the implementation. No tolerance is relaxed to hide
a mismatch. The senior review includes the merged result, not only agent reports.
