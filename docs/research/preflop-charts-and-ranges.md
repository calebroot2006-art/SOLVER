---
type: research
status: draft
date: 2026-09-05
---

# Preflop charts and range data

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

Where can the app get correct preflop ranges for 6-max, 8-max, and 9-max cash and
tournaments, what formats do they come in, and what would it take to generate our own?

## Answer

Free chart sites give visual or scraped ranges with no accuracy guarantee, and GTO
Wizard's terms forbid using downloaded ranges in any third-party application. Paid packs
are licensed for personal study, not redistribution. The de-facto interchange format is
the PioSOLVER range string (comma-separated hands with optional `:weight`), which every
tool reads and writes. The only path to ranges we can ship and trust is solving them
ourselves, and no free tool does a genuine 6-max multiway preflop solve today. That makes
our own multiway preflop solver (see `multiway-solving.md`) a real requirement, not a
nice-to-have. Until it exists, the app lets the user paste in their own ranges.

## Findings

### Free sources and their terms

* GTO Wizard's free tier is limited, and Article 7.5 of its terms says the user "may
  not monetize or otherwise use in commerce, individually or via third-party
  application, poker ranges, poker trees and poker charts downloaded or otherwise
  obtained from Service." Source: https://gtowizard.com/terms/
* Upswing Poker publishes free charts framed for personal use; no redistribution licence
  stated. Sources: https://upswingpoker.com/charts/ and https://upswingpoker.com/preflop/
* PokerCoaching publishes free downloadable GTO and exploitative charts.
  Source: https://pokercoaching.com/preflop-charts/
* GitHub chart viewers under MIT: davidt35/preflop_charts (no documentation of where its
  numbers came from) and AHTOOOXA/poker-charts (React 19, "multiple chart providers",
  origin of the data undisclosed). MIT covers the code, not necessarily the data, so
  scraped numbers may still carry the original provider's restriction.
  Sources: https://github.com/davidt35/preflop_charts and https://github.com/AHTOOOXA/poker-charts
* HoldemPokerTools/RangeAssistant is a free range builder and viewer with no bundled
  solved ranges. Source: https://github.com/HoldemPokerTools/RangeAssistant
* jbcazaux/preflop-academy stores ranges in a Prisma JSON schema; a data-model
  reference, not a range source. Source: https://github.com/jbcazaux/preflop-academy/blob/master/schema.prisma

### Paid packs and licences

* RangeConverter: Range Reg $20/month annual or $29 monthly, Range Pro $67 annual or $99
  monthly, individual sim files $198 to $398. 500+ preflop solutions. Personal study
  framing. Sources: https://rangeconverter.com/articles/range-reg-price-drop and
  https://www.vip-grinders.com/poker-tools/rangeconverter-review/
* Simple Preflop Holdem: $250 per year. Source: https://pokerstudysoftware.com/2024/12/19/simple-preflop-holdem-review/
* MonkerGuy and MonkerGuide sell per-pack Monker-format `.txt` ranges.
  Sources: https://www.monkerguy.com/ and https://monkerguide.com/site/ranges
* No paid source found that permits embedding ranges in a third-party app.

### Formats

* PioSOLVER/GTO+ range string: `AA,KK,AKs:0.5`. GTO Wizard's Ranges tab exports it;
  Pio, GTO+, HRC, Flopzilla, and PowerEquilab import it.
  Source: https://help.gtowizard.com/ranges-tab/
* HRC exports full strategy and EV trees as JSON and bulk ZIP.
  Sources: https://help.freebetrange.com/Import-HRC/ and
  https://www.holdemresources.net/blog/2023-hrc-v3-release/
* Monker packs are `.txt` in the same hand-string convention.
* No industry-standard JSON schema. Hobby projects key by position, then villain action,
  then 169-hand grid, then frequency per action.

### Size of a full chart set

* 6-max at one depth: RFI for 5 positions, vs-RFI for every position pair, vs-3-bet for
  every opener and 3-bettor pair, vs-4-bet, and squeeze for every opener, caller,
  squeezer triple. "Dozens" of grids per depth; no exact count published.
  Source: https://blog.freebetrange.com/article/smart-preflop-ranges-for-6-max-no-limit-holdem
* 9-max nearly doubles the positions and multiplies every layer.
* MTT libraries span about 10 to 100bb (one vendor lists 10, 15, 20, 25, 30, 40, 50, 75,
  100bb), each needing its own full matrix, doubled for ante versus no ante.
  Sources: https://www.preflopwizard.app/blog/mtt-preflop-strategy and https://www.monkerguy.com/

### How the products derive ranges

* MonkerSolver was the first widely used tool with true multiway solving, which is what
  RFI-vs-multiple-defenders and squeeze trees need. Paid, closed.
  Source: https://www.runitonce.com/nlhe/monkersolver-preflop-ranges-discussion/
* GTO Wizard's February 2026 hybrid engine (CFR plus neural network) does multiway
  preflop up to 9 players; no exploitability figure published yet.
  Sources: https://blog.gtowizard.com/introducing-multiway-preflop-solving/ and
  https://www.pokernews.com/news/2026/02/gto-wizard-launches-multiway-preflop-solver-50550.htm
* HRC v3 added postflop modelling on top of its ICM preflop solver.
  Source: https://www.holdemresources.net/blog/2023-hrc-v3-release/

### Building our own

* TexasSolver is the only widely cited open-source solver of real quality, but it is a
  heads-up postflop solver, not a multiway preflop one. The researcher reported it as
  MIT; its README says AGPL v3 (confirmed in `how-to-build-a-solver.md` and
  `open-source-libraries.md`). AGPL stands.
  Source: https://github.com/bupticybee/TexasSolver
* No open-source tool reproduces a genuine 6-max or 9-max multiway preflop solve.
  Building one means multiway CFR over the full preflop action tree with abstracted
  postflop, as described in `multiway-solving.md`.
* No hardware or wall-clock numbers found for a from-scratch multiway preflop solve.

### Chart study UX

* No direct user complaints surfaced in this pass. A secondary source rates GTO Wizard
  highest among web solvers for completeness and simplicity.
  Source: https://notpropoker.substack.com/p/poker-solvers-for-dummies-part-3

## Unverified

* Exact grid counts for full 6-max, 9-max, and MTT libraries.
* Upswing's and PokerCoaching's redistribution terms (full terms pages not fetched).
* Exploitability figures for GTO Wizard's multiway engine, RangeConverter, or Monker packs.
* User complaints about chart-study UX; needs a targeted forum search.

## Also worth knowing

* GTO Wizard's Article 7.5 rules out even a personal subscription as the data source for
  anything shipped. Clean paths are our own solves or a vendor licence that none of the
  reviewed vendors offer.
* MIT chart repos on GitHub are a licence trap for the data inside them.

## What this means for our plan

* The chart viewer and the chart drills are built data-agnostic: they load ranges in the
  PioSOLVER string format from files, so Caleb can paste in whatever he owns for personal
  use, and the app ships with only ranges we solved ourselves.
* The multiway preflop solver moves up the roadmap. It is what makes "look at charts"
  legal and correct at the same time, and it feeds the bots' preflop play.
* The range file format for the repo is the Pio string plus a small JSON wrapper
  (format, positions, stack depth, action, source, exploitability). Define it once in
  the solved-spot format work.

## Feasibility study, 2026-09-09 (Astra proposal 3, accepted by Caleb as Decision 13)

A `researcher` pass on whether our own multiway preflop solver (phase 11) can be sized
before phases 8 to 10 commit to it. Sources fetched 2026-09-09 unless noted.

**Compute for one representative configuration: no published number.** No vendor,
open-source tracker, or paper gives tree size, iterations to a stated exploitability,
wall time, cores, and RAM together for 6-max 100bb with one open, one 3-bet, one 4-bet,
and all-in. Partial anchors only: HRC's Classic licence refuses trees above 25,000 nodes
and its postflop abstraction runs 256 buckets (Classic) to 16k (Pro, experimental) per
street, and HRC advises capping active players at 4 or 5 even in larger games
(https://www.holdemresources.net/docs/tree-config/). MonkerSolver's accuracy-ceiling
runs take weeks on servers with very high RAM (`multiway-solving.md:67-68`). Pluribus's
whole-game 6-player blueprint (preflop plus bucketed postflop) trained in 8 days on 4
cores under 512 GB (`multiway-solving.md:48-49`), an upper-bound analogue only. Exact
tree sizing exists only for heads-up ACPC games (Johanson,
https://poker.cs.ualberta.ca/publications/2013-techreport-nl-size.pdf). Unverified; the
next check is a small prototype run of our own, not more reading.

**Postflop abstraction inside preflop solves.** HRC, Simple Preflop Holdem, and Monker
bucket postflop states by equity or hand class (250 to 16k+ buckets per street). GTO
Wizard's February 2026 multiway preflop engine uses CFR with a neural value estimator
trained on self-play and publishes no error or cost figure
(https://blog.gtowizard.com/introducing-multiway-preflop-solving/). No vendor publishes
an error bound against an unabstracted reference. Our plan has to set its own numeric
abstraction-error target; there is nothing to borrow.

**The OpenSpiel reference gate.** `universal_poker.h` in google-deepmind/open_spiel
confirms `kMaxUniversalPokerPlayers = 10` and the betting abstractions kFC, kFCPA,
kFCHPA, and kFULLGAME. The parameter for a true preflop-only game (zero board cards) was
not confirmed in the fetched file; the roadmap's caveat stands. Next check: read the
parameter registration in `universal_poker.cc` or load the game with `numBoardCards=0`.

**Scaling to the required formats.** Three table sizes x eleven depths x cash and ICM is
66 base solves, each yielding every chart layer as the vendors do. Total compute cannot
be estimated without the missing per-solve figure. Using Pluribus's blueprint as an
explicitly rough anchor gives about 528 core-days against a rented 32-core machine's
about 960 core-days in a month: plausible only if a preflop-only solve is no dearer than
that anchor and ICM does not multiply into an open-ended family of payout structures.
Multiway ICM at deep stacks is the reasoned hardest case (largest tree, non-linear
utility, no multiplayer convergence guarantee, `multiway-solving.md:39-44`); that is
inference, not a benchmark.

**Licence-clean cross-check data.** PokerBench (Apache-2.0,
https://huggingface.co/datasets/RZ412/PokerBench) is a solver-derived preflop decision
set usable as a cross-check, never as shipped data; its solver provenance and
multiway scope are unconfirmed. No MIT, Apache, or CC-licensed multiway solved chart set
was found. CC-licensed hand-history corpora are raw hands, not equilibria.

**What phase 11's plan must contain, from this study:**

* A self-measurement pilot before any timeline: one representative configuration solved
  by a prototype, with tree size, RAM, and time recorded.
* Verification of the OpenSpiel no-board-card parameter before that gate is called
  runnable.
* A numeric postflop-abstraction error target against a full-game reference on a small
  game, since no competitor publishes one.
* An honest count of tournament and ICM variants, not a flat factor of two.
* PokerBench checked for scope and provenance before it is used as a cross-check.
* The hardest case (multiway ICM, deep stacks) piloted first, to validate or correct the
  scaling guess before the full depth grid is committed.

**What this changes now.** The launch-critical chart requirement rests on a solver
nobody has published sizing for. That is not a reason to stop, but it is a reason to run
the pilot early: the pilot can start once phase 4's postflop core is stable enough to
serve as the bucketed rollout, without waiting for phases 8 to 10.
