---
type: research
status: draft
date: 2026-09-05
---

# Tournaments and ICM

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

What does a solver and trainer need to handle tournament poker correctly, and
how do the existing products do it?

## Answer

Tournament play adds one layer on top of a chip-EV solver: an equity function that turns
stack sizes into dollar equity, substituted at terminal nodes so CFR optimises dollars
instead of chips. On top of that: a bounty extension for knockout formats, push-fold
solving as a special small tree, and a tournament-structure model (blind levels, antes,
big-blind ante, payouts, field size, table balancing) that feeds the equity layer. Every
product reviewed (HRC, ICMIZER, GTO Wizard, RangeConverter, DTO) uses Malmuth-Harville
ICM as ground truth and approximates it for large fields. Future Game Simulation (FGS) is
ICMIZER's extension that looks a few hands ahead instead of taking one snapshot. No
product documents a full multi-table tournament engine with table balancing in technical
detail.

## Findings

### ICM models

* Malmuth-Harville: probability of finishing first equals your share of chips; the
  probability of finishing kth is computed recursively by removing the higher finishers'
  chips and renormalising. Exact computation sums over finishing-order permutations and
  is exponential in player count. The open-source poker-apprentice implementation says
  exact dynamic programming is fine for about 19 to 30 players and needs Monte Carlo
  above that. Sources: https://en.wikipedia.org/wiki/Independent_Chip_Model and
  https://github.com/poker-apprentice/icm-calculator
* HRC says "naive ICM implementations can handle about 15 players, and even optimized
  versions can't calculate exact Malmuth-Harville values beyond 25-30 players." HRC's
  newer large-field algorithm works on the original stacks and prizes with a full
  variant exact to 500 players on desktop and a fast approximation beyond, with mean
  error under 0.01 percent for the full variant and 1.8 to 11.5 percent for the fast
  one depending on payout structure.
  Source: https://www.holdemresources.net/blog/high-accuracy-mtt-icm/
* GTO Wizard claims a purely algorithmic technique (no sampling, no machine learning)
  that gives precise ICM values for fields of thousands in a fraction of the usual time,
  and defines Chip-Scaled Tournament Equity (total chips times dollar EV divided by
  remaining prizes) for comparing ICM to chip EV across formats. Vendor claim, not
  independently reproduced.
  Source: https://blog.gtowizard.com/theoretical-breakthroughs-in-icm/
* Malmuth-Weitzman: referenced as an alternative in some calculators (an iOS app lists
  "Malmuth Weitzman and Ben Roberts"), but no primary formula was found. Unverified.
  Source: https://github.com/isaacflaum/ICMCalculator
* Future Game Simulation: "essentially a recursion of ICM that calculates a few rounds
  in advance," accounting for position, rising blinds, and future play. ICMIZER's own
  FGS page returned a 403, so this is from a secondary explainer.
  Source: https://www.poker.pro/strategy/poker-as-a-game-of-war-how-fgs-future-game-simulation-transforms-tournament-strategy/
* Where ICM is wrong, per GTO Wizard: it ignores blind escalation (bad in fast structures
  and deep early levels), assumes equal skill, and is a single snapshot with no look-ahead.
  Source: https://blog.gtowizard.com/when-icm-breaks-down/

### ICM inside a solver

* Bubble factor = (dollar EV lost if you bust) divided by (dollar EV gained if you bust
  someone). Risk premium restates it: RP = (BF / (BF + 1)) x 100 minus 50.
  Sources: https://blog.gtowizard.com/what-is-the-bubble-factor-in-poker-tournaments/ and
  https://bbzpoker.com/the-complete-guide-to-independent-chip-model-icm/
* Mechanically an ICM-aware solver replaces the chip payoff at each fold and showdown
  leaf with the dollar equity of the resulting stack distribution, so regret updates
  are driven by dollar deltas. This is how vendors describe it; no paper or open-source
  solver was found that shows the substitution step in code. Inferred, not confirmed.
  Source: https://blog.gtowizard.com/icm-basics/
* In bounty formats, bubble factors can drop below 1 and risk premium can go negative,
  so a call can be right at worse than chip-EV pot odds. A real reversal of the cash
  intuition. Source: https://blog.gtowizard.com/the-theory-of-bounty-tournaments-part-3-key-calculations/

### Push-fold and short stacks

* Nash push-fold charts are the heads-up equilibrium of a shove-or-fold game per stack
  depth and are not valid for full-ring when action folds around.
  Source: https://www.888poker.com/magazine/strategy/push-fold-charts
* ICMIZER computes Nash push-fold in seconds and layers FGS on top; ICM tightens ranges
  near pay jumps. Generic charts are approximations; real thresholds vary with stack
  and ante. Source (secondary): https://deepfold.co/en/blog/push-fold-complete-guide

### Structure inputs a game engine needs

* Big-blind ante: only the big blind posts the ante for the table (usually equal to the
  big blind), short-handed tables often reduce it, and a short stack pays the blind
  before the ante if it cannot cover both.
  Source: https://www.pokernews.com/pokerterms/big-blind-ante.htm
* Progressive knockout split: roughly 50 percent prize pool, 25 percent capturable
  bounty, 25 percent added to the eliminator's own bounty. Eliminating a player pays
  half their bounty in cash and adds half to yours.
  Source: https://bbzpoker.com/ultimate-guide-to-pko-tournament-strategy/
* GTO Wizard's Bounty Power = total chips in play divided by (remaining bounty pool plus
  remaining prize pool), converting bounties to chip-equivalents so they fit the same
  solving framework. Source: https://blog.gtowizard.com/bounty-models-explained-solving-knockout-tournaments/
* Satellites: listed as a supported format by GTO Wizard's custom ICM solver (with KO,
  freezeout, PKO, mystery bounty, up to 4096 players) but the seat-or-bust payoff
  mechanics were not found. Unverified. Source: https://gtowizard.com/

### What the products offer

* GTO Wizard: custom ICM solving for any heads-up postflop spot across those formats, an
  MTT preflop and postflop library, and a trainer with drills and stats.
  Source: https://blog.gtowizard.com/revolutionizing-mtts-the-ultimate-icm-solver-upgrade/
* DTO Poker: trainer plus explorer, 3-way and ICM postflop sims (claimed unique),
  aggregated flop reports, hand replay, and social comparison.
  Source: https://www.dtopoker.com/tournament
* RangeConverter: MTT, ICM, and PKO preflop from 60bb down to 8bb push-fold, 8-max from
  10bb to 100bb+, downloadable for Pio and Monker.
  Source (secondary): https://elitepokerguide.io/rangeconverter-gto-solutions-2026/
* ICMIZER: fast Nash push-fold plus FGS across SNG, MTT, spin, heads-up SNG, and bounty.
* No source describes any product's tournament grading formula beyond "compare to solver
  EV and frequency." Unverified.

### Simulated tournament play

* GTO Wizard Play Mode is single-table only (up to 9 players) with Regular, Hyper, and
  Cash presets and customisable blind levels, stacks, and level timing. No multi-table
  field, table breaking, or balancing found.
  Source: https://blog.gtowizard.com/introducing-gto-wizard-play-mode/
* Advanced Poker Training advertises a full MTT simulator with 18 to 8,000 bots and a
  customisable structure, with "human-like" and "GTO-style" bot flavours. Its most
  technical public write-up covers a single final table simulated a million times from a
  fixed seat draw, with bots carrying 40+ behavioural traits including ICM pressure. The
  multi-table mechanics are not disclosed.
  Sources: https://www.pokersoftware.com/articles/2014/08/advanced-poker-training-launches-full-mtt-simulator.html
  and https://www.pokertraining.com/poker/blog/2026-wsop-main-event-simulation-bots/

### Open-source ICM code

* poker-apprentice/icm-calculator: TypeScript, MIT, exact Malmuth-Harville by dynamic
  programming over player subsets (final tables up to about 19 to 30 stacks) with a
  Monte Carlo fallback. Cleanest licence and language match for the UI; the solver core
  would want a Rust port. Source: https://github.com/poker-apprentice/icm-calculator
* gpratte/icm-calculator: Malmuth-Harville; language and licence unconfirmed.
* isaacflaum/ICMCalculator: iOS; the only lead on Malmuth-Weitzman; details unconfirmed.

## Unverified

* The Malmuth-Weitzman formula.
* The exact CFR terminal-node substitution for ICM, from code or a paper.
* Satellite payoff modelling.
* Per-product tournament grading formulas.
* Multi-table mechanics in Play Mode (appears absent) and APT (claimed, undocumented).
* gpratte/icm-calculator's licence and language.

## Also worth knowing

* ICMIZER's FGS explainer at icmizer.com returned 403 to the fetch tool; a manual browser
  visit would fill the algorithmic gap.

## What this means for our plan

* The solver core needs a pluggable terminal payoff: chip EV for cash, an ICM function
  for tournaments, and a bounty-adjusted function for knockouts. Design that interface
  from the start even if only chip EV ships first.
* The ICM function is a small, testable module (port of the poker-apprentice algorithm
  to Rust, with its Monte Carlo fallback), with a bubble-factor calculator the coach can
  cite in explanations ("you need 58 percent equity here, not 50, because busting costs
  more than doubling gains").
* The game engine's tournament model starts single-table (like GTO Wizard Play Mode):
  blind schedule, big-blind ante, payouts, elimination. Multi-table with balancing is a
  later phase and nobody has published how to do it well.
