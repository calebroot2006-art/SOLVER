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
which is what the estimate charges: the flop tree reaches the river through an
ordered pair of dealt cards, and the two orders name the same five-card board.

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
always cover the whole tree. Its `pairs` and `zero_sum_terminals` are zero when
their check was gated off, which means it did not run, never that it passed, and
the two are gated separately: the mass check needs the live pairs and the
expanded nodes inside their limits, and the zero-sum check needs that plus its
own limit on the columns it would read. A tree can therefore report its pairs
and no zero-sum terminals, so read each field before trusting its check. The
fixtures in `tests/streets.rs` sit inside every budget, so they exercise all of
it; a gate tree gets the structural half and reports zero for the rest.

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
strategy query that overlaps an iteration. `PostflopSolver::workers()` reports
the number that one answer produced: it is the size of the thread pool the
solver runs on and the worker count the estimate charged for, so the two cannot
drift apart.

That `workers + 1` is one overlapping query, not two. A second query running
beside the first is outside the bound: it takes its own decision report and
workspaces from the same budget, which returns `SolveError::MemoryLimit` as soon
as the configured limit is reached rather than allocating past it. A caller that
wants two concurrent queries has to configure a limit above the bound by another
decision report and one more traversal buffer set and scratch.

### Runouts in parallel, and why the answer does not move

Above one worker the solver builds a `rayon` thread pool of exactly the resolved
worker count, and every chance node that deals more than one card spreads its
outcomes across it. One worker builds no pool and runs the serial walk it has
always run.

The result is identical to the bit at any worker count, and that is a property
of three things rather than of luck. Each outcome is walked by the same `walk`
that a serial traversal uses, so the arithmetic inside a runout never changes.
The outcome values come back in outcome order, because collecting an indexed
parallel iterator preserves order, and the parent sums them in that order, so
the one associativity a floating-point sum is sensitive to is fixed. And each
outcome writes only its own regrets and strategy sums: accumulators are split
with `split_at_mut` along `outcome_range(chance, outcome)`, and a split that
does not match the tree is refused rather than allowed to overlap. Nested deals
nest the split, because each level partitions its own parent's range.

Errors follow the same rule. Every outcome's result is collected, then read in
outcome order, so a failed runout is reported by the lowest outcome index that
failed and not by whichever worker failed first. A failed iteration still
poisons the solver whole, so no partial strategy is ever readable.

The terminal boundary is the one piece that could not stay shared. `Traversal`
holds a `&mut dyn TerminalEvaluator` over one showdown workspace, so each worker
takes the workspace its own pool index names; `ShowdownTable::evaluate` clears
that workspace before it reads any of it, so nothing carries between workers.

The estimate charges for the change. A serial chance node holds one value vector
at a time. A parallel one collects every outcome's vector before reducing them,
so above one worker the traversal buffer term is sized by the widest deal rather
than the widest bet menu. At one worker the term, and every number a serial
solve has recorded, is what it was.

`rayon` is pinned at `=1.12.0` in the workspace manifest. It is dual licensed
MIT OR Apache-2.0, which is inside the licence decision recorded in
`docs/research/README.md`.

`serde_json` is a **dev-dependency**, added by step 5b and used by one example:
`examples/turn_capture.rs` reads `tests/reference/turn/cases.json`, the same
input file the independent reference reads. Nothing in `src/` parses JSON, so it
is not linked into the solver. It is pinned in the workspace manifest and is dual
licensed MIT OR Apache-2.0, the same decision as `rayon`'s.

### The memory estimate

`PostflopMemory::estimate` runs before a single row is allocated, and the game
refuses with `SolveError::MemoryLimit` rather than allocating part of a tree it
cannot hold. It reads the compact tree once for the per-street node counts, the
per-street sum of action counts, and the chance nodes on each street, then
multiplies each street by the number of boards that street has. It never
multiplies one street block by another's node count: a deep raise target can
clamp to the stack and merge into the all-in, which leaves the blocks different
sizes.

It is computed over the live combos, not over all 1326. A combo with no weight
in its range, or one the board prefix blocks, is dealt to nobody: its live mask
is zero at every terminal, so its regrets and strategy sums stay at zero for the
whole solve. In-range compaction drops those rows. `PostflopGame::new` counts them before it
asks for the estimate, so the refusal is measured against the buffers the
compacted walk will actually hold. Over the Decision 9 ranges that is 34.5% to
37.5% of the full width, worth about a factor of 2.7.

For a tree with `A` state-action entries after expansion over those live combos,
the two stored arrays cost 8 bytes each per entry and one retained average adds
8 more. There is no third array: the current policy is regret matching over the
regrets, derived where a walk reads it. There is no per-node `Vec` header either: every node's row is a slice of one flat
buffer. Showdown tables cost 64 KiB per complete board. The mask pool costs
8 bytes per live combo per player per card, plus vector headers. The topology
costs 40 bytes per node plus 4 per edge, with a separate bound for the interned
payoff table.

It also charges what construction itself holds and frees, so the refusal covers the
whole peak rather than only what survives. Expansion retains one `NodeBuild`
record per expanded node, with its child, probability and mask-index buffers,
while the flat layout allocates its copies. Those overlapping allocations are
charged in full. Expansion also holds copied decks on the recursive path, a
52-entry child table per board state, growing interning maps and vector headers.
Path validation uses a visited flag per node and a pending-node stack. That
stack holds at most `(depth + 1) * max(52, max_actions)` entries, charged at twice
its peak length for growth. Validation also holds terminal scratch, column
buffers, compatibility masks, scoped indices and a pair matrix between passes.
Construction frees these buffers before the solver starts. The sum deliberately
charges both lifetimes to bound the peak. The next section prices the
two gate trees term by term.

### The exact gate table

`PostflopMemory::rows_under` reports each buffer's representation, bytes and
lifetime. It also identifies rows that share an allocation. The counted rows sum
to `working_set_bound_bytes`. `MemoryReservation` names the rows behind every
fixed reservation; `crates/postflop/tests/streets.rs` checks these against the
budget counter. Imports additionally reserve their supplied
buffer capacities. `from_rows` reserves the consumed row buffers and the new
flat snapshot before copying; it releases the input reservation after the copy.
`from_values` retains and charges the vector's actual capacity. These variable
input costs are above the default snapshot row, so an import may need more
headroom than an average produced by the solver.

A `StoragePlan` prices a layout other than the implemented one.
`PostflopMemory::plan` is what the code stores, and its bound is the estimate.
Other plans use the same entry counts. `f32` is step 7; `i16` with one `f32`
scale per decision node per array is step 10. `StoragePlan::before_compaction`
prices the arrays and snapshots stored before step 6. These are estimates.

Every row whose size depends on how many private states a walk carries follows
the plan: the stored arrays, the snapshots, the compression scales, the chance
mask pool and the traversal buffers. The rest do not, and that is not an
oversight. The terminal boundary stays 1326 wide because `ShowdownTable` and
`evaluate_fold` are written against combo IDs, so the walk scatters a compacted
opponent reach into a full-width vector and gathers the live entries back out.
The two reports stay 1326 wide because a consumer of a solved spot asks in combo
IDs. The topology does not depend on the ranges at all.

Run the table with `cargo run -p postflop --example memory_table [workers]`. The
numbers below are its output on 2026-09-10 with one worker and the step 6
construction and scratch corrections. `tests/streets.rs` pins the two fixture
sums and both gate sums.

Both gate trees are the decided menu (Decisions 1 and 10) on a 100bb
single-raised pot: 33% pot plus all-in on the flop and the turn, `33%,75%` with
no all-in token on the river, one raise per street at 100% of pot. The turn gate
is 214 compact nodes expanding to 9,003 public nodes, 3,178 of them decisions,
over 49 board states. The flop gate is 925 compact nodes expanding to 1,792,006
public nodes, 637,500 of them decisions, over 2,402 board states. One stored array holds
11,363,820 entries on the turn and 2,222,795,016 on the flop. The same menu at
the reference capture's chip scale (pot 11, stack 195, minimum bet 1) builds the
same tree and the same bound as the phase 3 scale (pot 55, stack 975, minimum bet
10) the table uses.

The rows below are priced over all 1326 combos, which is the upper bound over
every pair of ranges; the live-combo sums a real game is refused against are the
table after it.

| Row | Turn gate, bytes | Flop gate, bytes | Lifetime |
|---|---:|---:|---|
| compact betting tree | 24,156 | 96,376 | whole solve |
| expanded topology and offsets | 410,848 | 78,908,484 | whole solve |
| chance probabilities and mask indices | 108,024 | 21,504,060 | whole solve |
| board metadata | 8,820 | 432,360 | whole solve |
| showdown tables | 3,145,728 | 77,070,336 | whole solve |
| chance mask pool | 1,106,304 | 1,106,304 | whole solve |
| ranges and evaluator tables | 369,712 | 369,712 | whole solve |
| regrets | 90,910,560 | 17,782,360,128 | whole solve |
| strategy sums | 90,910,560 | 17,782,360,128 | whole solve |
| CFR bookkeeping | 1,176 | 1,176 | whole solve |
| average-strategy snapshots (one) | 90,910,856 | 17,782,360,424 | held while the caller keeps it |
| per-node compression scales | 0 | 0 | whole solve |
| traversal value buffers | 1,889,536 | 2,576,640 | per iteration |
| terminal showdown scratch | 106,960 | 106,960 | whole solve |
| query workspace | 1,996,496 | 2,683,600 | per query |
| decision-value report | 106,592 | 106,592 | per query |
| node-value report | 85,376 | 85,376 | per query |
| construction transients | 5,925,835 | 160,737,150 | construction only |
| **counted total** | **288,017,539** | **53,692,865,806** | |
| best-response verification walk (not counted) | 1,996,496 | 2,683,600 | per verification |

Three rows moved for a structural reason rather than an arithmetic one. The
topology fell from 2,488,280 to 410,848 bytes on the turn and from 495,519,164
to 78,908,484 on the flop, because a node no longer owns three `Vec` headers and
an inline 56-byte payoff record. The chance row rose, from 6,912 to 108,024 and
from 1,851,024 to 21,504,060. The probability and mask arrays now run parallel to
the whole edge array, so an action edge carries a pair it never reads. That is 12
bytes per action edge against the 24 the headers cost per node. And the showdown tables halved on the flop, from 154,140,672 to
77,070,336, because they are charged per completed board rather than per ordered
runout, which is how the build has always interned them.

Terminal scratch includes both boxed 1326-entry buffers, the inline workspace,
the worker's mutex, and the vector and reservation wrappers. The construction
validation also charges its boxed workspace, compatibility masks, scoped indices,
opponent reach and output column.

The two report rows are counted separately rather than one aliasing the other.
A decision report is sized by the widest menu and a node report by the player
count. A caller can hold one of each. `decision_values` answers what each action
is worth to the actor; `node_values` answers what the history is worth to both
players. Counting both is what
lets a caller size `memory_limit_bytes` to this bound and still query a solved
tree.

Three rows need their overlap spelled out. The construction transients are
counted although they are freed before a solver exists, because the refusal has
to cover the peak construction reaches. The verification walk is not counted at
all, because it allocates nothing of its own. It normalises the strategy sums per
node as it reads them, so it takes the traversal buffers and scratch an iteration
already holds, and no average. Its row's bytes are what
`SolveSession::measurement` costs; a serial `PostflopStrategy::exploitability`
walks the same tree on the query workspace instead. And the snapshot row is one
snapshot, which is the 5d contract's "at most one alive per job". A running solve
retains none. The one the bound charges is what a caller browsing a finished
result holds. It is taken at an iteration boundary by
`average_strategy`, `uniform`, `from_rows` or `from_values`, and freed when its
`PostflopStrategy` drops, so its lifetime is the caller's rather than a phase of
the solve. A caller holding two averages at once, like one running two
concurrent queries, needs the configured limit raised by another snapshot; the
budget returns `SolveError::MemoryLimit` rather than allocating past it.

Sums per storage width, in bytes, at one worker. The compacted rows use the
widest board of each set, which is `8h 8d 3c Ks` on the turn (468 and 473 live
combos) and `8h 8d 3c` on the flop (491 and 504). Live combos are the ones with
positive weight that the board prefix does not block, over the Decision 9 ranges;
across the six flops priced they run from 34.5% to 37.5% of the full 1326-state
width, so compaction is worth roughly a factor of 2.7.

| Layout | Tree | f64 | f32 | i16 |
|---|---|---:|---:|---:|
| before step 6: 3 arrays, 2 snapshots, 1326 states | turn | 469,838,955 | 242,562,555 | 128,987,915 |
| now: 2 arrays, 1 browsing snapshot, live states | turn | 108,944,475 | 60,558,255 | 36,403,281 |
| now, mid-solve: 2 arrays, no snapshot, live states | turn | 76,686,699 | 44,429,219 | 28,325,903 |
| before step 6: 3 arrays, 2 snapshots, 1326 states | flop | 89,257,586,358 | 44,801,686,038 | 22,586,485,878 |
| now: 2 arrays, 1 browsing snapshot, live states | flop | 20,357,152,670 | 10,349,546,150 | 5,353,392,890 |
| now, mid-solve: 2 arrays, no snapshot, live states | flop | 13,685,414,694 | 7,013,677,014 | 3,682,908,174 |

The first row of each pair is the old storage plan under the current topology
accounting, so it is not the number step 5c recorded: the topology, chance and
scratch terms moved too. It is here to say what the storage change alone was
worth, not to reprice history.

What the arithmetic says about the order of the work. The turn gate fits the
12 GiB default at every width, with 11.9 GiB to spare, and it is where the gate
capture runs. The flop gate does not fit at `f64`. Compacted to live combos,
with the current policy derived rather than stored and no average retained, it
needs 13,685,414,694 bytes while it solves: 763.43 MiB over the 12 GiB default,
though inside the configured 16 GiB hard ceiling. Browsing a finished flop
result adds 6,671,737,976 bytes for the compact snapshot,
which takes it to 20,357,152,670 and out of reach of both.

So step 6 was required and is not sufficient, which is what step 5c predicted.
Step 7's projected `f32` accounting fits: 7,013,677,014 bytes mid-solve, 5.47 GiB spare, and
10,349,546,150 with a browsing snapshot alive, 2.36 GiB spare. Step 10's `i16`
is not required for the gate to fit; what it buys is headroom, 3,682,908,174
bytes mid-solve, which leaves room for more workers and a wider menu than the
gate's. These are allocation estimates; the required host peak-memory
measurement remains separate.

The reference solver is estimated separately, by itself: the pinned wasm-postflop
build reports `reference_memory_estimate_bytes` of 24,670,040, 18,654,832 and
17,239,588 for the dry rainbow, paired and flush turn cases (the `turn-wasm-reference`
artifact of CI run 34401787355). Those are its own accounting of its own solver
and are not comparable term by term with the rows above; it also merges isomorphic
runouts, which our tree does not until step 9. Its `private_hand_counts` are 469
and 470, 468 and 473, 445 and 448 for the three cases, which are exactly the live
combos this example counts from the same ranges and boards.

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

All storage and accumulation are f64. For M state-action entries, regrets and
strategy sums occupy 16M bytes, each of them one flat buffer sliced by the
layout's per-node `u64` row offsets. There is no third array for the current
policy: it is regret matching over the regrets, and a walk derives one node's
row into a pooled buffer at the moment it reads it, before it touches that
node's regrets. That is the row a stored copy would have held, so the arithmetic
is unchanged. Reading an average adds 8M bytes, and a measurement adds none: the
best-response walk normalises the strategy sums per node as it reads them. The
pooled masks add 8 bytes per distinct card per private state per player, plus a
`u32` index per edge. The validated layout is shared through an Arc. The legacy
callback binding retains its private-pair matrix on top of that.

An owned postflop game carries one private state per live combo rather than all
1326, and the layout's `states`, weights and masks are that wide. A strategy row
is indexed by that compact state, and `PostflopGame::live_combos` and
`state_of` are the two directions of the map. Everything a caller reads back is
still indexed by combo ID: `PostflopStrategy::row` takes a `Combo`, and the two
value reports return 1326-entry vectors. Callback games and the river module are
not compacted; they declare their own state counts.

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
cargo run -p postflop --example memory_table
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The `memory_table` example prints the working-set table of the section above for
both gate trees, at three storage widths, and takes an optional worker count.

Load `config/solver.toml` with `SolverConfig::load`, construct
`Cfr::new(game, config.dcfr.variant())`, then call
`solve(game, &mut cfr, &config.solve, callback)`. Both current and average strategy
accessors return a Result. The driver reports TargetReached or IterationCap;
the latter is never described as convergence.

Every thread count is accepted: 0 asks for one per available core, 1 asks for
serial execution, and a larger number asks for a pool of that size. Progress
checks occur between complete iterations, so a single slow iteration may exceed
log_every_secs. Configuring that interval does not start a background thread.

Crate unit tests pin discounting, signed storage, and invalid configuration.
The separate toygames package owns independent history-oracle, Kuhn/Leduc,
OpenSpiel curve, and fixed-budget convergence checks. Runtime results belong in
the integration review: local Rust compilation remains unavailable under Smart
App Control, and root runs the approved GitHub Actions checks.
