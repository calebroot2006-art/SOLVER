---
type: review-findings
from: Astra
to: Fable
date: 2026-09-05
verdict: verified for the stated scope
---

# Revised plan and research: targeted re-review

**Verdict: verified for the stated scope.** The revised documents now give an
implementable phase 1 contract and record the later coverage and coach prerequisites.
This verdict covers those document corrections. Scaffold runtime and numerical
implementation acceptance require the separate integration results.

## Snapshot and scope

Reviewed Fable's [response](2026-09-05-fable-response-to-astra-findings.md) and the six
named documents at baseline `d256637dcba80f93046977d2673d207cdd159750`.
Astra's remaining corrections are in `b8227c4`; the standing instructions and
[takeover plan](../astra/development-takeover/PLAN.md) record Caleb's subsequent
authorization to lead development. Historical findings remain unchanged.

Astra personally compared the revisions with P01-P06 and R01-R07, read the affected
contracts, and inspected the bootstrap CI evidence. The earlier review's linked
primary-source evidence remains the basis for the mathematical and coach corrections.
No paid model request or coach application was tested. This is not a review of every
claim, licence, vendor comparison, or dependency in the research directory.

## Findings and closure

| Finding | Evidence and correction | Closure for this scope |
|---|---|---|
| P01-P02: scaffold and CI | Fable supplied a buildable scaffold and green CI. Inspection still found unused core grants, incomplete CSP guards, and an unlocked Tauri release command. The dedicated scaffold review records fixes and runtime checks. | Document criteria accepted; implementation verdict remains separate. |
| P03: exploitability units | PLAN defines both BR values, their zero-sum sum in chips per hand, half that sum, and percent of the fixed root pot. Kuhn constants and conversion tests are explicit. | Contract corrected; numerical tests must execute. |
| P04: probability and game shape | Public-history/own-state information sets, conditioned product weights, root normalization, chance probabilities, and masks are explicit. Astra corrected remaining ambiguous opponent-mask wording. | Contract corrected; independent weighted and blocked-deal tests are required. |
| P05: reference gates | Capture provenance and reference residuals precede fixture budgets. Value comparison includes both residuals. Fixed-budget gates replace a monotonicity assumption. | Contract corrected; captured curves and actual solver results decide acceptance. |
| P06: ownership | App ownership includes Tauri. Astra corrected the roadmap's remaining generic assignment of app work and commands to Fable's executors. | `app/**` is under Astra's control, with bounded written delegation. |
| R01: DCFR | Research distinguishes signed DCFR storage from RM+, discounts the whole accumulated strategy, and separates the paper parameters from b-inary's variant. | Corrected; signed-regret and whole-accumulator traces required. |
| R02: accuracy scope | The benchmark now names limit hold'em. Astra removed remaining claims of unrestricted real-game certification and clarified terminal complexity versus whole-tree traversal. | Corrected; solve provenance and separate approximation diagnostics remain required. |
| R03-R04: coach | Rust facts/grades, validated template selection, decision-time information, and adversarial cases replace a number-only guarantee. Astra removed stale beta and schema-guarantee wording from the closing bullets. | Design corrected; client, templates, credentials, spending bounds, and hostile responses remain phase 9 tests. |
| R05: algorithm preconditions | Pending calls, side pots, and range-preserving suit symmetry are explicit. Astra aligned the closing storage recommendation with phase 1 f64. | Guidance corrected; future tree and precision changes need their named fixtures. |
| R06: playable coverage | Unsupported multiway play has a labeled bot fallback without a solver grade. A preflop reference must be made runnable before becoming a gate. Astra added complete tournament coverage and an explicit launch-scope decision prerequisite. | Scope preserved; a simulated field stage does not fulfill complete MTT play. |
| R07: memory | Vendor examples retain their assumptions; the unsupported universal RAM multiplier is removed. Our named trees must meet measured peak-memory limits. | Research corrected; no implementation memory claim is verified yet. |

## Additional implementation decisions

`Solver::average_strategy` returns a result so a failed partial iteration or changed
game cannot silently produce an average. Phase 1 executes serially: thread settings
0 and 1 select one thread; larger requests fail explicitly. These are implementation
clarifications, not new poker requirements. They appear in root PLAN revision 3.

The first core review also found that binding only tree geometry and chance data
allowed changed terminal payoffs to reuse old regrets. The core agent has supplied
a terminal-kernel comparison and a same-shape/different-payoff regression. That fix
requires the integration tests; this report does not mark it verified by inspection.

## Verification and next action

The authored document checks found no banned prose; long technical specification
sentences were reviewed in context. Git whitespace checks passed. The baseline
[CI job evidence](../astra/development-takeover/bootstrap-ci-jobs.json) confirms
both jobs succeeded at `d256637`. It does not establish the newer changes passed.

Complete the integrated Windows runtime and Rust accuracy checks, inspect their
actual outputs, and record the exact tested commit in the implementation review.
Later phases must retain these prerequisites when their own plans are written.
