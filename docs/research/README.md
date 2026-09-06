# Research notes

Everything about how to build the solver, the game, and the coach lives here. Read
before proposing an approach; add to these files rather than repeating the research.

Each note has frontmatter with `type`, `status` (draft, reviewed, decided) and `date`.
Findings carry their source. Anything unverified is marked as such. Each note ends with
a "What this means for our plan" section; `../ROADMAP.md` is built from those.

## Index

| File | Question it answers | Status |
|---|---|---|
| `how-to-build-a-solver.md` | Overview: what a solver is, the CFR family, components, existing projects, first-pass architecture | draft |
| `solver-algorithms.md` | Exact DCFR rules (confirmed from the paper), best response, the Johanson terminal sweep, isomorphism, memory, PioSOLVER RAM numbers, pitfalls | draft |
| `multiway-solving.md` | 3+ player and full-ring (6/8/9-max) solving, preflop and postflop, bunching, what is feasible | draft |
| `tournaments-and-icm.md` | ICM models, tournament payoffs in a solver, bounties, structures a game engine needs | draft |
| `open-source-libraries.md` | What to depend on, what to read only, licences; robopoker crate map | draft |
| `trainer-ux-and-coaching.md` | How trainers grade and teach, AI explanations, what makes it fun, the coach | draft |
| `bots-and-game-engine.md` | Real-time bot opponents at full-ring tables, live grading, game engine oracles | draft |
| `preflop-charts-and-ranges.md` | Sources, licences, and formats of preflop ranges; why we solve our own | draft |
| `app-stack-and-coach-integration.md` | Desktop shell, table rendering, data storage, Claude-based coach, avatar, voice, packaging | draft |

## Cross-cutting conclusions

* Heads-up postflop is solved with DCFR, enumerating cards exactly within a bet menu,
  fixed ranges, and a root history; its exploitability is measured within that game and
  stored with its provenance. Full-ring play is graded heads-up on the surviving ranges
  where the inputs match; 3+ player postflop is ungraded until a validated model
  exists; multiway preflop is its own bucketed solver.
* Bots play from a precomputed spot library plus action translation plus a bounded live
  re-solve, with a labelled fallback policy where no solver applies. Nobody solves
  full-ring live, including the commercial products.
* The coach's poker claims are rendered from facts and templates computed and validated
  in Rust. A language model selects and orders them; it never decides what is correct,
  and matching numbers alone is not a correctness check.
* Ship only ranges we solved ourselves. Vendor charts are licensed for personal study.
* Stack: Rust core, Tauri 2, React and TypeScript, PixiJS table, SQLite, Rive avatar.

## Decisions

* 2026-09-05: **Design direction.** Caleb chose a polished poker room with a friendly
  coach. The table is the main experience; the coach lives beside it; play and study
  connect both ways. Recorded by Astra in `../../ASTRA.md`.
* 2026-09-05: **Roles.** Claude leads development; Astra owns design, independent review,
  and security review; Caleb owns requirements and scope. Source: `../../ASTRA.md`.

* 2026-09-05: **Stack approved.** Rust solver and engine, Tauri 2, React with TypeScript,
  PixiJS table, SQLite, Rive later. Caleb's condition: work hand in hand with Astra while
  building (design before build, Astra reviews each plan and each phase).
* 2026-09-05: **Format order.** 6-max cash at 100bb is built and solved first. Stack depth
  is a setting from 10bb to 200bb from the start, never hardcoded. Cash and tournaments
  at 6, 8, and 9-max all follow on the same engine.
* 2026-09-05: **Coach behaviour.** The coach never interrupts play. Two entry points: a
  "why?" button on the hand just played, and an end-of-session review that sorts hands
  into correct, good, medium, and bad, from which Caleb picks the hands to see explained.
  No pre-action warnings in version 1.
* 2026-09-05: **Coach brain.** Claude Sonnet 5 with prompt caching, plus a templated
  offline fallback. Budget $5 to $20 a month accepted.
* 2026-09-05: **Coach look.** Text only in version 1. Face and voice in later versions.
* 2026-09-05: **Tournament formats.** All of them: freezeout MTTs, knockout, progressive
  bounty, sit-and-gos, satellites. Payouts are configurable (1 to N paid; satellites with
  N tickets), with ICM throughout.
* 2026-09-05: **Machine and audience.** Development machine has 16 GB RAM. The goal is a
  public launch so other people can use the app. Solves that ship must run on 16 GB
  consumer machines; heavy preflop solving happens once, elsewhere, and the results ship.
* 2026-09-05: **Charts.** Chart features stay dark in version 1 until our own multiway
  preflop solver produces ranges. Nothing vendor-owned is bundled. Before public launch,
  interactive click-through charts are a must.

Still open: how the coach reaches the Claude API in a public build (a small proxy service
versus a user-supplied key), and Windows code signing. Both are decided in the launch
phase of `../ROADMAP.md`.
