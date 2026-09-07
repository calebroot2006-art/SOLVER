---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-07
---

# Claude agent prompt efficiency

## Progress (updated 2026-09-07)

Prompt edits and static review complete. Implementation commit `a071225` was
fast-forwarded into the main checkout's `solver/phase-4` branch. Work was prepared
in `.claude/worktrees/astra-agent-prompt-efficiency`, branch
`docs/agent-prompt-efficiency`, based on `b82ae27`.
The main checkout was clean. Phase 4 has an executor assigned to step 3;
this task owns only the four agent prompts, the related `CLAUDE.md` instructions,
and this task's documentation. No solver or phase 4 plan edits.

The four definitions contain 1,347 words, down from 2,001. Frontmatter settings are
unchanged except for shorter descriptions. Prompt prose and whitespace checks
pass; the scenario review and measurement limits are recorded in
`docs/reviews/2026-09-07-claude-agent-prompt-efficiency.md`. The Claude CLI was
unavailable, so live agent behavior and account savings are unmeasured.
Before integration, all five existing target files still matched `b82ae27`, the
two new document paths were free, and the main checkout was clean. The last live
account check before saving showed 16% remaining, above Caleb's 8% reserve.
The task is complete; actual savings can be assessed during future Claude work.

## Task

Reduce unnecessary usage in Claude's agent prompts and delegation workflow while
preserving correctness checks, file ownership, and the existing model assignments.

## Approach

Replace repeated context gathering, compulsory agent chains, large default reports,
and pasted test logs with scoped briefs, reusable evidence, concise reports, and
explicit completion conditions. Keep independent numerical verification.

## Steps

1. Edit `.claude/agents/{reader,researcher,planner,executor}.md`: define bounded
   inputs, reports, follow-ups, and completion conditions for each role.
2. Align `CLAUDE.md`: remove conflicting cost and routing instructions, keep the
   model split, and make review and tests proportional to the affected behavior.
3. Check prompt loading, unchanged model/tool permissions, writing quality, and
   realistic routing and failure scenarios. Record prompt-size changes separately
   from unmeasured runtime usage savings.
4. Inspect the diff, recheck source files for concurrent edits, deliver the verified
   changes to the main checkout, and update `CLAUDE-UPDATE.md`.

## Tests

Run the repository prose checker on edited prose. Parse frontmatter and compare
model, effort, and tool settings to the base revision; use `claude agents` if the
installed CLI supports listing without a model call. Run `git diff --check`.
Review small fixes, large surveys, missing facts, failed numerical gates, stale
evidence, and exhausted research. No app build is needed for prompt-only edits.

## Risks and edge cases

- Short reports must still expose failures, assumptions, and unverified gates.
- Existing public APIs need citations; proposed new files need not already exist.
- Reuse applies only while cited files and relevant inputs remain unchanged.
- A word limit must not hide a blocker or weaken an accuracy threshold.
- The live Codex account meter showed 17% remaining before editing. Caleb's 8%
  reserve supersedes older usage limits for this task; check between work stages.

## Open questions

None.

## Decisions

- 2026-09-07: Caleb authorized fixing Claude's agent prompts to reduce wasted usage.
  Preserve agent names, models, effort, and tool permissions; optimize instructions.
- 2026-09-07: Apply the completed prompt edits to Claude's main checkout after
  checking for concurrent changes. Keep the active solver work and `ASTRA.md` intact.
- Incoming `ASTRA-UPDATE.md` describes phase 4 steps 1, 2, and 5a as merged, step 3
  as next, and the one-raise Decision 10 as binding. This task does not alter those
  decisions or claim a review of the solver changes.
