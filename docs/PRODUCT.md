---
type: product-brief
status: decided
date: 2026-09-05
---

# Product brief

What Caleb asked for on 2026-09-05, in his words where possible, and the decisions he
made the same day. Every plan in this repo is checked against this file.

## The one-line goal

An app that makes Caleb, and later other players, better at poker by being **fun and
educational at the same time**. His diagnosis of the market: every current tool is either
a game or a study tool, never both. This app is the in-between.

## What it must do

1. **Play real poker.** Sit at a table and play full hands against opponents:
   * Cash ring games and tournaments.
   * 6-max, 8-max, and 9-max tables.
   * Any stack depth from 10bb to 200bb.
   * Every tournament format: freezeout MTTs, knockout, progressive bounty, sit-and-gos,
     satellites, with configurable payouts (1 to N paid, N tickets) and ICM throughout.
2. **Do everything a solver does.** Build a spot, solve it, browse the strategy:
   frequencies and EV per hand, per street, per line. Postflop and preflop.
3. **Charts.** Look up and study preflop ranges for any position, format, and stack depth.
   Interactive, click-through charts are required before public launch.
4. **A little guy in your corner.** A coach who is there when asked:
   * A "why?" button on the hand you just played shows whether you were right or wrong
     and why.
   * At the end of a session, a review sorts your hands into correct, good, medium, and
     bad. You pick which hands to see explained.
   * The coach never interrupts play in version 1.

## How it should feel

Caleb's chosen direction, recorded by Astra in `../ASTRA.md`: **a polished poker room
with a friendly coach.**

* The table is the main experience and must stay readable at 6, 8, and 9 seats.
* Playing feels like playing, not like taking a test. Learning happens when you ask.
* Play and study connect: play a hand, ask for help, open the range or strategy, come
  back to the same decision.
* Every explanation is in plain language with the solver's numbers one click away.

## Decisions (all 2026-09-05)

| Topic | Decision |
|---|---|
| Roles | Claude leads development; Astra owns design, independent review, and security review; Caleb owns requirements and scope. Claude and Astra work hand in hand during the build. |
| Platform | Windows desktop app first (Tauri 2). Browser and mobile stay possible later. |
| Stack | Rust solver and engine, Tauri 2, React with TypeScript, PixiJS table, SQLite. Rive avatar later. |
| First format | 6-max cash at 100bb. Stack depth is a setting from 10bb to 200bb from day one. |
| Then | 8-max and 9-max cash, then every tournament format, on the same engine. |
| Coach timing | On request only: per-hand "why?" and end-of-session review with correct, good, medium, bad tiers. |
| Coach brain | Claude Sonnet 5 with caching and a templated offline fallback. $5 to $20 a month accepted. |
| Coach look | Text only in version 1. Face and voice later. |
| Charts | Dark in version 1 until our own preflop solver produces ranges. Nothing vendor-owned is bundled. Required before public launch. |
| Machine | Development on 16 GB RAM. Shipped solves must run on 16 GB consumer machines. |
| Audience | Caleb first, then a public launch. Licences stay MIT and Apache clean. |

## Assumptions still standing

| Topic | Assumed default | Decided in |
|---|---|---|
| Coach API access in a public build | A small proxy service that holds the key, with a user-supplied key as the alternative | Launch phase |
| Windows code signing | Azure Trusted Signing | Launch phase |
| Bot preflop ranges before our solver ships | Placeholder ranges we generate ourselves; clearly labelled | Phase 8 |
| Accuracy | Solves used for grading are under 1 percent of pot exploitability, labelled per grade | Phase 3 |

## Out of scope

* Real-money play or any connection to a poker site.
* Multiplayer with other humans.
* Hand-history import from poker sites (later).
