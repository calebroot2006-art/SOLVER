---
project: gto-solver-app
type: plan
status: reviewed-with-dependent-gates
date: 2026-09-10
---

# Phase 6: the game engine

Research: `docs/research/bots-and-game-engine.md` and
`docs/research/tournaments-and-icm.md`. This amends the 2026-09-09 planner
proposal after Astra's review at `d9c979d`:
`docs/reviews/2026-09-10-step6-review/phase6-plan-review.md`.
The isolated step 8 payout type and explicit ICM stub are implemented and
independently verified at `03518c1`; see `docs/astra/phase6-payoff/PLAN.md`.
The five payoff tests passed locally. No engine or fixture replay is complete.

## Progress and scope

After review of this amendment, state/view/error types, a bounded PHH adapter,
fixture inventory and the ICM stub can proceed. Betting and pots require an
explicit selected rule set; unresolved straddle, dead-blind, rake, forced-posting
and tournament-clock choices cannot become implicit defaults.

Build a no-limit Hold'em hand engine for 2–9 seats, with checked integer chips,
legal actions, forced bets, side pots, showdown, histories and a single-table
tournament. Keep ICM a named unsupported operation. This does not extend the
two-player solver's coverage to multiplayer strategy.

This phase owns `crates/engine/**`, the planned additions in `crates/payoff/**`
and phase 6 documents. It uses cards read-only. It does not edit postflop, tree,
app or phase 4 files. Reuse pinned cards/serde/thiserror/log/toml dependencies;
the rs_poker arena feature remains a separate approval and lockfile review.

## Contract requirements

Chip amounts and counts use checked arithmetic. Reject overflow before changing
state; a conservation sum that wraps is not evidence. Configuration validation
checks structural rules. Stack-dependent hand construction permits legal short
stacks and partial forced postings instead of rejecting an ante above a stack.

Name forced-posting priority and ante eligibility in the rule set. PHH's
`ante_trimming_status` must be represented or explicitly refused. Antes are
accounted for in pot construction and eligibility; they are not silently treated
as ordinary live wagers. Test stacks below ante, below blind, and between blind
and blind-plus-ante, with per-seat awards as well as conserved totals.
The research's blind-first short-big-blind rule cannot be replaced by an
unexplained ante-first default.

The authoritative state/history knows privately dealt cards. Public and
seat-specific views, observer history, serialized events and bot/coach inputs
expose only that observer's cards and cards already revealed. Private deal and
public reveal are separate events. A muck does not reveal a hand. Future board
cards in an imported complete history must not appear early in replay.

Astra's display decision: return an uncalled wager when the betting round
closes and no opponent can match it. Emit a distinct return event before awards;
contestable pots exclude it. Animation may show that recorded return before
showdown. This resolves original question 5.

## Steps

**1. State model and observer boundary.** Files: engine Cargo.toml,
`src/{lib,config,state,view,error}.rs`, README.
Define explicit table/hand rule types, checked stacks/commitments, status,
button, street, public board, action pointer and pots. No blanket Default
selects an unresolved rule. Legal seat counts are 2–9. Preserve existing payoff
trait behavior.

Provide seat/public views without a general observer accessor to private
authoritative history. Serialization and diagnostic formatting used for
observers follow the same boundary. Tests compare views and event bytes before
and after reveal/muck, including two revealed players and one mucked player.
Invalid seats, duplicate cards and configuration amounts return typed errors.
Gate: `check` plus boundary review.

**2. Forced posts and betting rounds.** File: `src/betting.rs`. Depends on 1
and the selected forced-posting, straddle and dead-blind rules.
Implement posting, first-to-act order, legal check/call/fold/bet/raise-to ranges,
minimum full raise, short all-in raises and reopening rights, round closure and
uncontested hands. Track each player's reopening rights; cumulative short raises
need explicit expected-rule tests.

Tests include heads-up button/small blind and action order, ordinary 6–9 seat
hands, partial posts, selected straddle positions/order, dead money and an
uncalled excess. Unsupported/unselected rule configurations refuse by name.
Gate: expected state and legal actions after every transition, not only final
chip totals.

**3. Pots, showdown and awards.** Files: `src/{pots,showdown,testing}.rs`,
`tests/property.rs`. Depends on 2 and explicit rake/ante eligibility rules.
Use eligibility sets excluding folded seats, cards evaluation, tied pots and
the specified odd-chip ordering (first eligible seat left of the button for
the proposed Hold'em rule). Include the return event before awards.

Keep the hand-written three-all-in/different-stack/one-fold case with expected
pots and per-seat awards. Add tied side pots, odd chips, short forced postings,
an uncalled excess and equal-stack simultaneous busts under the selected rules.
Run 10,000 seeded legal random hands across every seat count 2–9 and starting
stacks 1–300 big blinds. Record seed/count, fully allocate every pot and check
per-seat legality plus total chip accounting, including a separate rake sink
if rake is selected. Gate: `check` and independent known-answer review.

**4. Internal history and bounded PHH adapter.** Files: `src/history.rs`,
`src/phh.rs`. Depends on 1; parsing is testable before betting is complete.
Separate authoritative history from observer history. Accepted import subset:
`NT` no-limit Hold'em, known cards, complete integer starting/finishing stacks,
supported forced-bet semantics and complete hands. Refuse other variants,
unknown cards, partial hands and fractional/unknown stacks without fabricating
values. Bound encoded bytes, actions, strings, nesting and aggregate allocation
before full parsing.

Map PHH one-based positional players to engine seats, including heads-up
forced-bet reversal. Map `cbr` to raise-to and `cc` to legal check/call.
Preserve `ante_trimming_status`. Map private dealing and explicit `sm` reveal/
muck events separately. Test legal actions, commitments and views after each
mapped transition, including revealed all-in hands before future board deals.
Malformed/unsupported tokens name the input line. Serialization round trips
are additional checks, not the semantic oracle. Gate: `check`.

Export engine-generated histories to PHH and readable per-action text. Full PHH
export reads authoritative history through a separate audit/export path; it is
never the event stream given to an observer. Observer exports contain only
information visible to that seat at the selected time. Test generated hand
export/import with identical replay and redacted observer exports before reveal,
after reveal and after a muck.

**5. Pinned NLHE fixture replay.** Files: `tests/fixtures/**/*.phh`,
`tests/fixtures/SOURCES.md`, `tests/replay.rs`. Depends on 3 and 4.
The PPC set contains 83 mixed-game histories, of which eleven are NLHE.
Copy only the eleven supported PHH data files from `uoftcprg/phh-dataset`
at `e47fbd5816372360bade4de5d712346fe1bb70f6`, directory
`data/wsop/2023/43/5`:

```
00-02-07.phh  00-08-38.phh  00-15-36.phh  00-18-39.phh
02-51-10.phh  02-53-09.phh  02-54-12.phh  02-56-12.phh
02-57-27.phh  03-00-32.phh  03-02-41.phh
```

Record repository, exact revision, file hashes, data license and attribution
before copying. The saved review's `phh-inventory.json` records the independently
fetched fields/hashes. No upstream Python engine code is copied or run.
The fixtures are five-handed, integer-valued, with known cards and
`ante_trimming_status=false`; their big-blind ante is 1.5 big blinds. Import
that recorded amount rather than assuming one big blind. Two histories contain
four explicit show/muck actions.

Gate: all eleven replay to identical per-seat finishing stacks, with expected
state/action checks and observer-boundary assertions. A mismatch reports seat,
expected/actual stack and last action. These fixtures do not cover heads-up,
6–9 seats, short-stack eligibility, straddles or dead blinds; the separate tests
remain required.

**6. Independent rs_poker oracle.** Files: `tests/oracle.rs`, Cargo.toml
dev-dependency. Depends on 3 and Caleb's arena-feature approval/lockfile review.
Replay 2,000 seeded hands, 2–9 seats, blinds and uniform antes in the documented
common rule subset. Exclude unsupported straddles/dead blinds explicitly.
Match legal actions and finishing stacks. Every divergence needs a named rule,
an independently justified expected result and a test; a comment does not waive
a mismatch. If this oracle is deferred, the complete phase gate stays open.

**7. Single-table tournament.** Files: `src/tournament.rs`, config additions.
Depends on 3, 4, PayoutStructure from 8, and selected clock semantics.
Validate blind levels, run hands,
handle the dead button and eliminated seats, record finishing order and apply
PayoutStructure. Test simultaneous busts with unequal and equal starting stacks,
odd-chip/tied-place rules, payouts summing to the prize pool and schedule advance.
Use an injected clock if minute-based levels are selected; do not depend on test
wall time. Gate: `check`.

**8. ICM stub.** Files: payoff lib and README. Independent of engine steps.
Add validated finite, non-increasing, non-negative PayoutStructure and an ICM
stub using the unchanged Payoff trait. Checked utilities return a named
`ICM not implemented in phase 6` error; unchecked output contains NaN and
never plausible finite advice. ChipEv and its existing behavior remain intact.
Gate: payoff tests, Clippy and review.

**9. Docs and acceptance.** Depends on all required gates.
README and review handoff specify supported rules, excluded/unanswered paths,
chip units, PHH subset/provenance, view/event boundary, exact test revisions and
oracle results. Run engine/payoff tests, workspace Clippy with warnings denied,
formatting, prose checks and CI. Main session independently inspects per-seat
awards, information boundaries and fixture replay. Unresolved oracle or rule
gates keep full phase acceptance open.

## Product decisions still required

| Question | Dependent work |
|---|---|
| Enable rs_poker arena and its reviewed lockfile changes? | Step 6 oracle and full phase acceptance |
| UTG only, button/Mississippi/re-straddles, and action-order rules? | Straddle configuration and step 2 acceptance |
| Dead-blind input only, or new-seat posts/waits enforcement? | Steps 1–3 affected paths |
| Cash rake-free or a selected rake rule? | Pot accounting, conservation and cash acceptance |
| Hand-count or injected minute clock? | Step 7 schedule |
| Short-stack forced-posting priority and ante-eligibility rule set? | Steps 2–3 short-posting acceptance |

State types, bounded imports, fixture inventory and the explicit unsupported ICM
stub do not need those product answers. They must not silently select defaults
for dependent betting, pot or tournament behavior.
