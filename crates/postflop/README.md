# Postflop CFR core

Phase 1 implements alternating vanilla CFR, CFR+ with linear averaging, and
DCFR(1.5, 0, 2). The public API returns checked strategies, net chip EV, best
responses, and an explicit stopping reason. Hold'em trees and performance work
follow the toy-game accuracy gate.

Phase 2 adds standalone checked hold'em showdown and fold evaluation in
[`terminal`](src/terminal/mod.rs). These functions are not yet wired into a
hold'em game tree. The legacy `Game` validation still uses dense toy-game kernels.

## Hold'em terminal values

`ShowdownTable::new` validates a river board and sorts its 1081 live hole combos
into equal-strength groups. Reuse that table and a `ShowdownScratch` across
traversals. Inputs are all 1326 opponent reaches in `cards::Combo::id` order and
finite win/tie/loss utilities. The result is unnormalized counterfactual value:
opponent range, action, and chance reach must already be included exactly once.
Hero weights are not applied. Board-blocked outputs are zero.

For each weaker/equal/stronger bucket, compatible mass is
`total + same_combo - card_a - card_b`. Only the equal bucket includes the same
combo. Ascending and descending sweeps keep strict outcomes separate from ties.
`evaluate_fold` applies the same compatibility calculation to any checked dead set.

Each nonnegative finite f64 is stored exactly as an integer in units of `2^-1074`.
The largest input has highest set bit 2097; at most 1327 such contributions need
2109 bits. Each accumulator reserves 34 u64 limbs, or 2176 bits. Total and 52 card
bins preserve tiny compatible mass even when blocked mass exceeds f64 capacity.
Add-back occurs before subtraction, so exact intermediates stay nonnegative.
Each compatible mass converts once, rounding to nearest with ties to even.

Each mass-times-utility product rounds as f64. Their signed sum is then exact
before its final rounding. Nonfinite or negative reach, nonfinite utilities,
overflow, and nonzero products rounded to zero return an error. Caller output
stays unchanged on failure; scratch may change and remains reusable. This checked
policy can reject extreme values even when a rescaled calculation would be finite.
No normalization, clamping, or silent quadratic fallback repairs those inputs.

The table and scratch use linear storage, with no pairwise payoff matrix or
allocation per showdown traversal. Tests compare all outputs against independent
pairwise enumeration and cover ties, blockers, tiny residuals, folds, linearity,
unequal contributions, joint-weighted zero sum, and atomic failures. Run
`cargo test -p postflop --locked -- --nocapture` to include storage and timing output.

## Numerical contract

`Game` is an immutable tree of public histories. An information set is a public
node plus the acting player's own private-state index. Rows are indexed by public
node, with private states first and contiguous actions inside each state. Chance
and terminal nodes have empty strategy rows.

Initial private weights factor as `w0[h0] * w1[h1]`; compatibility removes card
conflicts. Values divide by the surviving joint mass Z. The representation does
not support correlated ranges, bunching, or multiway games.

The traversal passes opponent action reach, opponent initial weights, public
chance probabilities, and opponent masks into terminal evaluation. It applies
own masks separately to the output. Chance is multiplied exactly once. At every
chance node, validation checks that probabilities times both masks sum to one for
each compatible pair still legal after ancestor masks. Own reach for strategy
averaging contains only own action probabilities.

All storage and accumulation are f64. For M state-action entries, regrets,
strategy sums, and the current strategy occupy 24M bytes, before vectors and the
tree. Reading an average adds 8M bytes. The validated layout is shared through an
Arc and includes a private-pair compatibility matrix. This is a phase 1 baseline;
phase 2 must measure allocation cost before expanding to full hold'em ranges.

Each terminal also stores both players' payoff kernels as f64 bit patterns,
requiring 16 * H0 * H1 bytes per terminal. Construction and each checked reuse
evaluate H0 + H1 unit opponent vectors per terminal. This binds utilities as well
as geometry, catching a changed payoff with the same public tree. The cost is
deliberate for the three-state and six-state toy games and must be profiled before
the full hold'em implementation.

Vanilla and DCFR retain signed cumulative regrets. Regret matching uses only the
positive part when creating a strategy. CFR+ alone floors stored regrets after a
player's complete update. Player zero updates first; player one then faces that
updated policy. Averaging uses the policy played before each player's update.

DCFR discounts the entire accumulators after adding each iteration's contribution:
positive regrets by t^alpha/(t^alpha+1), negative regrets by t^beta/(t^beta+1), and
strategy sums by (t/(t+1))^gamma. The implementation uses an equivalent reciprocal
form for regret factors to avoid infinity divided by infinity. The unit trace
adds [1,0], [0,1], [0,1] over three iterations: the first average probability is
1/14. Discounting only each new contribution would incorrectly give 36/181.
A second trace checks that beta=0 changes negative regret -8 to -4, -2, and -1.

The update and linear-average comparison is based on
[OpenSpiel CFR](https://github.com/google-deepmind/open_spiel/blob/master/open_spiel/python/algorithms/cfr.py).
Algorithm background is the
[DCFR paper](https://arxiv.org/abs/1809.04040).
Our code was written for this public-vector contract; no AGPL source was copied.

## Accuracy and failure behavior

For two-player zero-sum chip utilities:

- br[i] is player i's maximum net chips against the fixed opposing policy.
- nash_conv = br[0] + br[1], in chips per hand.
- average = nash_conv / 2, in chips per hand.
- pct_of_pot = 100 * average / starting_pot.

Uniform Kuhn has best responses [1/2, 5/12], NashConv 11/12, average 11/24,
and 22.9166666667 percent of the two-chip root pot. Checked conversion methods
make raw chip targets and percentage targets explicit.

Metric conversions reject overflow and any positive value that underflows to
zero. A tiny positive residual cannot become an exact zero target by conversion.
Zero-sum and negative-NashConv checks use the same 1e-10 allowance relative to
1 + |u0| + |u1|, evaluated after scaling to avoid overflowing that denominator.

These values certify the supplied tree, ranges, and payoff model only. They are
not unrestricted no-limit accuracy or a per-decision uncertainty bound.
Construction checks finite kernel entries for zero-sum utility on every compatible
terminal deal still possible under ancestor masks. `exploitability` also rejects
non-zero-sum profile EV. These checks rely on the documented linear terminal hook;
unit-vector tests cannot prove arbitrary trait code is linear. Game implementers
must provide deterministic, immutable, linear evaluation. NaN bit patterns may be
stored as failure sentinels but are never accepted as valid strategy evidence.

Construction rejects malformed node indices, shared children, cycles, empty
ranges, invalid probabilities, and invalid pots. Recursive traversal has an
explicit phase 1 depth limit of 256. Strategy construction validates every row.
Updates and metric reads check the game's structural, probability, and terminal
kernel binding. Terminal behavior must remain immutable under the Game contract.

A numerical failure poisons Cfr: further iterations, current-policy reads, and
average-policy reads return the failure. Every terminal output begins as NaN, so
a terminal that fails to write a state cannot silently return zero. Errors name
the attempted iteration, public node, and player. Invalid configuration names the
field. Unknown and missing TOML fields are errors.

## Running and checking

From the workspace:

```text
cargo test -p payoff -p postflop --locked
cargo test -p toygames --locked -- --nocapture
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Load `config/solver.toml` with `SolverConfig::load`, construct
`Cfr::new(game, config.dcfr.variant())`, then call
`solve(game, &mut cfr, &config.solve, callback)`. Both current and average strategy
accessors return a Result. The driver reports TargetReached or IterationCap;
the latter is never described as convergence.

Phase 1 runs serially. threads=0 selects this available implementation, threads=1
requests it explicitly, and larger values are rejected. Progress checks occur
between complete iterations, so a single slow iteration may exceed
log_every_secs. Configuring that interval does not start a background thread.

Crate unit tests pin discounting, signed storage, and invalid configuration.
The separate toygames package owns independent history-oracle, Kuhn/Leduc,
OpenSpiel curve, and fixed-budget convergence checks. Runtime results belong in
the integration review: local Rust compilation remains unavailable under Smart
App Control, and root runs the approved GitHub Actions checks.
