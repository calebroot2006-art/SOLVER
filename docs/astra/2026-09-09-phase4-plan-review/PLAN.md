---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-09
---

# Claude's phase 4 plan review

Research: `docs/research/solver-algorithms.md`, `docs/research/README.md`,
`docs/research/trainer-ux-and-coaching.md`.

## Progress

Complete. Reviewed the active plan and roadmap at `b593f40`, with eight findings
and three product recommendations saved in
`docs/reviews/2026-09-09-astra-phase4-plan-review.md`. Verdict: needs changes.
Astra inspected the numerical and integration evidence personally; one read-only
helper contributed numerical and memory findings. No implementation was changed.

Python reproductions confirmed the mixed-sign DCFR counterexample, orbit blocker
counts, actual f32 conversions, and reference-only comparison behavior. Results
and seven source hashes are in `evidence.json`; `reproduce.py` reruns the checks.
Black formatting through its Python API, Ruff, strict prose checks, and four local
document links pass. The Black CLI stalled and was stopped; `black.format_str`
verified formatting directly. All seven reviewed source hashes remain unchanged.
No fresh solver or CI acceptance is claimed.

## Task

Critically assess Claude's active turn/flop plan and its place in the product
roadmap. Give Caleb and Claude concrete amendments with closure criteria.

## Approach

Check the written requirements against current code and reproduce small
counterexamples. Distinguish demonstrated plan defects, unresolved feasibility,
and proposals for improving the product. Preserve Claude's active plan.

## Steps

1. Read current decisions, implementation state, and relevant research.
2. Inspect numerical, resource, reference-comparison, and app dependencies.
3. Reproduce bounded findings and check changing external claims at primary sources.
4. Write `docs/reviews/2026-09-09-astra-phase4-plan-review.md`, with priorities,
   evidence, proposed amendments, and checks needed to close each finding.
5. Check prose, links, and the final diff; save `CLAUDE-UPDATE.md`.

## Ownership

Astra writes only this task folder, its new review file, and the outgoing
`CLAUDE-UPDATE.md`. Worktree: `.claude/worktrees/astra-phase4-plan-review`, branch
`docs/astra-phase4-plan-review`. The helper is read-only. Claude's solver branches,
plan, research, and incoming handoff remain unchanged.

## Tests

Run the small review reproductions with Python. Run `slopcheck.py` on authored
Markdown, check local links, and run `git diff --check`. No solver implementation
changes or full solve acceptance are part of this review.

## Risks and edge cases

The active plan contains superseded progress paragraphs and estimates. Do not use
old menu counts to declare the current gate impossible. CI success in incoming
notes is reported history until independently rerun. External runner specifications
are checked against current GitHub documentation.

## Open questions

No question blocks the review. Product proposals are recommendations for Caleb,
not changes to his recorded scope or decisions.

## Decisions

* 2026-09-09: Caleb requested a critical review of Claude's plan. This authorizes
  the review and a durable findings record, without starting solver development.
