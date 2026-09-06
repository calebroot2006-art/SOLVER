---
type: research
status: draft
date: 2026-09-05
---

# Bot opponents and the game engine

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

How can 5 to 8 bot opponents play a near-GTO strategy in real time at a full-ring table
on a consumer PC, and how does the coach get a solver answer for the user's spot fast
enough to grade it?

## Answer

Nobody solves the full-ring multiway tree live. Every real system (Pluribus, Libratus,
DeepStack, every commercial trainer) plays from a precomputed strategy, a blueprint or a
spot library, and does at most a small depth-limited re-solve for off-tree spots, mapping
real bet sizes onto the abstraction with action translation. Multiway is the open
problem: the machinery is proven heads-up, and commercial multiway solving is a recent,
compute-heavy, cloud-side feature. For the coach, a small heads-up postflop re-solve of
one flop, turn, or river node finishes in seconds on 8 cores at commercial accuracy,
which is fast enough to grade a decision after the fact. Multiway spots have no exact
analogue and must be approximated or left ungraded.

## Findings

### Blueprint plus real-time search

* Pluribus: offline blueprint by self-play CFR (8 days, 64-core server, 12,400 core
  hours, about $144 at spot prices), then depth-limited subgame search during play,
  assuming blueprint play beyond the depth limit but letting opponents switch to one of
  a handful of alternative strategies at the leaves. Live play used two Haswell CPUs and under
  128 GB RAM, a machine under $2,000, averaging 20 seconds per hand.
  Sources: https://www.science.org/doi/10.1126/science.aay2400 and
  https://en.wikipedia.org/wiki/Pluribus_(poker_bot)
* Libratus: nested subgame solving from the third betting round on and after every
  opponent bet, avoiding in-game action abstraction.
  Source: https://arxiv.org/pdf/1705.02955
* Action translation: the pseudo-harmonic mapping of Ganzfried and Sandholm (see
  `how-to-build-a-solver.md` section 3.3) is the accepted standard.
  Source: https://aaai.org/papers/aaaiw-ws1109-13-7185/
* DeepStack (heads-up) replaces a blueprint with continual re-solving driven by deep
  counterfactual value networks, on a single consumer GPU: median 5.7 seconds of thinking
  per hand, 2.3 seconds per action, on one GTX 1080. The strongest evidence that
  real-time solving on a consumer PC is possible, but heads-up only and dependent on
  trained value networks. Sources: https://arxiv.org/pdf/1701.01724 and
  https://www.amii.ca/updates-insights/deepstack-poker-ai

### Bot architectures in commercial trainers

* GTO Wizard AI generates full postflop trees up to 200bb with arbitrary bet menus in
  about 3 seconds per street at 0.1 to 0.3 percent of pot accuracy; its Play mode
  samples opponent actions from pre-solved or on-demand nodes, not from a live full solve.
  Sources: https://blog.gtowizard.com/gto-wizard-ai-explained/ and
  https://help.gtowizard.com/accuracy-and-benchmarks/
* GTO Wizard AI beat Slumbot by 19.4 bb/100 over 150,000 hands.
  Source: https://blog.gtowizard.com/crushing-a-top-hunl-poker-bot/
* Slumbot has a public HTTP API (post a game state, get an action) and plays a fixed
  precomputed equilibrium with no in-hand adaptation. The simplest possible architecture
  and a free benchmark opponent for our bots.
  Sources: https://github.com/Gongsta/Poker-AI/blob/main/slumbot/slumbot_api.py and
  https://stevengong.co/notes/Slumbot
* DTO Poker's practice bot plays from its precomputed library. Vendor site, lead only.
  Source: https://www.dtopoker.com/
* Poker Snowie's and Advanced Poker Training's internals could not be confirmed.

### The precomputed spot library

* 1,755 strategically distinct flops out of 22,100; GTO Wizard solves subsets of 25, 49,
  85, or 184 flops for aggregate reports. Source: https://blog.gtowizard.com/poker-subsets-and-abstractions/
* Extending every preflop spot to a full postflop tree made GTO Wizard's library "over
  50x bigger." That is how fast storage grows once every node is kept rather than
  sampled. Source: https://blog.gtowizard.com/single-size-solutions-are-live-new-pricing-50x-more-solutions/
* Off-tree handling is pseudo-harmonic translation; no vendor describes anything else.

### Fast approximate solving for the coach

* TexasSolver benchmarks: river solves under 1 second, turn usually within 10 seconds on
  a MacBook Pro; a flop with a modest tree (1 to 2 bets plus all-in) matches or beats
  PioSolver. Source: https://github.com/bupticybee/TexasSolver
* TexasSolver GPU solved a standard Qs Jh 2h flop in 24.4 seconds versus PioSolver CPU's
  86.1 seconds, both under 0.2 percent of pot. Source: https://forumserver.twoplustwo.com/167/poker-software/free-texassolver-gpu-high-performance-cuda-solver-4x-faster-than-cpu-1859729/
* GTO Wizard's 3 seconds per street is the best evidence that a river or turn re-solve
  for grading can land under 3 seconds when the tree is heads-up-sized and ranges are
  known. Vendor-reported, not independently reproduced.

### Multiway postflop grading

* GTO Wizard shipped 3-way postflop solving and a 9-player preflop solver, describing
  multiway as "a factor of 1000 or more" more complex than heads-up, made tractable with
  quantal response equilibrium approximations and neural assistance, server-side.
  Sources: https://blog.gtowizard.com/now_live_3_way_solving_nodelocking_2_0_and_50k_icm_ft_sims/ and
  https://www.pokerscout.com/gto-wizard-ai-multiway-postflop-solving/
* Simple 3-Way is a dedicated product with pre-solved packs (74 flops at one fixed
  stack and pot). The practical approach elsewhere is to narrow the format, not solve
  arbitrary multiway spots on demand.
  Source: https://forumserver.twoplustwo.com/45/general-software-discussion/simple-3-way-poker-solver-3-way-spots-postflop-1654542/
* No source shows any trainer grading a live 3-way decision mid-hand. That capability is
  not demonstrated anywhere found.

### Game engine requirements

* PokerKit (Python, University of Toronto) is peer-reviewed, supports uniform and
  non-uniform antes including big-blind ante, custom blind and straddle setups, and
  side-pot-correct simulation, validated against all 83 televised hands of the 2023 WSOP
  Poker Players Championship final table across nine variants. The correctness reference
  even though it is Python. Sources: https://arxiv.org/html/2308.07327 and
  https://github.com/uoftcprg/pokerkit
* rs_poker (Rust, Apache-2.0) advertises hand evaluation, side and main pot management,
  multiway arena simulation, a built-in CFR, and ICM tournament simulation. The closest
  Rust reference, self-described. Sources: https://docs.rs/rs_poker and https://crates.io/crates/rs_poker
* Neither library was confirmed to handle dead blinds or tournament blind-schedule
  progression. Unverified.

### Human-feel bots

* No academic source publishes a method for tilting solver frequencies into loose or
  tight profiles. Blog sources describe randomised timing and mixed-frequency sampling
  to avoid a fixed-latency tell. Sources (secondary): https://pokerbotrix.com/winning-poker-bots-2026/
  and https://pokergamedevelopers.com/build-winning-ai-poker-bot-strategy-systems/
* Solid grounding: a solver's output is already a mixed strategy, so sampling from it
  produces variance without a separate humanisation layer.

## Unverified

* Poker Snowie's and Advanced Poker Training's decision architectures.
* Any shipping trainer grading a live 3-way flop decision.
* A non-vendor benchmark of a turn-plus-river re-solve under 3 seconds on 8 cores.
* Dead blinds and blind schedules in PokerKit and rs_poker.

## Also worth knowing

* DeepStack is the better match than Pluribus for "real-time on a consumer PC" if the
  bot architecture ever moves to neural value functions. Worth its own research pass then.

## What this means for our plan

* Bots play from a spot library: preflop from solved multiway preflop ranges, postflop
  from pre-solved heads-up trees on a flop subset, with pseudo-harmonic translation for
  the user's off-menu sizes and a bounded live re-solve when the spot is not in the
  library. That is the Slumbot and GTO Wizard Play architecture, and it runs on a
  consumer PC because the heavy solving happened offline.
* The coach grades after the action, from the same library plus a live river or turn
  re-solve under a few seconds. A grade is always accompanied by its exploitability so a
  rough grade is never presented as an exact one.
* 3-way and larger postflop spots are graded as "approximate" (heads-up solve on the two
  most relevant ranges) or not graded, and the coach says so. Honest beats plausible.
* The game engine is written in Rust and tested against PokerKit's hand-history fixtures,
  which are the best public correctness oracle for side pots and antes.
