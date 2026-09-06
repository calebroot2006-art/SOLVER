---
type: review-handoff
from: Claude (Fable)
to: Astra
date: 2026-09-05
status: ready-for-review
---

# Review handoff: research pass and program plan

## Problem addressed and expected behaviour

Caleb asked for research into everything about solvers and a plan for an app that is
both a poker game and a teacher: cash and tournament play at 6, 8, and 9-max; a full
solver; charts; and a coach who flags mistakes and explains on request. The expected
output is a set of sourced research notes, a product brief, and a phased program plan
that a planner can turn into per-phase `PLAN.md` files. No application code exists yet.

## Files to review (SHA-256, no git yet)

| File | SHA-256 |
|---|---|
| `CLAUDE.md` | d4118bb4403eac2a0732f57c3d1d9261cc7e70d2818bb151e554b6aee09e9c6f |
| `docs/PRODUCT.md` | 6d03e1406b77138433a2e66ee422e710b0040b268d178c12809993380596e778 |
| `docs/ROADMAP.md` | 7bfca3c98bd9e4a4365fde93138cbec534e1a13b1343d258a461a31a282f7003 |
| `docs/research/README.md` | dbb155693171685c2473de7ad0c2818c3e0ad654502c84d9b20370476af0651d |
| `docs/research/how-to-build-a-solver.md` | e1686f1a0a2e0f45c8023aa37a15eb45504b9634b97355adb72322565f23b42c |
| `docs/research/solver-algorithms.md` | 4e95f8b2bd083d0ace3967110df549dc1a6543e63924c7a2923b7d84634b264d |
| `docs/research/multiway-solving.md` | fc7b50579c2cef4543a50e7284e043b009576f243645355b9d9fb1f5451aab0e |
| `docs/research/tournaments-and-icm.md` | 134286f3ee0ca8421d1bf0ef2628f9b5054a7996259051a21f198fc0630b6f2f |
| `docs/research/open-source-libraries.md` | 436fb0d6f9776fcb3d30d676320663ebe384b37de4609a5a057b98f18fc8225c |
| `docs/research/trainer-ux-and-coaching.md` | 9d9401705eb7a84693e1c6b789c75a09d2f5d80c9f89fa2f4f12cbed12703094 |
| `docs/research/bots-and-game-engine.md` | 1b9e90358116284135fba1f157c2d350e5eba4c28c56f50d2603941f049ee00c |
| `docs/research/preflop-charts-and-ranges.md` | 4bfd79df97e90a6f6c78870b5677c0345a38b9277ee189bf8ad89722bffdeac6 |
| `docs/research/app-stack-and-coach-integration.md` | 85549f901395be13fc693e2c7785dd6d97bcc54b84c81ccd34b62a7f9f6b6b3b |
| `templates/plan.md` | a461b9693bb8fdf1fd3f17fbc15e82194cfa4a52ebacd4faae57976177d4d17f |
| `.claude/agents/executor.md` | 3bc256891d70734ddf3dbec0bddcfbeda433efbbf03fbf839afb6e8771cd5db8 |
| `.claude/agents/planner.md` | bda5e40b62a9dd05e1805236a60f6a20397a4337d5f2d3ecb08261275d0102b3 |
| `.claude/agents/researcher.md` | 5941c8d5c7703209593afdc556c3df49284492e17d93d687c9d188d105c9178b |

`ASTRA.md` (b9ad39143d254e06202b5066a66f783f4e0d84b394960d74ded9a7a1ebe9dbcf) was
read and not modified. `.claude/skills/` is a verbatim copy of the R&R Automations skill
set (22 folders, 209 files) and is not part of this review.

## Where to look first

1. `docs/ROADMAP.md`: the phase order, the test gates, and the delegation table. This is
   what every later build is checked against.
2. `docs/research/solver-algorithms.md` sections 1, 3, 4, and 8: the numerical claims
   the solver core will be built on. The DCFR parameters were confirmed from the paper's
   PDF; the terminal sweep and best response come from Johanson et al. 2011.
3. `docs/research/app-stack-and-coach-integration.md`, "The coach's brain": the
   grounding design (structured output, cross-check every cited number) is the security
   and correctness boundary for the language-model integration.
4. `docs/research/preflop-charts-and-ranges.md`: the licence finding that vendor charts
   cannot be shipped. It moved the multiway preflop solver up the roadmap.

## Commands run and results

```
python .claude/skills/anti-ai-slop-writing/slopcheck.py CLAUDE.md docs/PRODUCT.md docs/ROADMAP.md docs/research/*.md ASTRO-UPDATE.md
```

Result: 0 banned findings, 28 review-tier findings (long sentences in bullet lists of
build items and in source citations, plus a handful of vague-count words). Review-tier
findings do not fail the linter.

```
find .claude/skills -type f | wc -l     # 209, matches the source repo
grep -n -i "client|automation|two-partner|vendor|pricing" CLAUDE.md .claude/agents/*.md templates/plan.md
```

Result: no leftover consulting-firm wording except the intentional mention of the
automations repo in the skills section.

No code, tests, or solves exist yet, so nothing else was runnable.

## What I verified myself versus what the researchers reported

* The eight research notes were written by `researcher` subagents (Claude Sonnet) and
  edited by me. I checked each note's claims against the sources the researchers quoted
  where the quote was included, and I marked as unverified everything they marked.
* I did not independently open every URL. Sources most worth a second look: the
  PioSOLVER RAM numbers, GTO Wizard's terms Article 7.5, and the robopoker crates.io
  metadata, because the roadmap leans on each.
* Three corrections I made to researcher output: robopoker's evaluator is `deuce`, not
  `kicker`; TexasSolver is AGPL, not MIT; Octopi's coach is "Ask George" and "Odin" is an
  unrelated product. The last one was an error in my own brief to the researcher.

## Known limitations

* The Pluribus figures come from a secondary summary because the Science paper returned
  a 403. The DeepStack and Libratus figures come from arXiv.
* The exact mechanism for substituting ICM payoffs into CFR terminal nodes is inferred
  from vendor descriptions; no code or paper shows it.
* Chart counts for full 6-max, 9-max, and MTT libraries were never pinned down.
* No benchmark exists for a small local model producing trustworthy poker explanations;
  the plan defaults to a templated offline fallback.
* Claude API facts (models, prices, structured-outputs beta header) were fetched on
  2026-09-05 and will drift.

## Open decisions

The seven questions in `docs/ROADMAP.md` under "Decisions Caleb owns". Nothing in
phases 0 and 1 changes with their answers except decision 1 (stack).

## Files still being edited

None. `ASTRO-UPDATE.md` at the repo root is the running handoff note and is rewritten
after every task; it is not part of this review.

## Requests to Astra

1. A review of the research, as your setup status says is pending, with findings filed
   in this folder.
2. Your design pass on the table, coach corner, and study screens can start from
   `docs/PRODUCT.md` and `docs/research/trainer-ux-and-coaching.md` ("What this means
   for our plan") without waiting for code. Phase 7 in the roadmap is blocked on those
   designs.
3. Your view on the coach grounding design before phase 9, since it is the one place a
   network call and a language model touch the app.
