---
type: research
status: draft
date: 2026-09-05
---

# Solver algorithms: the exact techniques

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.
Deepens sections 2 and 3 of `how-to-build-a-solver.md`.

## Question

What exact algorithms and engineering techniques does a from-scratch heads-up no-limit
postflop solver need to reach PioSOLVER-class accuracy and speed, and what are the known
pitfalls?

## Answer

Discounted CFR with α=1.5, β=0, γ=2 is confirmed from the primary paper and is still the
practical default. Newer variants (PCFR+, PDCFR+, DDCFR) exist but none has a mature
poker implementation found in this research pass. Johanson et al.'s 2011 technique
reduces terminal range-versus-range evaluation from O(n²) to O(n log n): sort hands
by strength, sweep running win, tie, and lose totals, and correct card overlap with
inclusion-exclusion. Best response still traverses the relevant public tree; the
terminal cost is not the complexity of solving or evaluating an entire no-limit game.
b-inary's postflop-solver source confirms 16-bit compressed storage, alternating
updates, signed regret storage with sign-dependent discounting, and a strategy-sum reset
at powers of 4. PioSOLVER's docs give the only credible tree-size RAM numbers found.

Corrected 2026-09-05 after Astra's review (findings R01, R02, R05, R07 in
`docs/reviews/2026-09-05-astra-research-and-program-plan-findings.md`): regret storage
and averaging are now stated exactly, the best-response benchmark is labelled as
heads-up *limit* hold'em, the all-in and isomorphism guidance is qualified, and the RAM
"triples" claim is removed.

## Findings

### 1. DCFR update rules, confirmed from the paper

Source for all of this section: Brown and Sandholm 2018, arXiv 1809.04040, pages 3 to 4.
Local copy: `C:\Users\Caleb\.claude\projects\c--Users-Caleb-Documents-GitHub-GTO-Solver-APP\013d1c4a-a52b-40da-9e2f-48babdb28e92\tool-results\webfetch-1788655471435-sngv8u.pdf`

* Each iteration t, after that iteration's contributions have been added: multiply the
  **whole accumulated** positive regrets by t^α/(t^α+1), the whole accumulated negative
  regrets by t^β/(t^β+1), and the whole accumulated strategy sum by (t/(t+1))^γ.
  Recommended default **α=3/2, β=0, γ=2**, which "matched or outperformed CFR+ in all
  settings" tested. Cumulative regrets stay **signed** in storage; the current strategy
  is regret matching over their positive part. The discount applies to the accumulator,
  not to the newly added contribution only: on a three-iteration example the two
  readings give first-action probabilities of 1/14 and 36/181 (Astra's `checks.py`).
* β=0 makes a suboptimal action's regret approach a constant rather than negative
  infinity, which breaks regret-based pruning. Use β=1/2 if pruning is planned.
* Linear CFR is DCFR(1,1,1), implemented as multiplying accumulated regret by t/(t+1)
  each iteration "to reduce the risk of numerical instability" rather than storing the
  weighted sum directly.
* CFR+ is DCFR(∞, −∞, 2): regret floored at zero and average-strategy weight t².
* Optimistic variants (count the last iteration's regret twice) helped in only 2 of 4
  HUNL subgames, and "Optimistic DCFR(3/2,0,2) did worse than DCFR(3/2,0,2) in all HUNL
  subgames." Do not stack optimism on DCFR.
* PCFR+ and Stable-Predictive CFR (Farina et al. 2019, arXiv 1902.04982) and PDCFR+
  (IJCAI 2024, https://www.ijcai.org/proceedings/2024/0583.pdf) converge faster on
  non-poker benchmarks but "PCFR+ assigns uniform weights, leading to substantial regrets
  when facing dominated actions", a weakness for poker trees. Secondary summaries only.
* DDCFR (ICLR 2024, learned discounting, code at https://github.com/rpSebastian/DDCFR,
  MIT, 4 commits) claims to beat fixed DCFR. No evidence of adoption in any shipped
  solver. Research lead, not a build requirement.

### 2. Alternating updates, regret-matching+, averaging and resets

* "In practice far better performance is achieved by alternating which player updates
  their regrets on each iteration", at the cost of complicating the convergence theory
  (Burch, Moravcik, Schmid 2018, arXiv 1810.11542). Source: the DCFR paper.
* Plain regret matching (used by vanilla CFR and DCFR): store the signed cumulative
  regret R^T(a) = R^(T−1)(a) + r^t(a) and play σ(a) ∝ max(R(a), 0). Regret-matching+
  (used by CFR+) instead floors the *stored* cumulative regret at zero each iteration:
  Q^T(a) = max(0, Q^(T−1)(a) + r^t(a)). The two differ in what is stored, and DCFR with
  β=0 depends on keeping the negative part (it decays by a factor of 1/2 per iteration
  rather than being discarded). Same source.
* b-inary's `src/solver.rs` (AGPL, read only) confirms: γ = 3.0, alternates over the two
  players, keeps signed regrets and discounts them by sign, uses max(r, 0) only when
  forming the strategy, and resets the accumulated strategy weighting at each power of 4
  (breakpoints 0, 1, 4, 16, 64, 256, ...) while regret discounting uses its own iteration
  expression. When the regret-sum denominator is near zero it falls back to a uniform
  strategy instead of dividing by zero. This is a variant of the paper's DCFR, not the
  default. Source: https://github.com/b-inary/postflop-solver/blob/main/src/solver.rs
* Averaging weight for CFR+: OpenSpiel's implementation weights iteration t's
  contribution linearly by t; the DCFR paper frames CFR+ as the γ=2 (quadratic) case.
  Both converge; they are different averages. Our implementation uses linear so its
  curve can be compared with OpenSpiel point for point.

### 3. Best response and exploitability

Source: Johanson, Waugh, Bowling, Zinkevich, *Accelerating Best Response Calculation in
Large Extensive Games*, IJCAI 2011, https://johanson.ca/publications/poker/2011-ijcai-abr/2011-ijcai-abr.pdf
Local copy: `...\tool-results\webfetch-1788656484571-njn9a2.pdf` (same folder as above).

* Exploitability for a two-player zero-sum game with alternating positions:
  ε(σ) = [u₂(σ₁, b₂(σ₁)) + u₁(b₁(σ₂), σ₂)] / 2, the average of the two positions'
  best-response gains (their equation 3).
* The paper reports it in milliblinds per game. Our "percent of pot" is the same idea
  with a different denominator. Pick one and be consistent.
* Efficient computation walks a **public-state tree** so opponent reach-probability
  vectors are computed once per public node and reused across all 1,326 combos, "an
  estimated 110x speedup" over per-information-set queries.
* Scale reference: a full-game best response for heads-up **limit** hold'em took about
  76 CPU-days, parallelisable to about a day on 72 cores. Competition bots measured 135
  to 422 mb/g against a benchmark of 50 mb/g for a strong human. This is a limit hold'em
  number; it says nothing about the cost for no-limit.
* **What our exploitability measures.** A postflop solve enumerates cards exactly but
  restricts bets to a menu and fixes both ranges and the root history. Its exploitability
  is therefore the exploitability *within that betting tree and those ranges*, not
  unrestricted no-limit exploitability. Every stored solve carries its tree, ranges,
  root history, payoff model, metric units, residual, and stop reason. A root residual is
  also not a per-decision error bar: a one-chip error in a branch reached with
  probability 0.001 contributes only 0.001 chips at the root, so grading a rarely reached
  decision needs its own uncertainty policy (Astra R02).

### 4. Terminal evaluation: sorted sweep with blocker correction

Same Johanson et al. paper, "Efficient Terminal Node Evaluation" and Example 3.

* Sort each range by hand rank. Sweep the acting player's hands weakest to strongest
  while maintaining running indices into the opponent's sorted list marking the worse,
  equal, and better boundaries, so the opponent's reach-weighted totals update
  incrementally. O(n) after the O(n log n) sort.
* Blocker correction, quoted: "Using the inclusion-exclusion principle, when computing
  the total probability of hands better and worse than ours, we subtract the total
  probability of opponent hands that include either of our cards. The opponent hand that
  uses both of our cards has then been incorrectly subtracted twice, so we correct by
  adding its probability back again."
* Measured: "this O(n log n) evaluation runs 7.7 times faster than the straightforward
  O(n²) evaluation" in Texas hold'em.

### 5. Suit isomorphism and hand indexing

* Waugh, *A Fast and Optimal Hand Isomorphism Algorithm*, AAAI Workshop on Computer
  Poker 2013, https://www.cs.cmu.edu/~kwaugh/publications/isomorphism13.pdf gives an
  optimal canonical index and its inverse. Existence and scope corroborated by three
  sources; the PDF was not read this session, so internals are unverified.
* Reference implementations: C at https://github.com/kdub0/hand-isomorphism, Java at
  botm/hand-isomorphism, a Rust wrapper at https://github.com/cleverpiggy/poker-hand-indexer.
* Payoff, from Johanson et al.: suit isomorphism makes the hold'em public state tree
  "21.5 times smaller than the full game", and there are exactly 1,755 canonical flops.
* **Precondition.** Merging suits is exact only when the symmetry preserves the board,
  *both weighted ranges*, and the rules. b-inary's card code checks range symmetry
  before merging suit outcomes. A range like "spade flush draws only" breaks the
  symmetry and the merge must be skipped for that suit pair (Astra R05).
  Source: https://github.com/b-inary/postflop-solver/blob/main/src/card.rs

### 6. Memory layout and multithreading

* b-inary README, exact wording: "each game node stores the values by 16-bit integers
  with a single 32-bit floating-point scaling factor." Multithreading via rayon.
  Source: https://github.com/b-inary/postflop-solver/blob/main/README.md
* No vendor publishes a bytes-per-node formula or the precision loss from 16-bit
  regrets. Our own formula (nodes x combos x actions x bytes) is our construction. Derive
  and benchmark the error bound in-house when the storage layer exists.
* Parallelism across chance nodes is formally justified by Johanson et al.'s "Parallel
  Computation" section: public-state subtrees where neither is an ancestor of the other
  share no computation and can be solved in parallel.

### 7. Tree size and RAM

Source: https://piosolver.com/docs/technical_details/ and https://piosolver.com/docs/faq/hardware/

* Single-bet-size trees: 500 MB to 4 GB. A full single-raised pot with two-thirds pot
  bets everywhere is about 1.2 GB (wide 6-max ranges) or 1.9 GB (wide heads-up).
* Pio's two-size examples: a 25bb heads-up tree with 30 and 60 percent needs 5.9 GB; a
  100bb 6-max tree with two sizes needs 7.8 GB. These are Pio's own trees and storage
  with their particular ranges and stacks; they do not isolate the effect of adding one
  size, so no general multiplier follows from them. Our Phase 4 gate measures peak
  memory for named trees, including working buffers and desktop overhead, against the
  16 GB target (Astra R07).
* Guidance: 8 GB is enough for occasional 2 to 3 bet sizes, 16 GB for very big trees,
  preflop needs 64 GB minimum and 128 GB recommended.
* No node counts published, only RAM.

### 8. Known pitfalls

* **Numerical instability from weighted sums.** Rescale in place each iteration (the
  DCFR paper's own reason for its Linear CFR implementation) rather than accumulating
  t-weighted sums that grow without bound.
* **Ties inside the sweep.** Johanson et al. track three buckets (weaker, equal, better).
  A strict total order silently misprices chopped pots.
* **Float floor on exploitability.** Even a correct best response has a floating-point
  error floor; do not report precision below it.
* **Zero-reach NaN.** Confirmed as a handled case in b-inary's code: uniform strategy
  when the denominator is near zero.
* **All-in handling.** Betting on a street ends only after every outstanding response is
  resolved and no further betting is possible. A bet that puts one player all-in still
  leaves the opponent a call-or-fold decision; heads-up, once both are all-in or one is
  all-in and called, remaining streets deal to showdown with no more decision nodes.
  Multiway, players with chips behind keep betting into a side pot after one player is
  all-in. Getting this wrong silently deletes decisions (Astra R05). PokerKit's
  simulation docs have all-in and side-pot examples suitable as fixtures:
  https://pokerkit.readthedocs.io/en/stable/simulation.html
* **Precision.** Phase 1 is `f64` throughout. Changing the shared numeric alias changes
  accumulation and best-response precision, not only storage, so `f32` storage and
  16-bit compression are each measured against the `f64` baseline on the same inputs
  before adoption, and the metric's numerical allowance comes from those comparisons
  rather than from a theoretical floor.
* **Abstraction overfitting.** Johanson et al. measured that training CFR to convergence
  in an *abstracted* game can *increase* full-game exploitability over time while
  abstract-game exploitability keeps falling ("a form of overfitting"). This bites the
  moment bucketed preflop solving is added on top of the exact postflop core. Source:
  same paper, "Overfitting" section and figure 6.

## Unverified

* Poker-specific behaviour of DDCFR, PCFR+, and PDCFR+.
* Waugh's indexing internals (PDF not read).
* b-inary's exact bytes per node and the 16-bit error bound.
* PioSOLVER node counts.
* Whether TexasSolver's showdown and best-response code differs from Johanson et al.

## Also worth knowing

* Johanson et al. 2011 is now the primary source for both the terminal sweep and the
  exploitability computation; it replaces the from-memory description in the overview.
* The overview's caveat that DCFR's parameters were "from memory" is removed: confirmed
  from the paper. b-inary's γ=3 and power-of-4 resets are confirmed from source, not
  just the README.
* The abstraction-overfitting result supports measuring the model actually used:
  exact card enumeration inside the chosen betting tree first, then separate
  diagnostics for abstraction and off-tree behavior. Sampled diagnostics do not
  establish unrestricted full-game exploitability.

## What this means for our plan

* Phase 1 implements CFR, CFR+, and DCFR(1.5, 0, 2) behind one trait, with alternating
  updates, signed regret storage, and in-place discounting of the whole accumulators,
  validated on Kuhn and Leduc against known values and captured OpenSpiel curves.
* The best-response module is built on the public-state tree from day one, because the
  solver's own traversal uses the same structure.
* The terminal evaluator is the Johanson three-bucket sweep with inclusion-exclusion,
  property-tested against a brute-force O(n²) evaluator on random ranges and boards.
* Phase 1 uses f64 storage and arithmetic. Later f32 and 16-bit storage changes are
  measured against that baseline, with accumulation and metric precision specified
  separately. Compression is added only after the uncompressed accuracy gates pass.
