---
type: research
status: draft
date: 2026-09-05
---

# How to build a poker solver

Research for the GTO Solver APP. Written 2026-09-05 from primary sources (papers, repo
READMEs, vendor docs). Anything marked *unverified* was not confirmed against a source.

## Question

What does it take to build a no-limit hold'em GTO solver good enough to train against,
and which parts can we borrow instead of writing?

## Short answer

A solver is a program that finds a Nash equilibrium of a *simplified* version of poker:
fixed ranges, a fixed menu of bet sizes, one board. Every serious solver (PioSOLVER,
GTO Wizard, TexasSolver, the open-source Rust ones) does the same four things:

1. Build a game tree from the ranges, stacks, and bet-size menu.
2. Run a Counterfactual Regret Minimization (CFR) variant over it until the strategy is
   within some fraction of the pot of unexploitable. Discounted CFR (DCFR) is the current
   standard.
3. Evaluate showdowns fast, which is where all the engineering is.
4. Show the result as per-hand action frequencies on the 13x13 grid.

The trainer on top is comparatively simple: pick a spot from a library of solved trees,
show the user a hand, compare their choice to the solver's frequencies, and score the
EV lost.

Recommended path: write the solver core in Rust as a library crate, ship it inside a
Tauri 2 desktop app with a React/TypeScript front end, and keep a WASM build of the same
crate so the trainer can run in a browser later. Postflop first, preflop later. Details
and reasoning below.

## 1. What "solving" means

* Poker is a two-player zero-sum imperfect-information game (multiway is not zero-sum
  between any two players, which is why multiway solving is a research problem and
  everyone ships heads-up).
* A Nash equilibrium strategy pair cannot be exploited by any counter-strategy. The
  solver's output is the equilibrium of the *abstracted* game, not of real poker.
  GTO Wizard's own writing is blunt about this: "No-Limit Hold'em is far from solved."
* Convergence is measured as **exploitability**, the amount a best response could win
  against the current strategy, quoted as a percentage of the starting pot. GTO Wizard
  solves its library to 0.4 to 0.8 percent of pot; anything under 1 percent is fine for
  study. A best-response calculator is therefore part of the solver, not an extra.

## 2. The algorithm family

All from the CFR line of work. Each step below is a small change to the previous one.

| Variant | What changes | When to use |
|---|---|---|
| Vanilla CFR (Zinkevich 2007) | Walk the whole tree each iteration; accumulate regret per information set per action; play proportional to positive regret; the *average* strategy converges. | Learning and tests on Kuhn or Leduc. Too slow for hold'em. |
| CFR+ (Tammelin 2014) | Floor regrets at zero after each update, alternate player updates, weight later iterations more in the average. Used to solve heads-up limit hold'em in 2015. | Baseline for a full-tree postflop solver. |
| Discounted CFR (Brown and Sandholm 2018) | Multiply accumulated positive regret by t^α/(t^α+1), negative regret by t^β/(t^β+1), and strategy contributions by (t/(t+1))^γ each iteration t. Paper's recommended values: α=1.5, β=0, γ=2. Outperforms CFR+ in every game tested. | **Use this.** b-inary's postflop-solver uses DCFR with γ=3 and resets the strategy sum at iterations that are powers of 4. |
| Linear CFR | The special case α=β=γ=1. Simpler, nearly as good, and compatible with sampling. | If DCFR's tuning gets in the way. |
| MCCFR, external sampling (Lanctot 2009) | Sample chance outcomes and the opponent's actions instead of walking everything. Converges slower per iteration, much cheaper per iteration. | Preflop or full-game trees that do not fit in memory. robopoker and cfr-edge use it. |
| Deep CFR / neural variants | Replace the regret tables with a network. | Not for us. Research-grade, hard to validate. |

The DCFR parameters α=1.5, β=0, γ=2 are confirmed from the paper (pages 3 to 4), and
b-inary's γ=3 with power-of-4 resets is confirmed from its `src/solver.rs`. The exact
update rules, the best-response method, and the terminal-evaluation algorithm are in
`solver-algorithms.md`.

## 3. The components of a postflop solver

Listed in the order they should be built. Each one is independently testable.

### 3.1 Cards and hand evaluation

* 52 cards, 1,326 two-card combos, 7-card hand ranking.
* A solver does not call a generic evaluator in the inner loop. On each river it
  ranks every combo in each range *once*, sorts them by strength, and then computes every
  showdown against a whole range in one linear sweep of the sorted list (subtracting the
  combos blocked by the hero's own cards). That sorted-sweep trick is the single most
  important optimisation in a postflop solver. It is Johanson et al.'s 2011 algorithm
  (see `solver-algorithms.md` section 4 for the exact three-bucket sweep and the
  inclusion-exclusion blocker correction).
* For the one-off ranking, or for an equity calculator in the trainer, use an existing
  evaluator. Candidates:
  * OMPEval (C++): 200 kB perfect-hash tables, SSE, roughly 2 to 10x faster than the
    older ACE and 2+2 evaluators at random evaluation. The 2+2 evaluator is fastest
    sequentially but its table is 130 MB.
  * PokerHandEvaluator / phevaluator (C, Python bindings): ~100 kB table for 7 cards,
    perfect hash, also does Omaha.
  * robopoker's `deuce` crate (Rust, MIT) claims the fastest open-source evaluator (its `kicker` crate is the game engine, not the evaluator).
    Being Rust and MIT, it is the natural pick if the core is Rust.

### 3.2 Ranges

* A range is 1,326 weights in [0, 1]. The UI edits them on the 13x13 grid (169 hand
  classes, expanded to suited/offsuit combos), with the standard "AKs, 77+, T9s:0.5"
  text syntax so ranges can be pasted from anywhere.
* Card removal: combos that collide with the board or the other range's cards are zeroed
  before solving.

### 3.3 Action abstraction (the bet-size menu)

* Real no-limit allows any size; the solver allows a short list per street and per
  player, plus all-in, plus a raise cap. GTO Wizard's published model for 100bb heads-up
  gives SB the choices 33, 67, 100, 150 percent of pot or all-in, with a cap of 5 bets or
  raises per street after which the action becomes all-in. GTO Wizard also found that two
  well-chosen c-bet sizes (one in 20 to 35 percent, one in 55 to 85 percent) lose no
  measurable EV versus three.
* An "all-in threshold": if a bet would leave less than some fraction of the stack
  behind, it becomes all-in. Cuts the tree a lot.
* Off-tree translation for the trainer: when a real opponent bets a size not in the
  menu, map it with the pseudo-harmonic mapping from Ganzfried and Sandholm:
  f(x) = (B - x)(1 + A) / ((B - A)(1 + x)) gives the probability of treating x as the
  smaller size A rather than the larger B. This is the accepted answer and beats naive
  nearest-size rounding.

### 3.4 Card abstraction

Two different things get called "abstraction":

* **Lossless suit isomorphism.** On a monotone flop, the two absent suits are
  interchangeable, so turn and river cards of those suits can share one subtree. On a
  rainbow flop nothing merges. This is free accuracy and b-inary's solver does it; it
  roughly halves memory on many boards. Waugh's hand-indexing work gives the fast
  canonical index.
* **Lossy bucketing** (E[HS], E[HS²], potential-aware k-means on equity histograms with
  earth-mover's distance). Needed only when solving preflop or the whole game at once,
  because the full tree does not fit. GTO Wizard's preflop solves use bucketed postflop
  play and report "almost perfect" preflop results from it. Not needed for a postflop
  solver, which solves one board exactly.

### 3.5 The game tree and memory

* Nodes: player decision, chance (turn card, river card), terminal (fold or showdown).
* Per decision node per information set per action we store accumulated regret and
  accumulated strategy weight. For a whole range of ~1,000 combos, a node with 3 actions
  stores 3,000 regrets and 3,000 strategy sums. Multiply by the number of nodes across
  49 turns and 48 rivers and it is gigabytes at PioSOLVER-style tree sizes.
* b-inary compresses both arrays to 16-bit integers with a per-node 32-bit scale factor,
  uses 32-bit floats elsewhere and 64-bit only for summations. Memory is the real limit
  on how big a tree you can solve, so design storage layout early.
* Turn and river subtrees are independent given the card, so the traversal parallelises
  naturally across chance outcomes. Rayon in Rust does this in a few lines.

### 3.6 Terminal evaluation

* Fold: payoff is the pot arithmetic, but computed for every combo in the acting range
  at once as a vector, weighted by the opponent's reach probabilities minus blocked
  combos.
* Showdown: the sorted-sweep described in 3.1, again as a vector over the whole range.
* The whole solver is therefore "vector CFR": every operation is over an array of
  1,326 (or fewer) combos, which is why SIMD helps and why Rust or C++ beats Python by
  two orders of magnitude here. Python is for prototyping Kuhn and Leduc only.

### 3.7 Convergence and output

* Stop on exploitability below a target (say 0.5 percent of pot) or an iteration cap.
* Output per node: strategy frequencies per combo, EV per combo per action, and equity.
  That is exactly what the UI and the trainer consume, so define this data format once
  and serialise it (JSON for small, a compact binary for the spot library).

## 4. Preflop is a different problem

* A preflop tree with postflop play attached is far too large to solve exactly, so
  every product either (a) bucket-abstracts the postflop streets, (b) replaces postflop
  play with an equity or EV model, or (c) ships fixed charts.
* For a trainer, the honest first version uses **published preflop ranges** (or ranges
  the user pastes in) as inputs and trains postflop. A preflop drill mode can quiz
  against a stored range chart without any solving.
* A real preflop solver is a later milestone: MCCFR over the preflop tree with a bucketed
  or equity-based rollout of later streets. robopoker (Rust, MIT) already has the
  k-means abstraction and MCCFR pieces; it is the reference to read when we get there.

## 5. The trainer layer

What the commercial trainers (GTO Wizard, GTOBase, GTO Gecko, Lucid, SOLVED GTO) all do:

* A library of pre-solved spots: positions, stack depth, pot type (single-raised, 3-bet,
  4-bet), board. Solve once, store, serve.
* Drill: deal a hand at a node, user picks an action (and size), the app shows the
  solver's frequencies and the EV of each action, and grades the choice. Grading is on
  EV lost, not on "matched the highest-frequency action", because mixed strategies make
  the second one wrong.
* Play-against-solver: the app plays the opponent's side by sampling from the solved
  strategy; off-tree user bets are translated (3.3).
* Range viewer with the 13x13 grid, per-street filters, and aggregate reports across
  many flops (GTO Wizard offers subsets of 25, 49, 85, and 184 flops out of the 1,755
  strategically distinct ones).
* A leak tracker: per spot category, the user's average EV loss over time.

None of this needs the solver to be fast at run time. It needs the solver to be
*correct* offline and a good data format in between.

## 6. What already exists (and what we can take)

| Project | Language, license | What it is | Status | Take-away for us |
|---|---|---|---|---|
| b-inary/postflop-solver | Rust, AGPL-3.0 | DCFR postflop library, isomorphism, 16-bit compression, SIMD, multithreaded; author claims faster than PioSOLVER and GTO+. | Suspended Oct 2023, author went commercial. | The reference design. AGPL: if we copy code and distribute the app, the app must be AGPL. Reading it for ideas is fine. |
| b-inary/wasm-postflop, desktop-postflop | Rust + Vue/Tauri, AGPL | Browser and desktop UIs on the library above. | Suspended. | Proof that Rust to WASM to browser works for a solver; desktop was ~2x faster than WASM in the author's tests. |
| jiyee/GTO-Solva | Rust + TS, Tauri, AGPL | Fork of desktop-postflop; wants to add node locking and training modes. | Barely maintained. | Nothing new, but shows the fork route others took. |
| bupticybee/TexasSolver | C++ (Qt GUI), AGPL | Postflop solver, JSON export, GPU version exists; benchmarked at 172 s vs PioSOLVER 242 s on one flop test. | Limited activity, 2.5k stars. | Second reference. Its config file format is worth borrowing for our tree spec. |
| krukah/robopoker | Rust, MIT | 20+ crate workspace: evaluator, isomorphism, k-means abstraction, external-sampling MCCFR, depth-limited subgame solving, HTTP backend. | Active (900+ commits). | MIT means we can depend on its crates. Best source for the *preflop / full-game* half and for a fast evaluator. |
| noambrown/poker_solver | Python + C++, MIT | River subgame solver with CFR, CFR+, MCCFR, DCFR, fictitious play; Kuhn and Leduc included. | Reference code. | The cleanest small implementations of each CFR variant to test ours against. |
| jeet-dekivadia/cfr-edge | C++ | HUNL via MCCFR with 169-class preflop and strength buckets. | Small. | Example of the bucketing approach. |
| OpenSpiel | C++/Python, Apache-2 | General game framework with CFR variants and a `universal_poker` ACPC game. | Active (Google DeepMind). | Useful as an oracle for Kuhn and Leduc test cases. Too general to build the product on. |
| OMPEval, PokerHandEvaluator | C++ / C, MIT-ish | Hand evaluators and equity calculators. | Stable. | Equity calculator for the trainer if we do not use robopoker's. |
| Neller and Lanctot, *An Introduction to CFR* (2013) | PDF | The tutorial with working Kuhn poker code. | | Read first. Phase 0 is its exercises. |
| aipokertutorial.com | Web | Plain-language walkthrough of abstraction, bucketing, and translation. | | Read second. |

Licensing conclusion: build our own core, MIT/Apache-compatible, and read the AGPL
solvers only to check our numbers. Depend on robopoker crates freely.

## 7. Recommended architecture

```
gto-solver-app/
├── crates/
│   ├── cards/        # card types, deck, combos, range parser, hand evaluator wrapper
│   ├── tree/         # bet-size config to game tree; action abstraction; translation
│   ├── solver/       # DCFR over the tree, isomorphism, compressed storage, best response
│   ├── report/       # strategy/EV output format, JSON + binary spot files
│   └── wasm/         # thin wasm-bindgen wrapper so the browser can call solver
├── app/              # Tauri 2 desktop shell, React + TypeScript UI
│   ├── src/          # range grid, tree builder, solution viewer, trainer screens
│   └── src-tauri/    # Rust commands that call the crates, run solves off the UI thread
├── spots/            # pre-solved spot library for the trainer (generated, gitignored if big)
├── docs/             # this folder
└── tests/            # Kuhn/Leduc known solutions; cross-checks against wasm-postflop
```

Why this shape:

* Rust for the core because the inner loop is vector arithmetic over ranges; the two
  fastest open-source solvers are Rust and C++, and Rust gives WASM for free.
* Tauri 2 rather than Electron: 5 MB installers instead of 150 MB, native WebView, Rust
  backend in the same process, and it also targets iOS/Android from the same codebase if
  the trainer ever goes to a phone. The known cost is WebView rendering differences
  across OSes.
* One solved-spot format shared by the desktop app, the WASM build, and the trainer, so
  a spot solved on the desktop can be drilled anywhere.

## 8. Build order

Each phase ends with something that runs and is tested.

0. **CFR on Kuhn and Leduc** (Rust). Vanilla CFR, CFR+, DCFR behind one trait. Assert
   the Kuhn equilibrium (player 1 game value of minus one eighteenth) and match
   OpenSpiel or noambrown/poker_solver on Leduc. This is where the algorithm gets
   debugged, on a tree small enough to print.
1. **Cards, ranges, evaluator.** Range text parser, 13x13 mapping, card removal, 7-card
   ranking, sorted-sweep showdown against a range. Property-test against a brute-force
   evaluator.
2. **River solver.** One board, two ranges, a bet-size menu. DCFR with vector terminals.
   Best-response exploitability. Cross-check frequencies against wasm-postflop (free,
   runs in a browser) on identical inputs.
3. **Turn + river**, then **flop**. Chance nodes, suit isomorphism, parallel over
   runouts, 16-bit compression. Memory budget reporting before a solve starts.
4. **Desktop app.** Tauri shell, range editor, tree builder, solve with progress, solution
   browser (grid, per-action EV, filters). Design skills in `.claude/skills/` apply here.
5. **Trainer.** Spot library format, drill mode with EV-loss grading, play-vs-solver with
   off-tree translation, leak tracking.
6. **Preflop.** Chart-based drills first (no solving). Real preflop solving via MCCFR
   plus bucketed postflop as a stretch goal, leaning on robopoker.

## 9. Open questions for Caleb

These change the design and should be answered before Phase 2.

1. Format: 6-max cash, heads-up, or tournaments (ICM changes the payoffs)? Default
   assumed: 6-max cash, 100bb, heads-up postflop trees.
2. Desktop only, or must the trainer also run in a browser or phone? Default assumed:
   desktop first, WASM kept buildable.
3. Personal tool or something to sell? Affects the licence decision in section 6.
   Default assumed: personal, but built MIT-clean so selling stays possible.
4. Rake: cash games with rake shift bet frequencies; do we model it? Default: rake
   config supported in the tree, off by default.
5. How much RAM is on the target machine? Sets the largest tree we design for.

## Sources

* Neller and Lanctot, An Introduction to Counterfactual Regret Minimization: https://modelai.gettysburg.edu/2013/cfr/cfr.pdf
* Johanson, Waugh, Bowling, Zinkevich, Accelerating Best Response Calculation in Large Extensive Games (IJCAI 2011): https://johanson.ca/publications/poker/2011-ijcai-abr/2011-ijcai-abr.pdf
* Waugh, A Fast and Optimal Hand Isomorphism Algorithm (2013): https://www.cs.cmu.edu/~kwaugh/publications/isomorphism13.pdf
* Brown and Sandholm, Solving Imperfect-Information Games via Discounted Regret Minimization: https://arxiv.org/abs/1809.04040
* Bowling et al., Heads-up Limit Hold'em Poker Is Solved: https://poker.cs.ualberta.ca/publications/heads-up_limit_poker_is_solved.acm2017.pdf
* Tammelin et al., Solving Heads-up Limit Texas Hold'em (CFR+): https://poker.cs.ualberta.ca/publications/2015-ijcai-cfrplus.pdf
* Gilpin and Sandholm, Lossless abstraction of imperfect information games: https://dl.acm.org/doi/10.1145/1284320.1284324
* AI Poker Tutorial, Game Abstractions: https://aipokertutorial.com/game-abstractions/
* GTO Wizard, Poker subsets and abstractions: https://blog.gtowizard.com/poker-subsets-and-abstractions/
* GTO Wizard, All you need to know about our solutions: https://blog.gtowizard.com/all-you-need-to-know-about-our-solutions/
* GTO Wizard, Accuracy and benchmarks: https://help.gtowizard.com/accuracy-and-benchmarks/
* b-inary/postflop-solver: https://github.com/b-inary/postflop-solver
* b-inary/wasm-postflop: https://github.com/b-inary/wasm-postflop
* b-inary/desktop-postflop: https://github.com/b-inary/desktop-postflop
* jiyee/GTO-Solva: https://github.com/jiyee/GTO-Solva
* bupticybee/TexasSolver: https://github.com/bupticybee/TexasSolver
* krukah/robopoker: https://github.com/krukah/robopoker
* noambrown/poker_solver: https://github.com/noambrown/poker_solver
* jeet-dekivadia/cfr-edge: https://github.com/jeet-dekivadia/cfr-edge
* rggibson/open-pure-cfr: https://github.com/rggibson/open-pure-cfr
* zekyll/OMPEval: https://github.com/zekyll/OMPEval
* HenryRLee/PokerHandEvaluator: https://github.com/HenryRLee/PokerHandEvaluator
* tansey/pycfr: https://github.com/tansey/pycfr
* Pokerfuse, Poker Solvers: A Beginner's Guide: https://pokerfuse.com/learn-poker/tools/poker-solvers/
* GTO Gecko, Best GTO trainer apps 2026: https://gtogecko.com/blog/best-gto-apps-platforms
* Tauri v2 vs Electron 2026: https://www.buildmvpfast.com/blog/tauri-v2-vs-electron-desktop-apps-2026
* Tauri in 2026 (DEV): https://dev.to/ottoaria/tauri-in-2026-build-cross-platform-desktop-apps-with-web-technologies-better-than-electron-11mo
