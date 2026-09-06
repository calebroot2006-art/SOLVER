---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-06
---

# Cards, ranges, evaluation, and terminal sweep

Research: `docs/research/solver-algorithms.md` sections 4–5 and
`docs/research/open-source-libraries.md`. The core agent supplied a read-only plan;
Astra owns the implementation decisions, integration, and final review.

## Progress

**Accepted, 2026-09-06:** all five jobs pass at `620ea85` in run `34041359029`.
Astra inspected both numerical logs and the passing native runtime evidence.
All evaluator, range, terminal, exact-arithmetic and prior solver gates pass on
both platforms. See `docs/reviews/2026-09-06-astra-phase-2-acceptance.md` for
timings, storage, closed findings, review scope and limits. Continue with
`docs/astra/phase-3/PLAN.md` under Caleb's renewed takeover authorization.

**Takeover resumed, 2026-09-06:** Caleb authorized full development takeover with
the 10% usage reserve. The live account counter reads 39% remaining; stop work at
12% and save the handoff. Astra owns integration and final verification. One
read-only helper reviews terminal arithmetic and coverage; no executor is editing.
The saved fixes are pushed in run `34041167846`. Both prior platform logs confirm
the same whitespace failure. Apply the reviewed formatter artifact for `ed73f38`,
then require all five CI jobs to pass before accepting phase 2.
The helper found no demonstrated terminal defect and reproduced all 1000 Fraction
fixtures. Add their Python regeneration check to both solver CI jobs alongside
the existing Rust replay, so fixture reproducibility remains enforced.

**Save cutoff, 2026-09-06:** Caleb stopped this session with three minutes to save.
Implementation is integrated; acceptance is pending. Linux passed exhaustive
five-card and ten-million seven-card checks, but a range whitespace test failed.
The parser correction and new exact-arithmetic fixtures need CI. Hosted formatting
is partly applied; see `docs/reviews/2026-09-06-astra-phase-2-checkpoint.md`.

The phase 0/1 foundation is verified. Fable's reference cleanup passes all local
and hosted checks at `fc8fd1a`; all five CI jobs passed in run `34019066360`.
The checked card/range and terminal implementations await integrated CI.
The evaluator adapter and independent exhaustive/random gates are being integrated.

## Task

Deliver standard 52-card hold'em cards, combos, weighted ranges, checked five/seven
card evaluation, and a showdown/fold sweep independently validated against brute
force. Keep all existing toy-game gates. Full hold'em tree integration is phase 3.

## Approach and ownership

- Astra: root integration branch, this plan, evaluator adapter and independent
  five-card oracle, exhaustive/random validation, benchmarks, CI, dependency
  review, crate manifests/locks, documentation, and final acceptance.
- Card/range executor: isolated worktree; `crates/cards/src/{lib,card,combo,range,error}.rs`,
  `crates/cards/tests/{cards,ranges}.rs`, and `crates/cards/README.md` only.
- Terminal executor: separate isolated worktree; `crates/postflop/src/terminal/**`
  and `crates/postflop/tests/terminal_sweep.rs` only. Root adds module/dependency wiring.
- Dependency helper: exact published source/API/license inspection read-only,
  with archives and evidence only in ignored `target/astra-temp/evaluator-audit/`.

No executor runs Rust locally, changes shared manifests, pushes, or delegates again.
Root integrates reviewed commits and reads GitHub Actions results. Account usage is
polled between work steps, with a stop at 12% remaining and a pause on counter failure.

## Checked card and range contract

`Rank` and `Suit` are enums ordered `Two..Ace` and `Clubs, Diamonds, Hearts, Spades`.
Card IDs are `4 * rank_index + suit_index`, both zero-based. `Card` supports
`new(Rank, Suit)`, checked `from_id(u8)`, `id()`, `rank()`, `suit()`, `mask()`,
`all()`, `Display`, and `FromStr`. Text is exactly one uppercase rank or digit
followed by a lowercase suit. Invalid input reports `CardError`.

`CardSet::new(&[Card]) -> Result<CardSet, CardError>` rejects duplicates.
`bits() -> u64`, `contains(Card) -> bool`, `len() -> usize`, and `is_empty()`
expose the checked set. Unchecked external bitmasks are not accepted.

`Combo::new(Card, Card)` rejects duplicate cards and orders IDs `a < b`.
`Combo::from_id(u16)` checks the bijection `id = b*(b-1)/2 + a` over 1326 slots.
Expose `id() -> u16`, `cards() -> [Card;2]`, `mask()`, `all()`, `Display`,
`FromStr`, and `grid_cell() -> (usize, usize)`. Canonical text prints the higher
card ID first; input accepts either order. Do not assume an external solver's
1326-entry order; a future UPI adapter must validate an explicit permutation.

`Range` stores `[f64;1326]` finite inclusion weights in `[0,1]` without normalization.
Provide `empty`, checked `from_weights`, `weights`, `weight(Combo)`, checked
`set_weight(Combo,f64)`, `without_cards(CardSet)`, `parse`, `FromStr`, `Display`,
and `to_canonical_string`. `combos_for_cell(row,col)` returns checked grid members:
descending ranks, pairs on the diagonal, suited above, offsuit below. Class sizes
are 6/4/12; arbitrary combo weights remain distinct within a cell. Empty ranges
are valid data; the future solve builder rejects an empty compatible joint range.

Support the documented Pio-style subset: `AA`, `AKs`, `AKo`, `AK`, explicit `AsKh`,
pair `99+`, fixed-high `ATs+`, pair intervals, fixed-high intervals, and constant-gap
intervals such as `65s-T9s`, including reversed endpoints. Nonpair interval suffixes
must match. A `:weight` suffix applies to the expanded expression. Commas and ASCII
whitespace separate tokens. Reject empty comma items, pair suffixes, percentages,
mixed interval shapes, explicit-combo intervals, and invalid/nonfinite weights.
Equal overlapping assignments are idempotent; conflicting assignments error with
token and combo context. Explicit `set_weight` supports later editor overrides.

Bound input to 131072 bytes and 4096 tokens before expansion. Canonical output lists
nonzero explicit combos in ID order, with roundtrippable fractional/scientific
weights. Test tiny positive weights so canonical output fits the parser limits.
Roundtrip means identical weights, not identical shorthand or whitespace.
This is not a claim of complete PioViewer shorthand compatibility.
[Documented UPI hand ordering](https://piosolver.com/docs/upi/commands/).

## Evaluator contract and selection

Production candidate: `rs_poker = "=5.1.0"`, defaults disabled.
The exact-source audit rejects `deuce = "=1.1.0"` before benchmarking: its default
ordering puts flush above full house and its flush strength discards four kickers.
These are static source findings; no deuce runtime result is claimed.
Benchmark checked rs_poker evaluation and cached-board evaluation, recording
platform, build flags, table size, and elapsed time. Adoption still requires the
independent correctness gates and a review of the resolved dependency graph.

Expose opaque `HandValue: Copy + Eq + Ord + Debug` (larger is stronger), checked
`evaluate_five([Card;5])`, `evaluate_seven([Card;7])`, and
`evaluate_holdem([Card;5], Combo)`, all returning `Result<HandValue, CardError>`.
Validate uniqueness before entering vendor code. Keep vendor encodings private.
Expose `RiverEvaluator::new([Card;5])` and `evaluate(Combo)` with the same checks,
so showdown tables can reuse board work without bypassing blockers.

## Terminal contract

`ShowdownTable::new([Card;5]) -> Result<Self, TerminalError>` caches ordered
rank groups and original combo IDs. `ShowdownScratch::default()` holds reusable
linear buffers. `OutcomeUtilities::new(win,tie,loss)` accepts only finite values.

`ShowdownTable::evaluate(&self, &[f64;1326], OutcomeUtilities, &mut [f64;1326],
&mut ShowdownScratch) -> Result<(), TerminalError>` computes values weighted
only by compatible opponent reach. Reject negative/nonfinite reach, arithmetic
overflow, and unresolved numerical errors. Outputs stay unchanged on failure.
Board-blocked hero slots are zero. Do not multiply hero weight, chance, or range
again, and do not normalize at a terminal.

For each weaker/equal/stronger bucket, compatible mass is total minus the mass
containing each hero card, plus the exact hero-combo mass. The add-back belongs
only in the equal bucket. Use ascending and descending strict sweeps and equal-rank
groups so chops are explicit and stronger mass need not subtract near-equal totals.
Document floating-point allowances and test near-cancellation with sparse reach.
Return `win*weaker + tie*equal + loss*stronger` using checked finite arithmetic.
`evaluate_fold(CardSet, &[f64;1326], finite_utility, &mut [f64;1326])` applies the
same compatible-mass rule on any checked dead-card set. No dense payoff matrix.

The phase 1 `Game` binding snapshots two dense payoff kernels: 28,132,416 bytes
per full 1326-combo terminal, about 26.8 MiB. Keep its toy-game audit intact.
A scalable immutable binding needs review before phase 3 connects these terminals.

## Steps and acceptance

1. Implement/exhaust card and combo IDs, grid counts, masks, removals, strict
   parser errors, and arbitrary weighted semantic roundtrips.
2. Audit and compare evaluator candidates. Build an independent five-card oracle
   from rank counts, flush/straight detection (including the wheel), and ordered
   category/kicker tuples. No vendor evaluator in the oracle.
3. Exhaust all 2598960 five-card hands. Require category totals and 7462 distinct
   strength classes, ties, and ordering to agree with the production backend.
4. Stream 100000 deterministic seven-card samples as a CI timing pilot, then make
   10000000 mandatory on each platform. For each, independently classify all 21
   five-card subsets and compare their maximum. Do not hide the gate with ignore.
5. Compare the terminal sweep to a separate O(n²) pairwise oracle on full/sparse
   random ranges, varied boards, board-playing chops, exact overlap, fractional
   weights, zero reach, both player orientations, and unequal contributions.
   Check linearity and joint-weighted zero-sum utilities; record time and storage.
6. Run all workspace formatting, lint, tests, reference gates, and affected frontend
   checks. Inspect exact CI results, document limits, and save the phase review.

## Decisions and risks

No new product choice is needed for standard hold'em or the agreed backend benchmark.
Short deck, Omaha, rake, tournament payoffs, suit merging, and playable UI are later
work. The 10-million sample gate performs 210 million independent five-card
classifications; measure runtime before sharding. Stream data instead of collecting
millions of hands. The 16 GB consumer target remains binding for shipped solves.
