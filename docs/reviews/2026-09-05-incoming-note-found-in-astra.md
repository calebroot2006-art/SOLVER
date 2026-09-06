# Update for Astra

Written by Claude on 2026-09-05 after the research and planning pass. Read `CLAUDE.md`
first; this note assumes it. Delete this file when you have read it.

## What was just done

1. **Read `ASTRA.md`** and folded it into the repo: CLAUDE.md now has a "Working with
   Astra" section (roles, `ASTRO-UPDATE.md`, `docs/reviews/`, design comes from Astra);
   the roadmap's delegation table names you for design and for the second review of
   every diff; the design direction and the role split are logged as decisions in
   `docs/PRODUCT.md` and `docs/research/README.md`.
2. **Review handoff filed:** `docs/reviews/2026-09-05-research-and-program-plan.md`,
   with SHA-256 hashes for every file, the commands run and their results, what I
   verified myself versus what the researchers reported, limitations, and three requests.
3. **Repo seeded from R&R Automations.** All 22 skills into `.claude/skills/`, the three
   subagents into `.claude/agents/` (retargeted to this project), `templates/plan.md`,
   and a rewritten `CLAUDE.md`. The subagents section of CLAUDE.md is the part Caleb
   cares most about; do not restructure it.
4. **Product brief:** `docs/PRODUCT.md`. A poker app that is fun and educational at once;
   cash and tournament play at 6, 8, and 9-max; everything a solver does; charts; a coach
   who flags mistakes and explains on request.
5. **Research:** eight `researcher` subagents ran in parallel; findings are in
   `docs/research/`, one file per question, indexed in `docs/research/README.md`. Every
   note ends with "What this means for our plan". Headline conclusions:
   * Heads-up postflop is solved exactly with DCFR(1.5, 0, 2), confirmed from the paper.
   * Full-ring play is graded heads-up on the surviving ranges.
   * Multiway preflop is its own bucketed solver.
   * Bots play from a precomputed spot library plus action translation.
   * The coach may only cite solver numbers.
   * Ship only ranges we solved; vendor charts forbid redistribution.
   * Stack: Rust core, Tauri 2, React and TypeScript, PixiJS table, SQLite, Rive avatar.
6. **Program plan:** `docs/ROADMAP.md`, 12 phases with build lists, test gates, and who
   builds each. Phases 3 to 5 (solver), 6 (engine), and 7 (app) run in parallel once
   phase 2 is done. Phase 7 is blocked on your table and study designs; phase 9 on your
   coach designs.
7. **Corrections made along the way:** robopoker's evaluator is `deuce`, not `kicker`;
   TexasSolver is AGPL, not MIT; Octopi's coach is "Ask George" and "Odin" is unrelated.

## What was verified

Skill file count matches the source (209). CLAUDE.md, agent files, and templates were
grepped for leftover consulting wording. The prose linter reports zero banned findings on
every doc. No code exists yet, so no tests were run.

## What is open

* Caleb's seven decisions in `docs/ROADMAP.md` ("Decisions Caleb owns").
* The repo is not a git repository. Phase 0 initialises it; until then hashes stand in
  for commits in review handoffs.
* `leadgen` and `demo` skills are carried over and unused; delete if Caleb says so.

## Next step

When Caleb answers the stack question, `planner` writes `PLAN.md` for phases 0 and 1,
`executor` bootstraps the repo, and the main session builds the CFR core on Kuhn and
Leduc. Your design pass can start now from `docs/PRODUCT.md` and the trainer research
note; it does not wait on code.
