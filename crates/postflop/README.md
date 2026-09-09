# Postflop CFR core

Phase 1 implements alternating vanilla CFR, CFR+ with linear averaging, and
DCFR(1.5, 0, 2). The public API returns checked strategies, net chip EV, best
responses, and an explicit stopping reason.

Phase 2 adds standalone checked hold'em showdown and fold evaluation in
[`terminal`](src/terminal/mod.rs). Phase 3 now connects them to an owned river
game and checked betting tree. Its individual reference-frequency explanations are
recorded in the [measured review](../../tests/reference/river/measured/2930550/README.md);
the [phase 3 plan](../../docs/astra/phase-3/PLAN.md) records acceptance at `06dd4f4`,
with all six hosted jobs passing.

## Owned river API

Construct `tree::RiverTree` with explicit pot, effective stack, minimum bet, both
players' size menus, raise cap, all-in thresholds, and node limit. Pass it with a
five-card board, two `cards::Range` values, and a byte limit to `RiverGame::new`.
There are no product betting defaults. This backend covers a heads-up, zero-rake
river starting with out of position to act and no outstanding wager.

`RiverSolver::new(game, variant)` binds CFR permanently to those inputs.
`run_iteration` performs a complete alternating update; `solve` uses the existing
target/cap driver. `solve_with_cancel` checks between full iterations and returns
a fresh measured average, so a cancelled session can resume. A numerical failure
poisons subsequent solver updates and diagnostic or average-policy reads.

`average_strategy` returns a `RiverStrategy` retaining the immutable game.
It exposes per-combo rows, expected net chips, information-set best response,
exploitability, and conditional action EVs at a public node. `from_rows` validates
an imported policy for the exact supplied game. Query methods take no replacement
game. Cloning a `RiverGame` retains its identity; constructing another game creates
a new binding even when its inputs compare equal.

Each range loses board-blocked combos and is divided by its largest live weight.
This preserves the independent-range product conditioned on compatible private
deals. The root normalizer applies once. All 1326 canonical combo slots remain;
zero initial-range and blocked combos have no public policy row. The river path
stores one showdown table and terminal payoff descriptors, with no private-pair
payoff matrix. Shared CFR and best-response equations also serve the audited
legacy toy-game API.

The payoff origin assigns half the root pot to each player's sunk contribution.
Terminal payoffs include future wagers and refunded uncalled chips. Folding at
a history where the actor has committed `c` returns `-starting_pot/2-c` net chips.
`decision_values` follows the policy after each candidate action. It returns no
EV for zero own reach or zero compatible opposing mass. These are conditional
values within the configured game; a root residual is not a per-combo error bound.
The measured reference comparison contains rare histories with materially poor
conditional mixtures despite tiny root influence. Consumers must preserve reach
and game coverage when interpreting these outputs. These captures certify no
coaching grades or decisions at a different starting state.

`memory_usage` gives a conservative working-set estimate. A shared reservation
counter accounts for retained solvers, snapshots, decision reports and concurrent
query workspaces; dropping an object releases its reservation. The counter itself
lives in `src/memory.rs` so every owned game in the crate shares one, not just the
river ones. Imported row capacities are charged as supplied. Large solver buffers
use fallible allocation. Tree construction has its own node limit before it is
passed into the game.
The estimates describe allocations under this API, not process RSS or a machine's
available RAM. Positive mass or value products that round to zero return checked
errors instead of becoming a false zero-exploitability result.

## Owned postflop API, flop through river

`streets::PostflopGame` is the river backend generalised to a tree that changes
street. It takes a `tree::PostflopTree`, a board holding exactly the cards its
start street knows (three, four or five), two ranges, and a `PostflopOptions`
carrying the byte limit, the storage width and the requested worker count. The
tree is compact: one abstract chance node stands for a whole street transition.
The game expands each of those into one child per dealt card, so every distinct
public history has its own node, which is what the traversal contract requires.

The deal. A chance node offers every card not already on the board, and each
outcome carries probability one over the unseen cards less the four private
cards: one forty-fourth on the turn, one forty-fifth and then one forty-fourth on
the flop. Its masks zero the combos holding the dealt card, so the chance mass
over any compatible pair is exactly one. A called all-in before the river is not
a special case: it is chance nodes down to a showdown terminal with no decision
in between, so the same sweep evaluates it.

What is shared and what is not. Each dealt card contributes one mask pair to the
layout's pool, so a turn tree keeps forty-eight pairs rather than one per chance
node and outcome. Showdown tables are interned on the completed board's card set.
Two runout orders that reach the same five cards therefore share one table, so a
turn tree builds forty-eight and a flop tree eleven hundred and seventy-six,
against the two thousand three hundred and fifty-two the estimate charges for.

Expansion is depth first, so the nodes expanded below any node occupy one
contiguous half-open range. `subtree(node)` reports that range and
`outcome_range(chance, outcome)` reports the slice one dealt card owns. Those
slices are non-empty, in outcome order, and together they partition their chance
node's own range less its root, which is the disjointness a parallel traversal
splits accumulators along. The contract holds at every chance level rather than
in one flat list: on a flop tree the turn deal's 49 ranges partition the deal's
subtree, and each turn card's river deal partitions that card's range in turn.

Construction also validates the expanded tree against the whole-tree contracts a
local traversal cannot see. The structural ones cost one pass over the nodes and
run on every game, however large: every node reachable exactly once, no cycles or
shared children, declared child counts against action and outcome counts, chance
probabilities inside [0,1], mask shapes, and terminals without children. Two are
quadratic in the live combos and are size gated: one unit of chance mass per
compatible pair still legal after ancestor masks, and zero-sum terminal
utilities, whose values are read one column at a time through the same terminal
boundary the solve uses.

`validation()` reports what ran. Its `nodes`, `chance_nodes` and `terminals`
always cover the whole tree; its `pairs` and `zero_sum_terminals` are zero when
the tree was above the budget for those two checks, which means they did not run,
never that they passed. The fixtures in `tests/streets.rs` sit inside every
budget, so they exercise all of it; a gate tree gets the structural half and
reports zero for the rest.

`PostflopSolver` and `PostflopStrategy` mirror their river counterparts.
`PostflopNodeView` adds what a multi-street history needs: the street, the board
in deal order, the runout after the root board, the cards a chance node can deal,
and the compact node the history was expanded from. `PostflopDecisionValues`
carries the street, the board and the runout beside the values, along with the
acting player's own reach and the compatible opponent mass, so a river frequency
cannot be quoted without the context that says how often the history happens.

A river-start `PostflopGame` is the load-bearing check on all of this: on the
three phase 3 fixtures it produces the same nodes, the same regrets, the same
strategy sums and the same exploitability as `RiverGame`, bit for bit.

`PostflopOptions::from_config` reads all three settings from a parsed
`config/solver.toml`: `memory_limit_mib` (12 GiB by default, refused above the
16 GiB ceiling), `precision` and `solve.threads`. Only `f64` is accepted today;
`f32` and 16-bit storage are steps 7 and 10 of `docs/phase-4/PLAN.md` and are
rejected by name until then. A `threads` of zero is resolved once through the
platform's reported parallelism, and the estimate charges one traversal buffer
set and one terminal scratch per resolved worker, plus one more of each for a
strategy query that overlaps an iteration. The walk itself stays serial on the
first workspace until step 4 gives each worker its own terminal evaluator.

That `workers + 1` is one overlapping query, not two. A second query running
beside the first is outside the bound: it takes its own decision report and
workspaces from the same budget, which returns `SolveError::MemoryLimit` as soon
as the configured limit is reached rather than allocating past it. A caller that
wants two concurrent queries has to configure a limit above the bound by another
decision report and one more traversal buffer set and scratch.

### The memory estimate

`PostflopMemory::estimate` runs before a single row is allocated, and the game
refuses with `SolveError::MemoryLimit` rather than allocating part of a tree it
cannot hold. It reads the compact tree once for the per-street node counts, the
per-street sum of action counts, and the chance nodes on each street, then
multiplies each street by the number of boards that street has. It never
multiplies one street block by another's node count: a deep raise target can
clamp to the stack and merge into the all-in, which leaves the blocks different
sizes.

For a tree with `A` state-action entries after expansion, the three solver arrays
and the current policy cost the same 24 bytes each the river charges, and one
retained average adds 8 more. The expansion adds three things on top: showdown
tables at 64 KiB per complete board, the mask pool at 8 bytes per card per
private state per player, and the per-node metadata.

It also charges what construction itself holds and frees, so the refusal covers the
whole peak rather than only what survives. Those terms are a 52-entry child table per
board state, one interning-map entry per complete board and per dealt card, and
everything the path validation walks with: one visited flag per expanded node, a
pending-node stack of at most `(depth + 1) * max(52, max_actions)` entries at
twice its peak length for the vector's growth, the showdown scratch and column
buffers its terminal reads need, and the pair matrix its zero-sum pass holds
between its two passes. They are
transient, which is why the total is a bound and not a snapshot: no run holds the
construction buffers and a solver at the same time. The turn gate tree at the decided menu expands to
9,003 public nodes and 11.4 million entries: about 91 MB per array, 454 MB for
the five. The flop gate tree expands to 1,792,006 nodes and 2.22 billion entries,
which is roughly 89 GB across the five arrays, so it does not fit in the 12 GiB
default at `f64`. Step 6's in-range compaction and step 7's `f32` storage are
what close that gap; until then the flop refuses and says how much it needed.

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

A chance mask depends on the card dealt, not on the history that deals it, so the
masks live in one pool on the layout and each chance node stores one index per
outcome. A turn tree holding a chance node per betting history keeps one mask pair
per card instead of one per node and outcome; the river builds no chance node and
pools nothing. Entries are matched on exact bit patterns, so a mask spelling zero
as `-0.0` gets its own entry rather than sharing one, and every value a traversal
reads is the value the game supplied.

All storage and accumulation are f64. For M state-action entries, regrets,
strategy sums, and the current strategy occupy 24M bytes, before vectors and the
tree. Reading an average adds 8M bytes. The pooled masks add 8 bytes per distinct
card per private state per player, plus one 8-byte index per chance node outcome.
The validated layout is shared through an Arc. The legacy callback binding
retains its private-pair matrix on top of that.

`precision` in the configuration file names the width the accumulators are kept
at between iterations: `"f64"`, `"f32"`, or `"i16"`. Only `"f64"` is accepted, and
it is the default when the key is absent. `"f32"` and `"i16"` parse and are then
rejected by validation with a message naming the step of `docs/phase-4/PLAN.md`
that implements them, so a file asking for a width this build does not have fails
at load instead of being silently solved in f64.

Each terminal also stores both players' payoff kernels as f64 bit patterns,
requiring 16 * H0 * H1 bytes per terminal. Construction and each checked reuse
evaluate H0 + H1 unit opponent vectors per terminal. This binds utilities as well
as geometry, catching a changed payoff with the same public tree. The cost is
deliberate for the three-state and six-state toy games. The owned river path does
not construct or inspect those legacy kernels.

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

Every thread count is accepted: 0 asks for one per available core, 1 asks for
serial execution, and a larger number asks for a pool of that size. The solve runs
serially whatever is asked, because the parallel traversal over runouts arrives in
step 4 of `docs/phase-4/PLAN.md`. Until then a value above 1 records an intent and
changes nothing about a run. Progress checks occur between complete iterations, so
a single slow iteration may exceed log_every_secs. Configuring that interval does
not start a background thread.

Crate unit tests pin discounting, signed storage, and invalid configuration.
The separate toygames package owns independent history-oracle, Kuhn/Leduc,
OpenSpiel curve, and fixed-budget convergence checks. Runtime results belong in
the integration review: local Rust compilation remains unavailable under Smart
App Control, and root runs the approved GitHub Actions checks.
