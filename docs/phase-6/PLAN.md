---
project: gto-solver-app
type: plan
status: proposed
date: 2026-09-09
---

# Phase 6: the game engine

Research: `docs/research/bots-and-game-engine.md`, `docs/research/tournaments-and-icm.md`.

Written by the `planner` subagent from a reader fact sheet on 2026-09-09, on Caleb's
instruction to start phases 5 and 6 beside phase 4. "Decisions" holds Caleb's answers,
with the date each one was given. Nothing else is assumed. Astra reviews this plan through
`docs/reviews/` before the executor starts (`docs/ROADMAP.md` principle 7).

## Progress (updated 2026-09-09)

Read this first when picking the work up. It says what is done and verified, what is half
done, and what was learned that the plan below did not know. The executor updates it after
every step it finishes; the main session updates it after review.

**Where it stands:** nothing started. Waiting on Astra's plan review and on open
question 1 (which blocks step 6 only).

## Task

Replace the phase 0 `engine` skeleton (`crates/engine/src/lib.rs:1-23`) with a hand engine
for 2 to 9 seats, blinds, antes, straddles, dead blinds, side pots, showdown, hand history,
and a single-table tournament; add an ICM stub to `payoff`. Gate: PokerKit fixture replay,
chip-conservation property tests, and the rs_poker arena as a second oracle
(`docs/ROADMAP.md:184-195`).

## Approach

Chips are `u64` integers throughout, so conservation is exact and side pots never round;
`payoff`'s real type appears only at the terminal boundary. The typed hand history is the
PHH format (PokerKit's hand-history TOML, parsed with the pinned `toml`), so fixture import
and export share one struct and round-trip by construction. Every observer gets a
`SeatView` from day one; no code path exposes another seat's hole cards before reveal.
Config structs follow `PostflopTreeConfig` (`crates/tree/src/postflop.rs:57-90`): explicit
fields with units, `validate()` returning a typed error, no `Default`. No new crates:
property tests use an in-crate seeded xorshift generator.

This phase owns `crates/engine/**` and `crates/payoff/**` only. It reuses `crates/cards`
read-only and never edits `crates/postflop`, `crates/tree`, `app/`, or `docs/phase-4/`,
where the phase 4 executors are working in parallel.

## Steps

**1. State model and information boundary.** Files: `crates/engine/Cargo.toml`,
`src/lib.rs`, `src/config.rs`, `src/state.rs`, `src/view.rs`, `src/error.rs`,
`crates/engine/README.md`. Dependencies: `cards` (path), `serde`, `thiserror`, `log`,
`toml`, all already pinned in the workspace. `TableConfig { seats: u8 (2..=9),
small_blind, big_blind, ante: AnteRule::{None, PerSeat(u64), BigBlindAnte(u64)},
straddles: Vec<u64>, min_bet }` with `validate()`. `HandState` (button, street, board,
per-seat `SeatState { stack, round_committed, hand_committed, status }`, action pointer,
pots). `HandState::view_for(seat) -> SeatView` and `public_view()`; no public method returns
another seat's cards while `status != Revealed`. `EngineError` via `thiserror`.
Tests: `validate()` rejects seats 1 and 10, an ante over the stack, a straddle below 2bb;
a `SeatView` serialised for seat 1 contains no seat 2 card bytes.
Gate: `check` job.

**2. Blinds, antes, straddles, betting rounds.** File: `src/betting.rs`. Depends on 1.
Posting order (ante, small blind, big blind, straddles, dead blinds as a per-hand
`Vec<(seat, u64)>` input), first to act per street, `legal_actions()` returning the minimum
and maximum raise, an all-in short raise that does not reopen action, round close,
uncontested pot.
Tests: one per rule, including heads-up button-posts-small-blind and a 9-seat straddle.
Gate: `check` job.

**3. Pots, showdown, chop, uncalled bet.** Files: `src/pots.rs`, `src/showdown.rs`,
`tests/property.rs`, `src/testing.rs` (xorshift PRNG). Depends on 2. Side pots with
eligibility sets, `cards::evaluate_holdem` ranking, chops with the odd-chip rule (first
seat left of the button), the uncalled bet returned before pot awards.
Tests: a hand-written case with three all-ins of different sizes and one fold, expected
pots per seat listed explicitly; a property test over 10,000 seeded random hands, 2 to 9
seats, random stacks from 1 to 300bb, random legal actions, with the invariant
`sum(stacks_after) == sum(stacks_before)` and every pot fully allocated; the seed and case
count are named constants with a comment.
Gate: `check` job.

**4. Hand history: typed struct and PHH text.** Files: `src/history.rs`, `src/phh.rs`.
Depends on 1; testable before 3. `HandHistory { variant, antes, blinds_or_straddles,
min_bet, starting_stacks, actions: Vec<HandAction>, finishing_stacks, players, metadata }`
with serde derive; `to_phh()` and `from_phh()`; `Display` for a plain one-line-per-action
text form. The parser rejects unknown action tokens loudly with the offending line number.
Tests: round trip on a hand-built history; malformed action strings fail naming the line.
Gate: `check` job.

**5. Fixture replay.** Files: `crates/engine/tests/fixtures/**/*.phh`,
`crates/engine/tests/fixtures/SOURCES.md`, `crates/engine/tests/replay.rs`. Depends on 3
and 4. Copy only PHH data files from PokerKit (MIT; the 83 WSOP 2023 Poker Players
Championship final-table hands, `docs/research/bots-and-game-engine.md:107-117`);
`SOURCES.md` records repository, revision, licence, and file count. Replay every fixture's
actions through the engine.
Gate: `finishing_stacks` identical for every hand; a mismatch prints seat, expected, actual,
and the last action. Runs inside the existing `check` job through `cargo test --workspace`;
no `ci.yml` change unless the fixture count needs an artifact.

**6. rs_poker oracle.** Files: `crates/engine/tests/oracle.rs`, `crates/engine/Cargo.toml`
(dev-dependency). Depends on 3. Blocked on open question 1. Seeded random hands, blinds
plus a uniform ante only (straddles and dead blinds excluded: rs_poker support is
unverified per the research note), 2 to 9 seats, the same action script fed to both.
Gate: identical finishing stacks over 2,000 hands; any difference reported with the PHH
text of the hand, and any rule divergence (odd chip, minimum raise) explained in a comment,
never waved off.

**7. Blind schedule and single-table tournament.** Files: `src/tournament.rs`,
`src/config.rs` additions. Depends on 3 and 4. `BlindSchedule { levels: Vec<Level {
small_blind, big_blind, ante: AnteRule, hands_or_minutes }> }` with validation
(non-decreasing blinds); `Tournament` runs hands, rotates the button through eliminated
seats (dead-button rule), records finishing order, and pays a `PayoutStructure`.
Tests: elimination order with a simultaneous bust (the larger starting stack finishes
higher), payouts sum to the prize pool, schedule advance.
Gate: `check` job.

**8. ICM stub.** Files: `crates/payoff/src/lib.rs`, `crates/payoff/README.md`. Independent
of 1 to 7. Add `PayoutStructure` (validated: finite, non-increasing, non-negative) and
`Icm { payouts }` implementing the unchanged `Payoff` trait: `try_utilities` returns
`Err(PayoffError("ICM not implemented in phase 6"))`, `utilities` fills NaN, matching
`ChipEv` (`crates/payoff/src/lib.rs:83-98`). `ChipEv` and the trait signature untouched.
Test: the stub never writes a finite number.
Gate: `check` job.

**9. Docs and handoff.** Files: `crates/engine/README.md`,
`docs/reviews/<date>-phase-6-engine.md`. Depends on all. README: how to run, fixture
provenance, the chip-unit contract, the view boundary. `slopcheck.py` clean.

## Tests

* `cargo test -p engine -p payoff --locked` and `cargo clippy --workspace --all-targets
  --locked -- -D warnings`, in CI (Smart App Control blocks local cargo for some sessions).
* Known answers: every PHH fixture reproduces `finishing_stacks` (step 5).
* Property: chip conservation and full pot allocation over seeded random hands (step 3).
* Oracle: rs_poker arena finishing stacks (step 6).
* Most likely failure path: a side-pot misallocation that still conserves chips (for
  example a folded all-in seat left eligible). Caught by step 3's hand-written three-all-in
  case with per-seat expectations, and by the fixture replay, which checks per-seat stacks
  rather than the sum.
* Boundary failure path: `view_for(seat)` on a hand with two revealed and one mucked seat
  shows only the revealed cards.

## Risks and edge cases

* Heads-up blind order, the dead button after an elimination, and a short all-in raise
  reopening action each get a named unit test in steps 2 and 7.
* PHH files may carry actions the engine does not model (for example show-muck ordering);
  the parser rejects unknown tokens loudly rather than skipping them.
* Fixture licence: data files only, provenance in `SOURCES.md`, no PokerKit code.
* rs_poker rule divergences are explained in `oracle.rs` comments.
* Windows: fixture paths through `env!("CARGO_MANIFEST_DIR")`, no `/` literals.
* Chip units: `crates/tree` has its own `Chips` type for solver trees. The engine's `u64`
  and the tree's type meet in phase 8 when the bot maps a live hand onto a spot; the
  conversion is written there, not assumed here.

## Open questions

1. rs_poker's `arena` feature: enabling it changes `Cargo.lock` (transitive crates,
   unverified list; check `cargo tree -p rs_poker --features arena` in CI or WSL2). Caleb
   approves the feature and its lockfile update, or step 6 is deferred.
2. Straddle rules: under the gun only, or also button and Mississippi straddles and
   re-straddles? Does a straddle act last preflop?
3. Dead blinds: the engine accepts dead-blind postings as an input (the default), or also
   enforces the "new seat posts or waits" rule?
4. Rake: cash games rake-free (the product brief is silent), or a `RakeRule` field now?
5. Uncalled bet: returned before the showdown display (the default in step 3), or shown in
   the pot until reveal?
6. Blind schedule clock: levels by hand count (deterministic, the default), or minutes with
   an injected clock?

## Decisions

None yet. Format: `* 2026-09-09: question. Answer: ...`
