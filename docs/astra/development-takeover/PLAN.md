---
project: gto-solver-app
type: plan
status: in-progress
date: 2026-09-05
---

# Development takeover and phase 1 delivery

## Progress

Caleb asked Astra to take full control after Fable reached his session limit, then
requested a team led by Astra, then allowed Astra to choose its size. The main
checkout is on `solver/astra-takeover`, based on `d256637`. Fable's branches and
worktrees are preserved. The core, payoff implementation, independent toy-game
oracles, six captured reference runs, and all 80 checkpoint fixtures are integrated.
Run `34013229097` passes all fixed numerical budgets and the independent weighted
oracle. Strict late Leduc curve comparisons fail and are being checked through
shared-state updates and independent policy evaluation. Astra reproduced 18
updates and 39 metric checkpoints per platform with maximum metric difference
`1.34e-15`. The reviewed replacement retains accuracy budgets and requires these
direct checks in CI. Implementation is not marked complete until those gates and
the runtime checks pass; findings are in the phase 0/1 implementation review.

Fable reported green bootstrap CI. Astra verified run `34009574830` at `d256637`:
both operating-system jobs succeeded. The job evidence is saved in
`bootstrap-ci-jobs.json`. The unused permissions and incomplete regression checks
were corrected and passed Astra's local frontend checks. Hosted Windows built the
release executable; WebDriver session startup failed before the page and security
probes. A direct signed Microsoft driver now reports a missing DevToolsActivePort;
the app agent is checking whether hosted-runner elevation blocks its overrides.
Smart App Control stays on; GitHub Actions is the
approved Rust build environment. No compiler-security settings will be changed.

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

Root `PLAN.md` revision 3 supplies the algorithm and probability rules. Freeze the
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
