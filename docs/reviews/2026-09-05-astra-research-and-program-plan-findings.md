---
type: review-findings
from: Astra
to: Fable
date: 2026-09-05
verdict: needs changes
---

# Astra review: solver research and program plan

**Verdict: needs changes.** The research supports starting with toy-game validation and
a two-player solver. The descriptions below would produce incorrect implementation or
overstate what the app can verify. The coach's numeric cross-check is useful, but the
proposed contract does not establish that an explanation is true.

## Snapshot and scope

Reviewed [the research handoff](2026-09-05-research-and-program-plan.md), the current
product decisions and roadmap, and the numerical and coach research in detail.
The two main research hashes still match the handoff:

| File | SHA-256 |
|---|---|
| `docs/research/solver-algorithms.md` | `4e95f8b2bd083d0ace3967110df549dc1a6543e63924c7a2923b7d84634b264d` |
| `docs/research/app-stack-and-coach-integration.md` | `85549f901395be13fc693e2c7785dd6d97bcc54b84c81ccd34b62a7f9f6b6b3b` |

`CLAUDE.md`, `docs/PRODUCT.md`, `docs/ROADMAP.md`, and the research index have changed
since the older research handoff. This review uses their current versions, which match
the newer phase-plan handoff. It does not reopen Caleb's recorded decisions.
The [snapshot](../astra/2026-09-05-plan-and-research-review/snapshot.json) records every
handoff file's current hash. Hash inventory alone does not mean every claim was verified.

Astra performed the numerical and coach review personally. The one helper's contribution
was bounded source checking for the bootstrap review. Official sources linked below
were inspected on 2026-09-05; upstream branch URLs remain mutable. No solver, model
request, paid service, or application was run. Vendor performance, licensing, and all
nine research notes have not received an exhaustive audit. No upstream solver code was
copied into this repository.

## Findings

### R01. High: DCFR is incorrectly described as flooring cumulative regrets

**Location:** `docs/research/solver-algorithms.md:20`, `:37`, `:63`, `:65`;
implementation dependency at `PLAN.md:148`.

**Evidence and impact:** b-inary's code retains signed cumulative regrets and discounts
them according to their sign. `max(regret, 0)` constructs the next strategy; it does not
discard the stored negative regrets. Its power-of-four reset applies to accumulated
strategy weighting, while regret discounting uses a separate iteration expression.
The note's descriptions of regret-matching+ and a general discount-schedule reset
would lead to a different algorithm. [Inspected implementation](https://github.com/b-inary/postflop-solver/blob/main/src/solver.rs).

The average-strategy wording at line 38 also leaves open whether only the newly added
contribution is discounted. Discounting the whole accumulated sum after adding each
iteration gives different results. Our three-iteration example yields first-action
probability `1/14` for the full accumulator and `36/181` when only each new contribution
gets its one-time discount. [Independent calculation](../astra/2026-09-05-plan-and-research-review/checks.py).

**Correction:** Separate RM from RM+, specify the complete accumulator recurrence and
iteration indexing, and distinguish the paper default `(1.5, 0, 2)` from b-inary's
gamma-3 strategy-reset variant. Keep signed regret storage for DCFR. Explain that the
paper's quadratic CFR+ averaging is a variant; OpenSpiel uses linear averaging.
The default parameters and accumulated discounting are supported by the
[DCFR paper](https://arxiv.org/html/1809.04040).

**Closure:** Correct the research before phase 1 consumes it. Add hand-calculated
positive/negative regret and averaging traces to the phase 1 acceptance plan. The root
plan already says to discount strategy sums in place; make the source note agree.

### R02. High: the research and roadmap overstate the game covered by accuracy measurements

**Location:** `docs/research/solver-algorithms.md:22`, `:85`, `:175`;
`docs/ROADMAP.md:18`, `:22`, `:234`; research index, cross-cutting conclusions.

**Evidence and impact:** The cited 76 CPU-day best-response result is for heads-up
**limit** hold'em, not full no-limit hold'em. The `O(n log n)` result concerns terminal
range evaluation; best response still traverses the relevant public tree. These are
not time or complexity guarantees for our planned no-limit game.
[Johanson et al., sections 4 and 5](https://johanson.ca/publications/poker/2011-ijcai-abr/2011-ijcai-abr.pdf).

Own inference from the planned game model: enumerating all cards while limiting bet
sizes measures exploitability inside that betting tree and its supplied ranges. It
does not measure unrestricted no-limit exploitability. Selecting sampled spots also
does not certify a full multiway game's worst-case value.

Global exploitability is not a per-decision error bar. For example, a one-chip error
in a branch reached with probability `0.001` can contribute only `0.001` chips to a
root expectation. A small global number alone cannot justify grading that local
decision to the same tolerance.

**Correction:** Call results approximate equilibria of the specified game, with exact
card enumeration where applicable. Store the tree/action menu, ranges, root history,
rake/payoff model, metric units, residual, and stopping reason. Separate restricted-tree
exploitability, sampled diagnostics, and any proven full-game bound. Define a separate
policy for uncertain or unsupported grades; do not equate a root residual with local
EV certainty. Correct the benchmark's game name and scope.

**Closure:** Revise the research/index and roadmap accuracy gates before treating them
as program-wide promises. Phase 3/5 must carry this provenance, and phase 8/9 must
demonstrate that mismatched or uncertain spots produce an approximate or ungraded state.
The two-player phase 1 metric is addressed separately in P03 of the
[phase-plan review](2026-09-05-astra-phase-0-1-plan-findings.md).

### R03. High: a valid response can pass the proposed coach checks and still teach the wrong action

**Location:** `docs/research/app-stack-and-coach-integration.md:98`, `:103`, `:108`,
`:151`, `:163`; `docs/ROADMAP.md:195`.

**Entry point and preconditions:** A model response contains a free-text explanation
plus valid citation fields. The proposed checker verifies schema and cited numbers.
There is no application implementation yet; this is a demonstrated limitation of that
minimum contract, not an exploited application vulnerability.

**Evidence:** In the synthetic case below both actions have EV `0.4bb`; the solver
mixes bet at `0.62` and check at `0.38`. This response has valid types, a real action,
and numbers matching that action:

```json
{
  "explanation": "Betting is mandatory; checking always loses.",
  "cited_action": "bet",
  "cited_ev": 0.4,
  "cited_frequency": 0.62
}
```

The explanation is false even though the citation fields pass. The
[counterexample check](../astra/2026-09-05-plan-and-research-review/checks.py) reproduces
that minimum validation behavior. Structured output constrains response shape; it does
not supply a proof of the explanation's poker claims.
[Anthropic's structured-output contract](https://platform.claude.com/docs/en/build-with-claude/structured-outputs).

**Correction:** Make the Rust grading and fact builder authoritative. A fact needs a
stable ID linked to its hand, decision, actor, action/size, solve revision, units, and
coverage. Send only information available at the original decision: later revealed
cards and future runouts must not enter decision-quality reasoning. Request approved
fact IDs or explanation-template IDs from the model and render numbers and comparisons
from trusted data. Derive EV loss and grade in code.

Validate semantic claims as well as numeric equality. An arbitrary prose field can
invent blockers, motives, comparisons, or certainty without inventing any numbers.
For v1's strict grounding requirement, render substantive poker advice from validated
facts/templates. Any freer model narration needs a separately reviewed claim-validation
and evaluation design; do not advertise it as guaranteed true. Templates also need
applicability tests. Remove the unconditional claim that they cannot be wrong.

**Closure before phase 9:** The contract and adversarial fixtures cover equal-EV mixes,
swapped actions, percentage/fraction and bb/chip confusion, stale or mismatched solves,
unsupported multiway spots, fabricated blockers, future-card leakage, and prompt
instructions embedded in untrusted hand text. Every rejected, refused, truncated, or
unavailable response uses an applicable deterministic fallback. Extend the 200-hand
gate to these failure cases; a number-only pass is insufficient.

### R04. Medium: coach API and display boundaries need an explicit implementation gate

**Location:** `docs/research/app-stack-and-coach-integration.md:98`, `:154`, `:163`,
`:167`; `docs/ROADMAP.md:195`, `:240`; `docs/PRODUCT.md:68`.

**Evidence and impact:** The research calls structured outputs a beta requiring the
old header. Current official documentation uses `output_config.format` and says the
beta header is no longer required. The launch API route is deliberately undecided,
but phase 9 already introduces the client. The plan needs to prevent that temporary
client from becoming an unreviewed public credential or spending path.
[Current API documentation](https://platform.claude.com/docs/en/build-with-claude/structured-outputs).

**Correction:** Update the request shape and recheck support on the pinned SDK/model
before implementing. Keep Caleb's Sonnet default and budget. A cost forecast needs
measured token counts and cache behavior; the claim of cents per day is not a spending
limit. Bound response size, timeouts, retries, concurrency, and cost. Tie request and
cache keys to the exact decision, solve revision, prompt/schema revision, and model;
discard stale completions when the user changes context or cancels.

Keep credentials and provider calls in the Rust/backend layer, with no project key
embedded in the frontend or a publicly distributed executable. Record what hand data
leaves the machine. Render returned text as text, with no automatic HTML, shell, tool,
filesystem, or link execution. A coach request requires no model tool access to those
capabilities. The public proxy-versus-user-key choice remains Caleb's launch decision;
the phase 9 contract must keep that choice possible.

**Closure:** Record these boundaries in the phase 9 plan and test cancellation, stale
results, rate limits, offline fallback, and hostile response text when the client exists.
The selected launch path needs credential, authorization, and abuse-limit review before
public distribution. No API request, cost benchmark, or credential storage was tested
in this review.

### R05. Medium: three algorithm preconditions are missing from the implementation guidance

**Location:** `docs/research/solver-algorithms.md:107`, `:151`, `:187`;
`PLAN.md:46`, `PLAN.md:273`.

**Evidence and impact:** The all-in instruction would remove a pending call/fold
decision even heads-up. In a multiway hand, two other players with chips can continue
betting into a side pot. Suit canonicalization is exact only for symmetries that preserve
the board, both weighted ranges, and the game rules. The b-inary card code checks range
symmetry before merging suit outcomes. [Card implementation](https://github.com/b-inary/postflop-solver/blob/main/src/card.rs).
PokerKit's official simulation reference provides all-in betting and subsequent action
examples suitable for fixture selection. [Game simulation](https://pokerkit.readthedocs.io/en/stable/simulation.html).

Finally, the research starts with f32 while the root plan uses f64 and calls a later
storage change one line. Changing a shared `Real` alias also changes accumulation and
best-response precision; that does not establish a compression error bound. The plan's
asserted best-response error floor near `1e-15` also lacks a measured bound for the
proposed inputs and accumulation order.

**Correction:** End betting only after all outstanding responses are resolved and no
further betting is possible; retain side-pot decisions. Restrict suit merging to proven
input symmetries. Keep phase 1 f64, distinguish stored values from accumulation/metric
precision, and measure f32 and compressed output against the f64 baseline. Establish
the metric's numerical allowance from reference comparisons and conditioning tests.

**Closure:** Add pending-call, multiway side-pot, asymmetric-suit-range, and precision
comparison fixtures to their respective phase plans. Correct the all-in and precision
guidance now so it is not copied into tree construction. These are future implementation
checks, not claims that those features already fail in code.

### R06. High: later roadmap gates do not yet demonstrate the promised playable coverage

**Location:** `docs/ROADMAP.md:179`, `:212`, `:234`, `:256`;
`docs/PRODUCT.md:20`; research index, cross-cutting conclusions.

**Evidence and impact:** Phase 8 promises full-ring sessions, but the specified solver
is heads-up and three-way postflop is deferred. A library miss or an off-menu translation
can involve more than two players, where the planned live resolver has no applicable
game. Ten thousand completed hands and aggregate frequency checks do not establish
that recommendations used the correct information or matching ranges.

Phase 11 names wasm-postflop's preflop configurations as a reference. The linked
project and its engine document postflop solving with a flop card configuration; no
runnable preflop comparison is supplied. The gate needs a demonstrated preflop oracle.
[wasm-postflop](https://github.com/b-inary/wasm-postflop),
[engine configuration](https://github.com/b-inary/postflop-solver/blob/main/src/card.rs).

The tournament phase implements a single-table MTT stage while complete multi-table
balancing is deferred. Its 25-ticket satellite completion fixture lacks the tournament
field needed to award those tickets; a nine-seat table alone cannot represent that
field. The current plan does not map every approved tournament format to a complete
playable milestone.

**Correction:** Add a coverage table for each play/grade path: supported game, required
range/history inputs, approximation, bot fallback, and whether grading is allowed.
Provide an explicitly labeled legal bot policy for unsupported multiway and translated
spots, and leave their solver grade unavailable unless a validated model applies.
Do not claim a heads-up continuation validates the earlier full-ring decision.

Replace the preflop oracle with a pinned runnable reference or an independent small-game
exact oracle. Label sampled full-game checks as diagnostics. Map complete tournament
play to explicit phases, including field-wide stacks, payout/bounty state, eliminations,
and table balancing where required. Distinguish a training stage from a whole tournament.
Keep Caleb's approved format scope intact; a reduced launch scope would need his decision.

**Closure:** Correct the roadmap's claimed gates now. The coverage table and references
must be settled before phases 8, 10, and 11 execute. This does not require implementing
multiway solving or tournament play during phase 0.

### R07. Medium: vendor RAM examples are supported, but the scaling claim is not established

**Location:** `docs/research/solver-algorithms.md:132`; `docs/ROADMAP.md:126`.

**Evidence:** Pio publishes the cited `1.2`, `1.9`, `5.9`, and `7.8 GB` examples.
Its hardware page supports `64 GB` minimum and `128 GB` recommended for practical
preflop solving. These sources describe Pio's trees and storage, not our implementation.
The examples do not isolate a fixed tree with one added bet size, so they do not
establish that a second size triples RAM.
[Technical details](https://piosolver.com/docs/technical_details/),
[hardware requirements](https://piosolver.com/docs/faq/hardware/).

**Correction and closure:** Keep the figures with their range/stack/sizing assumptions;
remove the universal multiplier. Use Pio as a contextual comparison. Our phase 4 gate
needs measured peak memory for named trees, including working buffers and desktop
overhead, within the 16 GB target. Keep heavy offline preflop generation separate, as
Caleb's current decision log already does. The node counts and our memory formula
remain unverified until implementation.

## Product, design, and ownership follow-up

The confirmed design direction remains a polished poker room with a friendly coach.
I can start the table, coach, and study design from the current brief; this review does
not deliver those screens or clear the phase 7 design gate. I will implement the main UI
personally. The precise scaffold and folder split is in the
[phase-plan review](2026-09-05-astra-phase-0-1-plan-findings.md).

Reconcile stale roadmap text that still assigns the app to executors. Also align the
phase 9 gate with the product's post-hand and session-review entry points; its current
wording permits a during-hand coach button. The research's opening stack recommendation
must mark face/voice as later and text as v1. These are document corrections to existing
decisions, not requests for new product choices.

Retain Caleb's decision to ship our own charts. This review does not certify all vendor
terms, data redistribution rights, robopoker metadata, model pricing, voice licensing,
or the complete dependency list. Those remain source/version-specific checks before
adoption or launch. Do not mark the entire research directory verified from this report.

## Verification and next action

The independent fraction-based Kuhn enumeration, Leduc chance-mass identity, DCFR
averaging counterexample, and synthetic coach example all passed their assertions.
They are review evidence, not tests of an application. Source checking also confirmed
the original paper's default DCFR parameters, the public-tree terminal sweep approach,
and Pio's stated RAM examples within their documented scope.

Fable should revise the numerical research and record the later coverage/coach gates,
then send the changed hashes for re-review. R01 and the phase-plan findings affect the
immediate build brief. R02 through R07 must be recorded as corrections and prerequisites
for the relevant later phases; they do not authorize building those phases now.
Caleb's go-ahead is still required before Git initialization or executors.
