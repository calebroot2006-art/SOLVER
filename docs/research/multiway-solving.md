---
type: research
status: draft
date: 2026-09-05
---

# Multiway solving: 3+ players and full-ring tables

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

How do solvers handle multiway play (3+ players, 6-max, 8-max, 9-max), preflop and
postflop, and what is realistic for a personal project that wants to solve and train
full-ring cash spots?

## Answer

Multiway poker is not a two-player zero-sum game, so CFR has no convergence guarantee
there. It still produces strong strategies in practice: Pluribus beat professionals at
6-player using a CFR blueprint plus real-time search. Commercial products handle the two
halves differently:

* **Preflop multiway** is solved by abstracting postflop play into buckets (HRC, Simple
  Preflop Holdem, MonkerSolver). GTO Wizard shipped a 9-player preflop solver in
  February 2026 that mixes CFR with a neural value estimator and solves in seconds.
* **Postflop multiway** is solved exactly only for small trees and mostly 3-way. GTO
  Wizard AI does exact 3-way river solves and neural-assisted 3-way turn solves.

No open-source project solves 3+ player postflop trees at production quality. The
realistic layering for us: solve preflop multiway with bucketed postflop, solve postflop
heads-up on the ranges that reach the flop after folds, and treat true 3-way postflop
(limped pots, squeezed calls) as a later stretch goal.

## Findings

### Theory and Pluribus

* CFR is proven to converge to Nash equilibrium only in two-player zero-sum games. For
  multiplayer games there is no strong theoretical guarantee, and the Pluribus authors
  say so. Source: https://www.lesswrong.com/posts/6qtq6KDvj86DXqfp6/let-s-read-superhuman-ai-for-multiplayer-poker
* Pluribus's authors judged finding a true Nash equilibrium in 6-player poker "extremely
  hard and possibly not even worth it" and optimised for beating strong humans instead.
  Source: https://vitalab.github.io/article/2019/07/24/Superhuman-AI-For-Multiplayer-Poker.html
* Pluribus design: an offline blueprint trained with a modified Monte Carlo CFR, then
  depth-limited real-time search against biased opponent models (fold-, call-, and
  raise-leaning). Bet sizes and similar hands were bucketed. Same source.
* Pluribus beat five pros over 10,000 hands at roughly 5 bb/100, trained in 8 days on a
  single 4-core server with under 512 GB RAM and no GPU. Same source. The primary Science
  paper returned a 403, so these numbers come from a secondary summary.
* A 2018 paper on a 3-player equilibrium agent is a lead for the academic side, not read
  in full: https://arxiv.org/pdf/1804.04789

### Preflop multiway in practice

* HRC (HoldemResources Calculator) uses a proprietary imperfect-recall bucketing that
  needs much less memory than competitors, and describes itself as the first product to
  make 6-max and 9-max deep-stacked preflop calculations feasible on consumer hardware.
  HRC Classic caps at 50,000 nodes; HRC Pro is limited by RAM.
  Source: https://www.holdemresources.net/blog/2023-hrc-v3-release/
* Simple Preflop Holdem claims to be the first with true 3+ player preflop solving, by
  bucketing postflop hands and boards by equity so each bucket is one combined hand
  postflop. Vendor marketing, so secondary.
  Source: https://simplepoker.com/en/Solutions/Simple_Preflop_Holdem
* MonkerSolver builds full preflop trees (opens, 3-bets, 4-bets, limps, squeezes) with
  hand-class buckets, and its own help page says simulations "can take weeks to complete
  on dedicated servers with very high RAM requirements."
  Source: https://www.monkerguy.com/help.htm
* GTO Wizard's multiway preflop solver (February 2026) supports up to 9 players, custom
  raise sizes, limping, straddles, and asymmetric stacks, and solves in seconds in the
  browser. It uses CFR plus a neural network value estimator trained by self-play over
  randomised stacks, blinds, rake, and antes, with a "Fast Mode" (short lookahead,
  frequent re-solve) and a "Classic Mode" (street by street, better for node locking).
  Sources: https://blog.gtowizard.com/introducing-multiway-preflop-solving/ and
  https://www.pokernews.com/news/2026/02/gto-wizard-launches-multiway-preflop-solver-50550.htm
* The shared bucketing idea: group postflop hand-and-board states into buckets (from a
  few dozen to tens of thousands, such as "top pair good kicker, no backdoor flush draw").
  GTO Wizard says well-chosen buckets give "almost perfect" preflop accuracy.
  Source: https://blog.gtowizard.com/poker-subsets-and-abstractions/

### Postflop multiway in named products

* PioSOLVER and GTO+ support multiway postflop but not multiway preflop, per a vendor
  comparison thread. Secondary source, not confirmed against their own docs.
  Source: https://forumserver.twoplustwo.com/69/online-no-limit-holdem-cash/what-solver-should-i-buy-1787077/
* MonkerSolver is credited as the first solver with multiway postflop. Same secondary
  source.
* GTO Wizard AI built an exact 3-way river solver (Nash distance under 0.1 percent of pot
  within seconds on 100 test spots) and a neural-assisted 3-way turn solver averaging
  0.24 percent of pot EV loss, versus traditional solvers taking minutes per turn spot on
  7.5 to 16 million node trees. They say multiway equilibrium is "more of an ill-defined
  problem" and their target is closer to a quantal response equilibrium than pure Nash.
  Source: https://blog.gtowizard.com/gto_wizard_ai_3_way_benchmarks/
* TexasSolver's README documents no multiway support; all examples are heads-up.
  Source: https://github.com/bupticybee/TexasSolver

### Open-source multiway

* OpenSpiel's `universal_poker` can represent N-player no-limit trees, but Google's own
  tutorial says 3-player *limit* hold'em alone has about 5 x 10^17 information sets,
  needing "hundreds of petabytes of RAM" for tabular CFR without abstraction.
  Source: https://mlanctot.info/files/open_spiel_tutorial-mar2021-comarl.pdf
* open-pure-cfr says average-strategy computation "is not currently supported" for games
  with more than two players. Source: https://github.com/rggibson/open-pure-cfr
* RLCard's CFR implementations are fixed to 2 players. Source: https://github.com/datamllab/rlcard
* robopoker: no documentation found describing 3+ player support; unverified.
  Source: https://github.com/krukah/robopoker
* Net: no open-source project found that solves a full-scale 3+ player no-limit postflop
  tree to solver-grade accuracy. Academic work stays at toy scale, such as 3-player Kuhn:
  https://arxiv.org/pdf/1704.08124

### The bunching effect

* When earlier players fold, their folding ranges skew what is left in the deck (aces are
  more likely to be live), which changes equities, runouts, and strategy versus plain
  card removal. Source: https://www.runitonce.com/nlhe/solvers-removal-and-bunching-effect/
* Magnitude: in a 9-max push-fold spot at 7.5bb folded to the small blind, aces appear
  26.5 percent more often than in a random deck and deuces 7.6 percent less. At 100bb the
  skew is about 16.5 and 9.1 percent. About 24 percent of combos in a range solved
  without bunching become unprofitable pushes once bunching is modelled, some losing up
  to 0.5bb. Source: https://www.holdemresources.net/blog/card-bunching-effects/
* HRC handles bunching with a Monte Carlo engine mode, which its blog recommends as the
  default for experienced users. Same source.
* b-inary/postflop-solver says it is the only implementation that handles bunching,
  supporting up to 4 folded players in 6-max by counting card combinations exactly.
  Enabling it slows the solve noticeably, in the author's words "significantly",
  because terminal evaluation gets more expensive. Source: https://github.com/b-inary/postflop-solver/blob/main/README.md

### The layering the commercial products use

Inferred by combining the findings above, not stated by any one source:

1. Preflop multiway via bucketed or abstracted postflop rollouts.
2. Postflop heads-up on the ranges that survive to the flop after folds.
3. True 3+ way postflop only for the narrow high-value cases (limped pots, squeeze
   calls, 3-bet pots with a cold-caller), and there with neural assistance or exact but
   narrow (river-only) solves rather than full-tree CFR.

### Compute budgets

* HRC defaults to a 4 GB memory limit and says 6 to 8 GB of RAM is fine for standard
  runs; deep-stacked 6-max and 9-max preflop is its stated consumer-hardware target.
  Forum reports: simple 6-max trees in seconds, full trees with postflop modelling need
  "some patience". No hour figures found.
  Sources: https://www.holdemresources.net/blog/2023-hrc-v3-release/ and
  https://forumserver.twoplustwo.com/167/poker-software/holdemresources-calculator-hrc-1705032/index43.html
* MonkerSolver at the accuracy ceiling: weeks on dedicated servers.
* Pluribus's blueprint: 8 days, 4 cores, under 512 GB RAM. An upper bound for a serious
  6-player blueprint, but a research system.
* GTO Wizard AI's "seconds" per 3-way spot depends on a pre-trained network built on
  server-scale infrastructure, so it is not comparable to raw CFR on a personal machine.

## Unverified

* Concrete hour or core-hour figures for HRC or Simple Preflop Holdem on an 8 to 16 core,
  32 to 64 GB machine. Only qualitative statements were found.
* Whether PioSOLVER and GTO+ truly cannot solve multiway preflop or merely do it badly.
  Single secondary source.
* TexasSolver's and robopoker's actual multiway capability; both need a code-level check.
* Pluribus numbers were not checked against the primary Science paper (403).
* HRC's Monte Carlo bunching mechanism is described only by the vendor.

## Also worth knowing

* GTO Wizard's February 2026 multiway preflop solver is the strongest precedent that a
  CFR plus neural-network hybrid can make 9-max preflop interactive. Relevant if this
  project ever goes beyond bucketed preflop.
* Bunching matters most at shallow stacks and in push-fold, and less at 100bb cash. For a
  100bb cash trainer it is a real but second-order correction: model it eventually, using
  b-inary's approach as the blueprint, but it does not block a first version.
* b-inary/postflop-solver is the single most relevant open-source reference for both
  DCFR postflop and bunching. Read it, do not copy it (AGPL, see
  `how-to-build-a-solver.md` section 6).

## What this means for our plan

* 6-max, 8-max, and 9-max *preflop* is a bucketed-abstraction solver, a separate module
  from the postflop solver, and the harder research problem of the two. Plan it as its
  own phase with its own reference comparisons (HRC's free outputs, GTO Wizard's free
  spots).
* Full-ring *postflop* play is graded by solving heads-up on the surviving ranges. That
  covers the large majority of hands that reach a flop. 3-way flops are flagged as
  ungraded or approximately graded until a 3-way solver exists.
* The bunching correction goes on the roadmap after the heads-up postflop solver is
  validated, not before.
