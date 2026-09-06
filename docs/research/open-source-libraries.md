---
type: research
status: draft
date: 2026-09-05
---

# Open-source libraries: depend on, read only, or avoid

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.
Extends the table in `how-to-build-a-solver.md` section 6.

## Question

Which open-source libraries should a Rust-core, TypeScript-frontend solver and trainer
depend on, and which should it only read, given their licences?

## Answer

Depend on krukah/robopoker's individually published crates (MIT) for hand evaluation
(`deuce`), the game engine (`kicker`), the MCCFR framework (`mccfr`), and subgame
solving (`subgame`). They are modular enough to use without the workspace, but they were
published to crates.io only in July 2026 with tiny download counts, so treat them as
young. Never depend on or copy from b-inary/postflop-solver (AGPL); read its API shape
and memory-estimation design only. For game engines, PokerKit (MIT, Python) is the most
tested reference for side pots and antes; rs-poker (Apache-2.0, Rust, active) is the
best Rust option and can be a dependency. Use OpenSpiel (Apache-2.0) as a Kuhn, Leduc,
and ACPC oracle in tests only.

Correction to `how-to-build-a-solver.md`: robopoker's evaluator is `deuce`, not
`kicker`. `kicker` is the game-state and settlement engine. Fixed there on 2026-09-05.

## Findings

### krukah/robopoker

* Crates on crates.io, all created 2026-07-12 at version 1.1.0: `pokerkit` (type
  aliases and constants; a name collision with the unrelated Python PokerKit), `deuce`
  (cards, hand evaluation, abstraction primitives), `monge` (optimal transport, Sinkhorn,
  earth mover's distance), `elkan` (accelerated k-means), `kicker` (game engine: state,
  actions, settlement), `mccfr` (game-agnostic Monte Carlo CFR), `subgame` (safe and
  depth-limited subgame solving), `nlhe` (no-limit hold'em MCCFR with Pluribus-style
  abstraction), plus `daybook` (Postgres) and `vitals` (OpenTelemetry).
  Sources: crates.io API for each crate and https://github.com/krukah/robopoker
* Licence: MIT, Kelechi Ukah 2024. Source: https://github.com/krukah/robopoker/blob/main/LICENSE
* Maturity: 902 commits, active, but the published crates had 43 to 143 downloads each
  at fetch time. No independent adoption signal. Test coverage and CI status unverified.
* Internal crates (`lloyd`, `kuhn`, `leduc`, `roshambo`, `bouncer`, `forge`, `parlor`,
  `portal`, `arena`, `spar`, `litmus`) are `publish = false`, so the published ones can
  be added as normal Cargo dependencies without the Postgres or telemetry infrastructure.

### b-inary/postflop-solver (AGPL-3.0-or-later, suspended October 2023)

* API shape from `examples/basic.rs` and docs.rs: bet sizes are a small DSL,
  `BetSizeOptions::try_from(("60%, e, a", "2.5x"))` (percent of pot, `e` for every
  size shortcut, `a` for all-in, `2.5x` for a multiplicative raise). Board and ranges go
  in `CardConfig { range: [oop, ip], flop, turn, river }`, tree shape in `TreeConfig {
  initial_state, starting_pot, effective_stack, ... }`. Build with
  `ActionTree::new(tree_config)` then `PostFlopGame::with_config(card_config, tree)`.
  Solve with `solve(&mut game, max_iterations, target_exploitability, print_progress)`.
  Query with `game.strategy()`, `game.expected_values(player)`,
  `game.expected_values_detail(player)`, `game.equity(player)`.
* Memory estimation is a real API surface worth copying the shape of:
  `memory_usage() -> (uncompressed, compressed)`, `memory_usage_bunching()`,
  `target_memory_usage()`, `allocate_memory(enable_compression)`.
  Source: https://b-inary.github.io/postflop_solver/postflop_solver/struct.PostFlopGame.html
* AGPL: read for design, never copy, never link.

### Game engines

* PokerKit (uoftcprg, Python, MIT): 234 commits, explicit side pots with multi-runout
  examples, uniform and non-uniform ante automation, cash and other modes, 99 percent
  coverage claimed in its paper. No ACPC or ICM support confirmed.
  Sources: https://github.com/uoftcprg/pokerkit and arXiv 2308.07327
* rs-poker (elliottneilclark, Rust, Apache-2.0): 177 stars, 436 commits, active. 2 to 16
  player arena with blinds, antes, and stacks, a single-table tournament mode, Monte
  Carlo ICM, and a claimed 50M+ hands per second evaluator (about 20 ns per 5-card, under
  25 ns per 7-card) using PDEP. Side-pot correctness not independently confirmed.
  Sources: https://github.com/elliottneilclark/rs-poker and https://crates.io/crates/rs_poker
* No TypeScript hold'em engine with confirmed multiway side pots and tournament handling
  was found. The Rust engine via WASM covers the browser case.

### Hand evaluators in Rust

* `deuce` (robopoker, MIT): claims fastest open-source; no independent benchmark.
* rs-poker (Apache-2.0): published numbers above.
* `poker` crate (deus-x-mackina, MIT, v0.7.0, June 2025): downloads static data at build
  time; no benchmark surfaced.
* `poker_eval`: 7-card lookup tables. Source: docs.rs/poker_eval
* A crate named `holdem-hand-evaluator` was not found.

### Range formats and grid libraries

* The `AKs, 77+` text syntax is read and written by PioSOLVER, GTO+, HRC, Flopzilla, and
  PowerEquilab; GTO Wizard calls its export "standard Pio/GTO+ text."
  Source: https://help.gtowizard.com/how-to-use-the-range-builder/
* UPI (Universal Poker Interface) is PioSOLVER's scripting protocol, modelled on chess's
  UCI: node IDs are colon-separated actions (`c` for check or call, `b<amount>` for bet
  or raise to a cumulative amount, cards appended as dealt).
  Source: https://piosolver.com/docs/upi/
* JoakimMich/opensolver (Rust, licence not stated, 5 stars, 25 commits, self-described
  first Rust project, about 2x slower than commercial, incomplete isomorphism) speaks
  UPI. A format reference, not a dependency.
* `@holdem-poker-tools/hand-matrix` (npm) is a React 13x13 matrix component; licence and
  maintenance unverified. AHTOOOXA/poker-charts is a full React 19 app to read for its
  grid, not to depend on.

### Reference oracles

* OpenSpiel (Apache-2.0): `universal_poker` wraps the ACPC engine (original C at
  https://github.com/ethansbrown/acpc), needs an optional build flag. Oracle only.
* noambrown/poker_solver (MIT, 165 stars): CFR, CFR+, external-sampling MCCFR, fictitious
  play, DCFR, Python reference plus C++, validated on Kuhn and Leduc. Commit count
  unclear from conflicting fetches; likely a rendering artifact.

### Licence summary

| Project | Licence | Implication for a personal app that might later be sold |
|---|---|---|
| robopoker crates (deuce, kicker, mccfr, subgame, nlhe) | MIT | Depend on freely, sell freely. |
| b-inary postflop-solver, wasm-postflop, desktop-postflop | AGPL-3.0-or-later | Read for design only. Never copy or link. |
| TexasSolver | AGPL | Read its config format only. |
| PokerKit | MIT | Safe to depend on (Python) or port ideas from. |
| rs-poker | Apache-2.0 | Safe to depend on; keep notices, state changes. |
| poker (deus-x-mackina) | MIT | Safe. |
| JoakimMich/opensolver | unstated | Treat as all rights reserved. |
| OpenSpiel | Apache-2.0 | Dev and test dependency only. |
| noambrown/poker_solver | MIT | Adapt test vectors freely. |
| poker-apprentice/icm-calculator | MIT | Port to Rust freely (see `tournaments-and-icm.md`). |

## Unverified

* robopoker crate test coverage and CI.
* Whether `kicker` handles multiway side pots, antes, and tournament structure natively.
* PokerKit tournament and ICM support.
* opensolver's licence and last commit.
* `@holdem-poker-tools/hand-matrix` maturity and licence.

## Also worth knowing

* opensolver is worth a five-minute skim for its bet-size DSL and its honest TODO list.
* The robopoker `pokerkit` crate and the Python PokerKit are unrelated; note it in any
  dependency docs.

## What this means for our plan

* Phase 1 evaluates `deuce` and rs-poker head to head on our own benchmark and
  property tests before either becomes a dependency. Two months on crates.io is not a
  track record; our tests are the track record.
* The solver's public API copies the *shape* of b-inary's (bet-size DSL, card config,
  tree config, solve with a target exploitability, strategy and EV queries, memory
  estimate before allocation). Same vocabulary as the tool everyone compares against.
* The game engine is our own Rust crate tested against PokerKit fixtures, with rs-poker's
  arena as a second oracle. A TypeScript engine is not needed; the Rust one runs in the
  browser via WASM if that day comes.
