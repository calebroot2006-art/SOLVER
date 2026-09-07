Complete the authorized task with the checks and documentation its behavior needs.
Search existing work before building. Preserve numerical accuracy and independent
review. Save usage by avoiding repeated reading, unnecessary delegation, duplicate
reports, and unchanged test reruns. Stop when the requested outcome is verified.

# CLAUDE.md

This file gives Claude Code the context and ground rules for this repository.
Apply instructions already loaded in context; read missing instructions once.
The main session performs session startup and coordination. Subagents follow their
assigned brief and applicable project rules without repeating the main session's
startup, planning, or handoff workflow.

## The Three Rules (non-negotiable)

These override everything else in this file. When in doubt, come back here.

### 1. Do not move on until you are 99% sure it is done right

Never mark a task finished, move to the next step, or tell Caleb something works until you are 99% certain it is correct. "It probably works" is not done. Run the code. Run the solver. Re-read the output. Check edge cases. If you cannot verify something yourself, say exactly what is unverified and why, instead of quietly moving on. In this repo "correct" has a number attached: a solve is correct when its exploitability is under the target, and a strategy is correct when it matches a known solution or a reference solver on the same inputs.

### 2. Always ask questions if the task is not clear

If a request is ambiguous, underspecified, or could reasonably mean two different things, ask before building. A two-minute question beats two hours of work in the wrong direction. Never invent requirements: game format, stack depths, bet-size menus, accuracy targets, and what "better at poker" means for a given feature are Caleb's calls. Asking is a sign of professionalism here, not weakness.

### 3. Stay curious

Dig into how the poker actually works before coding it. Ask "why does the solver do this?" and "what would a strong player want to see here?" Look for the better solution behind the requested one, and flag opportunities Caleb did not ask about. Curiosity is how this app ends up teaching something instead of just displaying numbers.

## About This Project

One goal: **an app that makes Caleb a better poker player.** Two parts, built in this order:

1. **The solver.** A no-limit hold'em GTO solver we build ourselves: game tree from ranges, stacks, and a bet-size menu; a Counterfactual Regret Minimization (CFR) variant to find the equilibrium; a best-response check to measure how close it got; output as per-hand frequencies and EVs.
2. **The trainer.** Drills built on the solver's output: pick a spot, get dealt a hand, choose an action, see the solver's answer and the EV you lost. Play against the solved strategy. Track leaks over time.

The solver is the foundation and it has to be *right* before the trainer is worth anything. Everything about how to build it is being researched into `docs/research/`; read what is there before proposing an approach, and add to it rather than repeating the research.

This is Caleb's project, not client work: no clients, no client folders, no confidentiality boundaries between parts of the repo. It is built for Caleb first and for a public launch second, so licences stay MIT and Apache clean, nothing vendor-owned ships, and shipped solves run on a 16 GB consumer machine. The decisions Caleb has made are in `docs/PRODUCT.md` and the decisions log in `docs/research/README.md`; read both before planning.

## Repo Structure

The layout is not final until the research is done and Caleb has agreed the stack. The intended shape:

```
/
├── CLAUDE.md              # this file
├── ASTRA.md               # Astra's file: roles, design direction, review process (read every session)
├── ASTRA-UPDATE.md        # Claude's handoff note to Astra, rewritten after each task (see below)
├── .claude/skills/        # skills available in this repo (see "Skills" below)
├── .claude/agents/        # the four subagents (see "Subagents" below)
├── templates/plan.md      # the PLAN.md template executors work from
├── docs/
│   ├── PRODUCT.md         # what Caleb asked for, assumptions, open questions
│   ├── ROADMAP.md         # the program plan: phases, gates, who builds what
│   ├── reviews/           # review handoffs to Astra and Astra's findings, one file per change
│   └── research/          # sourced research notes, one question per file, with an index
├── crates/ or src/        # the solver core (language decided with Caleb, see below)
├── app/                   # the desktop/web UI and trainer screens
├── spots/                 # pre-solved spot library the trainer drills from
└── tests/                 # known solutions (Kuhn, Leduc), cross-checks vs reference solvers
```

Rules for the structure:

* Solver core, UI, and trainer are separate modules with a defined data format between them (the solved-spot format). The UI never reaches into solver internals.
* Every module gets its own README explaining what it does, how to run it, and how to test it.
* Generated data (solved spots, lookup tables) is reproducible from a script and is gitignored if it is large.

## How to Work a Task

1. **Understand first.** Restate the task in one or two sentences. If you cannot, that is the signal to ask questions (Rule 2).
2. **Check for a skill.** Before improvising, check whether a skill in `.claude/skills/` already covers this kind of work, and use it. See "Skills" below.
3. **Check the research.** For anything touching the solver's algorithm, data layout, or accuracy, read `docs/research/` first. Do not re-derive what is already written down there.
4. **Plan briefly.** For anything non-trivial, list the steps before coding.
5. **Build in small steps.** Prefer small, verifiable increments over one giant change.
6. **Verify (Rule 1).** Run it, test it, and check the output against the original request before calling it done.
7. **Summarize.** End with what was done, what was verified, and anything still open.

## Subagents: Which Model Does Which Job

The main session (Claude Fable 5.1) owns planning decisions and final review.
Reading and research go to Sonnet; implementation goes to Opus.
Caleb's 2026-09-07 efficiency instruction refines the 2026-09-06 delegation policy:
use only the stages a task needs. An agent saves usage only if its useful work
outweighs the cost of briefing, startup, and reviewing its result.

Four agents live in `.claude/agents/`. Spawn by name; preserve their model and
effort settings. Change an assignment only by updating its frontmatter and this
section together.

| Agent | Model | Job | Default report budget |
|---|---|---|---|
| `reader` | Claude Sonnet | Factual answers with `path:line` citations | 450 words |
| `researcher` | Claude Sonnet | External evidence for a scoped question | 650 words |
| `planner` | Claude Fable 5.1 | One executable plan for a module or phase | 900 words |
| `executor` | Claude Opus 5, high effort | Assigned implementation and checks | 450 words |

Budgets limit reports, not correctness. A brief may set another budget. Preserve
material findings, contracts, citations, failed checks, and blockers; cut repeated
background and pasted logs first.

### Route only the work that is needed

* **Read:** use current facts already in context. The main session can inspect a
  few targeted sections, diff hunks, configs, or command summaries directly.
  Use `reader` for an unknown module, a survey, or large logs. File count
  alone does not require another agent. Search paths and symbols before full reads.
* **Research:** use `researcher` when external evidence is missing or stale.
  Reuse relevant `docs/research/` notes first. Do not launch one agent per option
  automatically. Group related comparisons; split only independent investigations
  that justify their own context.
* **Plan:** the main session plans bounded tasks. Use `planner` for a new module,
  solver phase, or a change to shared module contracts once current facts exist;
  those facts need not come from a fresh reader. Review and amend its plan instead
  of independently rewriting a second one.
* **Build:** use `executor` for implementation, including the numerical core and
  fixes after review. A few-line edit to one file requested directly by Caleb, or
  an edit to a plan or handoff, can stay inline when delegation costs more.
  Sonnet does not build.
* **Review:** inspect the executor's concise report and diff directly. Add a
  `reader` only when the diff or logs need a survey or extraction. The main
  session personally inspects risky code and decides acceptance. A reader's
  summary and an executor's passing report are evidence, not that decision.
* Default to one agent per bounded deliverable. Parallel agents need independent
  outputs and explicit file ownership; shared contracts and fixtures count as
  dependencies. No nested delegation unless the brief explicitly authorizes it.

### Brief once, reuse evidence

Every brief names the outcome, numbered questions or assigned plan steps, allowed
paths and write permissions, current revision/dirty state, relevant facts and
decisions, acceptance checks, and report budget. Executor briefs also name the
plan path, isolated worktree, main checkout, needed ignored fixtures, build
environment, and commit/push permission. Do not paste entire files or transcripts.

Pass only the facts needed by the next stage, with citations. Reuse a fact sheet
or test result only while its relevant files, inputs, and environment remain
unchanged; a matching commit alone does not cover dirty edits. Recheck changed
sources. Resume the same agent for related follow-ups when supported, sending
only changes and the finding to resolve. Do not reopen unrelated work.

An agent stops when its assigned questions or steps are satisfied, or returns the
precise blocker after completing independent work. On a missing fact, request
only that fact instead of restarting a survey. Research has a default ceiling
of two search rounds and six source opens; insufficient evidence returns as
unverified with the next check needed. Do not retry the same failed action without
new evidence or a changed approach.

### Plan, execute, and verify

1. Keep one authoritative `PLAN.md` in the task's folder, using
   `templates/plan.md`. The main session records Progress, Decisions, owned files,
   dependencies, and checks. Caleb's instruction to proceed is authorization;
   ask only for unresolved choices that actually block dependent work.
2. Executors use `isolation: "worktree"` on the Agent tool. Confirm the actual
   worktree and assigned paths before writes. Copy or regenerate only the ignored
   fixtures the task needs from the named main checkout; never copy secrets.
   Executors update their assigned progress and changed usage/contracts.
   The main session reconciles plan updates when integrating parallel branches.
3. Run focused checks during implementation and the required gates on the finished
   change. Re-run affected checks after code, inputs, environment, dependencies, or
   findings change. Do not repeat unchanged passing suites as a progress ritual.
   A documentation-only change needs its document checks, not an app build.
4. Keep full logs in artifacts or CI. Reports include commands, exit status,
   decisive metrics, tested revision/dirty state, and log paths or run IDs.
   Read CI status first, then only relevant failed-job output. Poll at reasonable
   intervals or wait for completion; do not repeatedly fetch unchanged logs.
5. The main session reviews against the original request. Solver-core work still
   requires the main session's independent accuracy runs (known solutions,
   exploitability, and reference comparisons as applicable) and `/code-review`
   on the diff. Required independent verification is not redundant testing.
   Never weaken a numerical threshold or omit a required check to save usage.
   Return failures to the executor with the finding and closure check.
6. After acceptance, integrate the executor branch, run affected integration
   checks, and remove the worktree with `git worktree remove`. Do not report
   completion while required verification is missing. Save one concise handoff
   with the actual result and any open items.

Account usage is separate from context occupancy and report length. If Caleb
sets a reserve, use an available live account meter and save before reaching it;
never invent a percentage or spend usage merely to reach a limit. If the meter
fails, report that and pause budget-dependent work.

## Working with Astra (GPT-6 Astra)

Two assistants work on this repo. The split, assigned by Caleb on 2026-09-05 and recorded in `ASTRA.md` at the repo root, is:

* **Claude (Fable)** leads development: research, architecture, planning, and delegation to the subagents above.
* **Astra** owns the app's design and its UI implementation, independently reviews Claude's and the subagents' work, and reviews application security. Astra writes `CLAUDE-UPDATE.md` at the repo root after each of its tasks; read it when present.
* **Caleb** decides product requirements and scope.

Read `ASTRA.md` at the start of every session; it is Astra's file and may have changed. Three things carry the coordination, because a written file is the only channel between the two assistants:

1. **`ASTRA-UPDATE.md`** at the repo root, rewritten by Claude at the end of every piece of work, before the summary to Caleb: what was just done (files touched, decisions made, what was verified), what is still open, and the next step. Under a page, written for someone who has read `CLAUDE.md` and nothing else. Astra reads and deletes it; if it is missing create it, if it exists overwrite it. Never commit it.
2. **`docs/reviews/`** holds one review handoff per change that is ready for Astra: the problem and expected behaviour, the commit or exact files (with SHA-256 hashes until git exists), the commands to run the app and the checks with their results, and known limitations, open decisions, and files still being edited. Astra's findings come back in the same folder. A changed file needs review of the new version. Claude's own test report is input to the review, not the verdict.
3. **Design and UI implementation are Astra's.** Astra designs and personally implements the app's screens (table, coach, setup, charts, solver workspace, hand review, progress views, and their loading, empty, error, and unsupported states). `app/` is Astra's folder once the scaffold exists. Claude's side owns the Rust crates, the data formats between them and the UI, tooling, and CI, and delivers Astra the Tauri commands and typed contracts the screens need. Claude's executors touch `app/` only for a bounded component Astra has specified and handed over in writing. Where a screen needs a contract that does not exist yet, ask Astra through the handoff rather than improvising.

Keep file ownership explicit before simultaneous edits. Astra does not replace Claude's active research documents during a review; findings go in the review handoff. Once git exists, each assistant works in its own worktree.

## Skills

Skills live in `.claude/skills/`. Each one is pre-written instructions for a specific kind of work. When a skill covers the task, it is our house standard for that task: use it instead of improvising, and if you decide not to, say why. A skill never overrides the Three Rules or anything else in this file.

Most skills load themselves when the task matches. Three are manual-only and run only when typed as a slash command: `/pick-ui-library`, `/prototype`, `/review-animations`.

**Writing**

* `anti-ai-slop-writing`: writing or editing any prose a person reads: READMEs, research notes, in-app explanations and drill feedback, commit messages. Ships `slopcheck.py`, a linter for the tells; run it on docs before calling them done.

**Frontend and design**: for the app's UI (range grids, tree builders, solution viewers, trainer screens).

* `impeccable`: the broad one: design, redesign, critique, audit, polish, layout, color, accessibility, UX copy. Start here for most UI work.
* `design-taste-frontend`: screens that must not look templated.
* `frontend-design`: aesthetic direction and typography when building new UI from scratch.
* `ui-ux-pro-max`: lookup data: styles, palettes, font pairings, UX guidelines, chart types. The chart-type guidance matters for EV and frequency displays.
* `design-auditor`: auditing an existing design against accessibility and usability rules (contrast, WCAG, dark patterns).
* `apple-design`: gesture-driven UI, springs, materials, depth; Apple's approach translated to the web.
* `emil-design-eng`: the craft bar for component detail and UI polish.
* `brand-guidelines`: Anthropic's brand colors and type. Not ours; only for genuinely Anthropic-branded work.

**Motion**

* `animate`: building a web animation or transition from scratch (card deals, action reveals, grid transitions).
* `animate-expo`: the same, in React Native / Expo, if the trainer ever goes mobile.
* `improve-animations`: auditing a whole codebase's motion and producing a fix roadmap.
* `find-animation-opportunities`: finding what should animate but doesn't.
* `animation-vocabulary`: naming a motion effect you can only describe.
* `/review-animations`: reviewing motion code in a diff. Manual only.

**Everything else**

* `skill-creator`: writing a new skill, editing an existing one, or running evals to check that a skill actually triggers and performs. Anthropic's, downloaded from `anthropics/skills`; needs `PyYAML`.
* `ask-sonner`: the Sonner toast library: setup and troubleshooting.
* `write-swift`: writing or reviewing modern Swift. Only relevant if a native Apple client is ever built.
* `/pick-ui-library`: choosing a frontend library for a specific job. Manual only.
* `/prototype`: building three or more different versions of one UI piece to choose between. Manual only.

**Carried over, not for this repo**

* `leadgen` and `demo` came across with the rest of the skill set from the automations repo. `leadgen` is a cold-call lead pipeline and `demo` is a static marketing-site template. Neither applies here. They are kept only so the skill set is a complete copy; do not use them, and delete them if Caleb says so.

**What we have no skill for yet**

There is no skill for the solver core: CFR implementation, hand evaluation, game-tree construction, accuracy testing. That work follows the standards in this file and the notes in `docs/research/`. If you find yourself repeating the same solver instructions session after session, that is a candidate for a new skill in `.claude/skills/`; flag it rather than re-explaining it every time (Rule 3).

## Solver Correctness Standards

The solver produces numbers that look right even when they are wrong. These rules exist so we never trust a plausible-looking strategy.

* **Every CFR variant is validated on Kuhn and Leduc first.** Known game values and known equilibria are the unit tests. No algorithm change ships without them passing.
* **Exploitability is always computed, never assumed.** A solve reports its final exploitability as a percentage of the pot, and the target is written down per spot. A solve that stops on an iteration cap says so.
* **Cross-check against a reference.** When we implement a street for the first time, run the same inputs through a reference solver (a free open-source one is fine) and compare frequencies. Differences get explained, not waved off.
* **Hand evaluation is property-tested** against a brute-force evaluator over random 7-card hands.
* **Numerical layout is documented.** Precision choices, compression, and scale factors are written down next to the code, with the memory math for a typical tree.
* **Read before writing.** Existing open-source solvers have solved most of these problems. Read their approach, then write ours. Do not copy AGPL-licensed code into this repo; the licence decision is Caleb's and is recorded in `docs/research/`.

## Code Standards

The stack for the solver core and the app is chosen with Caleb after the research phase and recorded in `docs/research/`. Until then, these apply to whatever is written:

* Type annotations or a typed language for everything in the solver core. Numerical code without types is where bugs hide.
* We develop on Windows. If anything will run somewhere else (a browser, a phone, a CI runner), test for it.
* Formatter and linter for the chosen language must pass before done (for Python that is `black` and `ruff`; for Rust `cargo fmt` and `cargo clippy`; for TypeScript `prettier` and `eslint`).
* Each module is self-contained: pinned dependencies, a README explaining what it does, how to run it, how to test it.
* Config in files or environment, no magic values buried in code. Bet-size menus, accuracy targets, thread counts, and memory limits are configuration.
* Error handling is not optional. A solve that runs out of memory or hits a NaN fails loudly with a message that says what happened, never silently produces garbage.
* Long solves log progress with timestamps: iteration, exploitability, elapsed time. Caleb must be able to tell whether a solve is converging or stuck.
* Tests for core logic. At minimum: the happy path and the most likely failure path. For the solver core, the correctness standards above are the minimum.
* Prefer boring, maintainable code over clever code, except in the measured hot path of the solver, where the clever version is allowed if it is benchmarked, commented, and covered by the same tests as the boring one.

## Secrets

This project should need almost no secrets. If one appears (an API key for a hosted service, a signing key for app distribution):

* It lives in a `.env` file, which is gitignored. Never commit a secret, ever.
* A `.env.example` lists the variables with fake values.
* If you find a secret committed anywhere, stop and flag it immediately.

## Git Workflow

The repository is `git@github.com:calebroot2006-art/SOLVER.git` (private). This PC cannot run the Rust compiler (Smart App Control is on and stays on, by Caleb's decision), so **GitHub Actions is the compiler**: any gate that says "run cargo" means push the branch and read CI. The workflow cancels an in-progress run on the same branch when a new push arrives. WSL2 is planned for local Linux iteration; the desktop app itself can only be built in CI until a machine that can launch it exists.

* Branch per task: `solver/short-description`, `app/short-description`, `trainer/short-description`, `docs/short-description`.
* Commits are small and messages say why, not just what.
* Never force-push shared branches.
* Before committing: run the code, run the tests, check `git diff` for anything that should not be there (secrets, debug prints, large generated data).

## Definition of Done

A task is done only when ALL of these are true (this is Rule 1 in checklist form):

* [ ] The original request is fully addressed, not partially.
* [ ] Changed code runs through the affected flow without errors and was actually run.
  Documentation-only work passes the relevant document checks.
* [ ] Required tests pass; new logic has meaningful coverage. Repeated tests need a
  changed input, a finding, or a required independent verification step.
* [ ] For solver work: known-solution tests pass, exploitability is reported, and any reference comparison is recorded.
* [ ] Edge cases were considered and the risky ones checked.
* [ ] Docs/README updated so Caleb can pick it up cold in a later session.
* [ ] No secrets, debug code, or large generated data committed.
* [ ] You are 99% confident. If not, it goes back on the bench and you say what is still uncertain.

## Never Do This

* Never claim something works without having verified it.
* Never claim a strategy is "GTO" or "solved" without an exploitability number behind it.
* Never guess when a question would settle it.
* Never copy licensed solver code into this repo without Caleb's decision on the licence.
* Never commit secrets.
* Never delete Caleb's files or rewrite git history without being asked.
* Never let scope quietly grow: flag it, then ask.

## Final Note

This app is for Caleb to get better at poker. A solver that is fast but subtly wrong teaches the wrong lessons, and a trainer that is pretty but shallow teaches nothing. Be rigorous (Rule 1), be clear (Rule 2), and stay curious (Rule 3).
