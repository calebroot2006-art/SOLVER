# Turn reference set

Cross-check for turn-start solves, against the pinned external solver. It is the sibling of
`tests/reference/river/`, and everything the river README says about the licence boundary
still holds: the external solver is cloned in CI, into the runner's temp directory, built
there, and never committed. Read `docs/astra/phase-3/reference-contract.md` first if you have
not.

Pinned revisions, identical to the river's:

| Component | Revision |
| --- | --- |
| `b-inary/wasm-postflop` | `97360db7644329b1c23a7adf06e9aa59406e4d4b` |
| `b-inary/postflop-solver` | `9d1509fe5077d019825f833eed04b16d342dfda1` |
| Rust toolchain | `nightly-2023-10-01` |
| `wasm-bindgen-cli` | `0.2.87` |
| Node | `v24.19.0` |

## Files

| File | What it does |
| --- | --- |
| `cases.json` | The three turn fixtures: board, ranges, menus, targets, exported runouts. |
| `capture.py` | Clones and builds the pinned reference, runs the driver, validates the result. |
| `capture.mjs` | The driver itself: talks to the WASM binding and walks the tree. |
| `raise_cap.py` | Derives the `removed_lines` that prune the reference to our raise cap. |
| `compare.py` | Reference-only validation, or project versus reference. |
| `review_combos.py` | Per-row evidence for every policy difference over two percentage points. |
| `oracle.py` | An independent scalar evaluator, used by `review_combos.py`. |
| `_fixture.py` | Synthetic captures for the unit tests. Not used by anything above. |
| `test_*.py`, `capture.test.mjs` | The guards: 86 Python tests and 14 Node tests. They need no WASM build and run in a second. |

## What a turn tree adds

Three things, and each one shows up in the schema.

**A four-card board.** The binding infers the street from the board length, so a four-card
board builds a turn tree. `capture.mjs` sorts the first three cards before passing them,
matching upstream's convention, and leaves the turn card last.

**Per-street, per-player menus.** The binding takes sixteen separate size strings, not two.
`cases.json` therefore carries a `menus` object with a `flop`, `turn` and `river` entry, each
naming `oop_bet`, `oop_raise`, `ip_bet`, `ip_raise`, and, on the turn and river, `oop_donk`.
`initArguments` in `capture.mjs` maps them onto the binding's argument list in order, and
`capture.test.mjs` pins that order so a future edit cannot silently transpose two strings. A
turn tree never reads the flop entry, so it must be empty.

**A chance node.** The turn betting round ends at a river deal. In an exported history a
chance entry is a card ID between 0 and 51, not an action index, and its label is
`chance:<card>`, for example `chance:Qd`. Everything below such an entry has `street:
"river"` and carries the runout card in `runout`.

## The raise cap, and how the reference is pruned to it

Decision 10 caps every phase 4 gate tree at one raise per street and sizes that raise at 100%
of pot. The pinned binding has no raise-cap input, so a tree built from that menu keeps
re-raising until the stack runs out: on these cases a turn street reaches four wagers,
`bet:4`, `raise:23`, `raise:80`, `allin:195`. That is why `max_raises: 1` first failed the
capture outright (run 34079922254).

Decision 11 (2026-09-07) prunes the reference instead of changing the menus. `init`'s last
argument, `removed_lines`, takes a comma-separated list of lines; a line is a sequence of
action tokens joined by `-` or `|` (`F` fold, `X` check, `C` call, `B<n>` bet, `R<n>` raise,
`A<n>` all-in, `<n>` the amount the tree stores). Removing a line deletes that action *and its
whole subtree* from its parent, and chance actions are omitted from a line, so a street change
is implicit in the token sequence. A line that does not exist in the tree makes `init` fail, so
a wrong derivation is loud rather than quiet.

`raise_cap.py` derives those lines. It replays upstream's own amount arithmetic in Python (the
pot, the clamp to the stack, the all-in threshold, the sort and the dedup) and returns one line
per wager the cap forbids, ending on that wager. It never descends into a branch it has
removed, so no line sits under another and the order they are applied in does not matter. For
the three committed cases the answer is the same 18 lines, because the cases differ only in
their board and the amounts depend on the pot and the stack:

```
B4-R23-R80              the turn: bet, raise, and the re-raise the cap forbids
X-B4-R23-R80            the same after a check
X-X-B4-R23-R80          the river after the turn checks through
B4-C-B14-R61-A191       the river after a turn bet and call; the third wager is all-in
B4-R23-C-B19-R114-A172  the river after a turn bet, raise and call
```

The lines are derived at capture time, not committed. They are a function of the menus, pot,
stack and cap in `cases.json`, and a stale copy in the input file would silently prune the
wrong tree. The capture records what it used in each case's `removed_lines`, beside `input`
rather than inside it, and `capture.py` re-derives them from the case whenever it validates a
capture, so an artifact read months later still proves which branches were dropped. Step 5b's
project capture echoes `input` verbatim and needs to know nothing about the pruning.

Two checks then say the pruning worked. `capture.py` counts the wagers on each street of every
exported history and requires no street to exceed the cap, and requires that a node already at
the cap offers no further wager. `compare.py` repeats both over the committed artifact and
names the case and the history it rejects. Upstream's action tree holds a single chance action,
so every runout has the same betting structure: the exported runouts cover the whole tree's
shape rather than a sample of it.

`max_raises` accepts 0 to 32. Zero is a real setting, a bet that can only be folded to or
called. Thirty-two is deeper than these stacks can reach, so it derives no lines at all and
captures the unpruned tree, which is what the river cases still do.

## The exported runouts

The reference solves every dealable runout. It exports only the ones a case names in
`export_runouts`. The pruned tree exports 21 turn nodes and 135 nodes per runout, so all 48
would be 6,501 nodes per case. At the measured 7 KB a node that is 45 to 50 MB per case: past
the capture's 4,000-node ceiling, past its 64 MiB output ceiling once three cases share a file,
and past what anyone would read. The exploitability, the iteration count and the stop reason all
come from the whole tree; only the node dump is scoped.

The cases export three or four runouts each, chosen to mean something on that board rather
than at random:

| Case | Board | Exported runouts | Why |
| --- | --- | --- | --- |
| `turn_100bb_dry_rainbow` | `9c 5d 2h Ks` | `Qd`, `7c`, `9s` | An overcard, a low blank, and a card that pairs the board. |
| `turn_100bb_paired` | `8h 8d 3c Ks` | `Ac`, `4d`, `4h`, `8s` | An overcard, the case eight, and a pair of runouts the reference treats as isomorphic. |
| `turn_100bb_flush_possible` | `As Js 8s 4h` | `Ts`, `2c`, `2d` | The flush completes, plus another isomorphic pair. |

### Isomorphic runouts

The reference merges two runouts whenever some suit permutation fixes the four-card board and
leaves both ranges unchanged. That is not only a monotone-board phenomenon. `As Js 8s 4h`
shows no club and no diamond, so every club river merges with its diamond twin: 13 merges,
`possible_cards` 48, `representative_action_count` 35. `8h 8d 3c Ks` shows all four suits and
still merges 12 runouts, because the board is *paired*: swapping hearts and diamonds maps
`8h 8d` onto itself. Only `9c 5d 2h Ks`, with four ranks and four suits, merges nothing.

Playing a merged card still works. The engine replays the representative and swaps the suits
back, so the exported rows are indexed by the card that was actually dealt. `compare.py`
checks that rather than trusting it. It takes each pair of exported runouts of the same rank
whose suits are interchangeable on this board and whose swap leaves both ranges unchanged
(`ranges_are_suit_symmetric` decides the second part from the range text, weights included).
Every policy cell must then equal its twin under the swap, and any difference fails the run. On
the committed cases this compares 56,615 cells for `2c`/`2d` and 58,305 for `4d`/`4h`, and finds
a maximum difference of exactly zero in both. Comparing the same `2c`/`2d` cells without the
swap differs by up to 0.599, so the check is not vacuous.

Our solver does not merge anything in phase 4 (isomorphism is step 9), so `compare.py` records
both merge counts side by side and requires only that the two `possible_cards` sets agree.

## What the reference measured

From run 34284399884, the first capture with `max_raises: 1` and the pruning in place,
reproduced unchanged by run 34285342323, the fully green run at 4147355. Each
case has 5 chance nodes, 48 dealable runouts, and 18 removed lines. The target is 0.25% of pot
and all three reach it, so none of these numbers is an iteration-cap number.

| Case | Iterations | Exploitability | Merged runouts | Exported nodes | Memory estimate | Solve |
| --- | --- | --- | --- | --- | --- | --- |
| `turn_100bb_dry_rainbow` | 150 | 0.1587% of pot (0.01746 chips) | 0 of 48 | 426 | 24.7 MB | 6.7 s |
| `turn_100bb_paired` | 200 | 0.1783% of pot (0.01961 chips) | 12 of 48 | 561 | 18.7 MB | 6.6 s |
| `turn_100bb_flush_possible` | 150 | 0.1851% of pot (0.02036 chips) | 13 of 48 | 426 | 17.2 MB | 5.2 s |

The memory estimate is the reference's own `memory_usage(false)`, not ours, and the solve time
is single-threaded WASM under Node. Both are context for the step 5b comparison rather than a
target: our numbers are measured separately by `turn-solve`.

## Ranges

Button opens 2.5bb, big blind calls, no ante: a 5.5bb preflop pot with 97.5bb behind. The
flop checks through, so the turn pot is still 5.5bb. `chips_per_bb` is 2, which makes those
11 and 195 chips exactly.

The two ranges were drafted by us from `docs/research/preflop-charts-and-ranges.md` and
approved by Caleb on 2026-09-06. No vendor chart is copied. `cases.json` records the same
provenance in `ranges_provenance`, and `capture.py` refuses an input file that omits it. OOP
is the big blind caller (542 combos, 532.5 weight); IP is the button opener (546 combos).

**Range strings.** The tokens are written in the grammar of `crates/cards/src/range.rs`, and
both `capture.py` and `capture.mjs` reimplement that grammar rather than calling the external
parser, which never sees a range string: the binding takes 1326 raw weights. The two grammars
are not interchangeable in either direction.

* Our parser accepts an interval in either order (`22-TT`, `A2s-AJs`). The external parser
  demands descending order and rejects both of those with "Range must be in descending
  order"; it wants `TT-22` and `AJs-A2s`. The sets are identical.
* `+` on a non-pair differs. Ours holds the high rank and walks the low one, so `ATo+` is
  `ATo,AJo,AQo,AKo`. The external parser holds the gap, so its `98s+` is
  `98s,T9s,JTs,KQs,AKs`. The gate ranges use `+` only on pairs (`22+`), where the two agree.

If you change a range, run `python -m unittest discover -s tests/reference/turn -p 'test_*.py'`
first. The capture also cross-checks its own expansion against the private cards the reference
reports, so a disagreement fails the run instead of quietly comparing two different ranges.

## The two comparison modes

**Reference only.** No project capture required. This is what CI runs today.

```bash
python tests/reference/turn/compare.py \
  --reference target/turn-reference/cases.json \
  --output target/turn-reference/reference-summary.json
```

It validates the capture against the output contract in `capture.py` (schema, pinned
revisions, topology, strategy rows, private cards against the input ranges), runs the
isomorphic-runout check above, and prints each case's exploitability, iteration count, stop
reason, runout counts and memory estimate. It exits 0 when the capture is well formed. It does
not decide whether our solver is right, because our solver has not produced anything yet.

**Project versus reference.** The turn gate itself.

```bash
python tests/reference/turn/compare.py \
  --reference target/turn-reference/cases.json \
  --project target/turn-project/cases.toml \
  --review tests/reference/turn/per-combo-review.json \
  --expected-revision <the commit the capture measured> \
  --output target/turn-comparison/report.json
```

Every exported public history must exist on both sides with the same kind, street, runout,
contributions and action labels. Every dealable-runout set must match, and every combo's
policy row is compared. A row whose frequency differs by more than two percentage points on
any action is listed in `differences`. With `--review`, each such row must appear in the
committed review with a non-empty `review_reasoning`. There is no aggregate waiver: one
unexplained row fails.

On top of the structural comparison the joint mode refuses, each with a named entry in
`gate_failures` and exit code 1:

* a missing, empty or skipped project capture, by name, so a job that compared nothing can
  never read as a green gate;
* a case that did not reach its own target, or stopped for some other reason, on either
  side, and a capture whose `reached_target` disagrees with its own measurement;
* any difference in the two `input` tables, which carry the case id, board, ranges, menus,
  raise cap, target and starting pot;
* a project capture that does not name a 40-character commit, or names one other than
  `--expected-revision`. In CI that flag is the workflow's own SHA, so a stale artifact
  cannot stand in for a fresh solve. The reference's pinned engine and interface revisions
  are checked separately, by `validate_reference`;
* a stale review: an entry whose recorded row values are no longer the captured ones, or
  which records no values at all. A reasoning sentence is a statement about numbers, so
  re-solve either side and it explains a row that no longer exists. Every covered row has to
  record at least one of `final_frequency_differences`, `project_refined.strategy`,
  `reference_refined.strategy` or `actions`, and each recorded value has to still match.

`accepted` in the report is the gate's whole answer.

`review_combos.py` builds the review file from an initial and a refined capture of each side,
the same four-input shape the river uses. It needs the per-hand values the capture cannot
emit yet; see "Not emitted yet" below.

### Two tree conventions the comparison reconciles

The two trees describe the same game and name parts of it differently. The comparison
restates ours rather than accepting either name, so a real disagreement about chips still
fails.

* **Wager labels.** Our tree names a wager by the actor's total commitment since the root, so
  a river `bet:10` where the actor already put 4 in on the turn is the reference's `bet:6`.
  On a river-rooted tree the two coincide, which is why phase 3 never had to choose.
  `restate_project_labels` subtracts the actor's standing contribution from every project
  wager before matching, and the count it restated is reported under `tree_reconciliation`.
  On the three gate cases it restates 320 labels and every menu then matches exactly.
* **A called all-in.** After an all-in is called on the turn our tree still deals the river:
  the line holds one chance node and one showdown terminal per card, and not a single
  decision. The reference ends the same line at a turn showdown whose value already spans
  every runout. `called_all_in_runouts` finds those subtrees, checks that every exported
  child really is a childless showdown terminal at the same contributions, checks the
  reference really does end the hand there, and reports them per case under
  `called_all_in_run_outs`. No policy row lives in them, so nothing is dropped from the
  comparison. It is worth knowing for the memory work: each of those lines costs 48 expanded
  nodes on our side and one node on theirs.

## What the scalar oracle can and cannot say

`oracle.py` recomputes values from the exported policies alone. Inside an exported runout it
is exact: terminal values come from a brute-force seven-card comparison against the five-card
board, and reach comes from the exported rows. A history in the turn round is different,
because its continuation runs through 44 runouts nobody exported. There the walk stops at the
chance node (and at an all-in called on the turn) and uses the value that capture reported there,
converted out of the wrapper's display origin. Every row records which happened in
`continuation_sources`.

So the oracle does not compute a turn exploitability, and `metrics()` deliberately has no
best-response entry. Exploitability comes from the reference's own `exploitability()` and,
for our side, from the f64 best-response walk over every runout.

## What step 5b emits

`turn_capture.rs` takes the cases file and writes a TOML capture:

```
cargo run --release --locked -p postflop --example turn_capture -- \
  tests/reference/turn/cases.json target/turn-project/cases.toml
```

It reads `config/solver.toml` for the memory limit, the storage width, the worker count and
the progress interval, and takes the target, the iteration cap and the check interval from
each case. `--config <path>` points it at a different file. Nothing about the solve is
compiled in. The capture is written before the gate is judged, so a case that misses its
target still leaves a file to read; the process then exits non-zero naming the case, its
exploitability and its target.

Top level: `schema_version = 1`, `street = "turn"`, `project_revision` and
`project_revision_source` (`GITHUB_SHA`, else `git rev-parse HEAD`, else `unknown`),
`execution_stop_policy`, `os`, `architecture`, `config_path`, `solver_variant`,
`requested_threads`, `resolved_workers`, `ranges_provenance`, and a `[host]` table with
`physical_memory_bytes` and `logical_cpus`. Both host numbers are strings, and both are the
literal `unknown` on a host the capture could not read: an unread host is recorded as unread
rather than failing a solve that is otherwise fine.

Each `[[cases]]` carries `input` (the case verbatim), `iterations`, `stop_reason`
(`TargetReached` or `IterationCap`), `reached_target`, `exploitability_pct_of_pot`,
`exploitability_chips`, `root_centered_expected_values`, `best_response_values`,
`compatible_weight`, `working_set_bound_bytes`, `reserved_bytes`, `elapsed_seconds`,
`exported_nodes`, `workers`, `checkpoints`, `timings` and `nodes`.

Each `[[cases.nodes]]` carries `history_labels`, `kind`, `street`, `runout` (`""` off a
runout), `contributions`, `terminal` (`""` on a decision node), `fold_winner` (`-1` when
there is none), `player` (`-1` when there is none), `actions` as labels, `possible_cards`
and `isomorphic_merged_cards` (both empty or zero away from a chance node). A decision node
adds `hands`, one entry per live combo, with `cards`, `strategy`, `action_expected_values`
(centered: our own convention, not the wrapper's display origin), `ev_available`,
`own_reach` and `opponent_mass`.

### The three timings

`[cases.timings]` records three different clocks, and says what each one covers, because a
single "how fast is it" number would hide the thing phase 7 needs to know.

* `mean_iteration_seconds`, over `timed_iterations` iterations timed one at a time, with the
  minimum and maximum beside it. These are the first iterations of the solve itself, timed
  before the driver takes over and continues from the same solver, so the mean carries no
  measurement time in it and costs no extra work.
* `average_strategy_snapshot_seconds` and `best_response_measurement_seconds`, for a
  measurement taken through the snapshot path an outside caller uses.
  `PostflopStrategy::exploitability` walks on one thread whatever the worker count is, so
  this is not what the driver's own in-solve measurement costs.
* Cancellation, measured twice on a second short run. `cancel_latency_seconds` is a watcher
  thread setting the flag while an iteration is in flight, which is what an app does: the
  number holds the rest of that iteration, the measurement `drive` takes once it observes
  the cancel, and the return. `cancel_measurement_and_return_seconds` is the same run with
  the flag set by the poll itself at a loop top, so it holds only the measurement and the
  return. `cancel_latency_excluding_measurement_seconds` is the difference, which is the
  iteration a mid-iteration cancel had to wait out. Each probe records the iteration it was
  cancelled at and refuses to report anything if it stopped for another reason.

### Not emitted yet: chance-node and turn-showdown per-hand values

`_fixture.project_capture` also gives chance nodes and turn-street showdown terminals a
`hands` list carrying `player` and `expected_value`, and `oracle.py` reads exactly that when
a continuation crosses a runout it cannot walk (`_is_leaf_with_reported_values`). The
capture does not emit it, because there is no accessor for it:
`PostflopStrategy::decision_values` refuses any node that is not a decision, and nothing
else on `PostflopStrategy` returns per-hand values at an arbitrary node.

`compare.py` never reads those values, so the joint comparison is unaffected. `oracle.py`
and `review_combos.py` are, and they fail quietly rather than loudly. `_reported_values`
reads a project leaf out of `node["hands"]`, an omitted list is simply an empty one, and
`leaf` turns a missing value into `0.0`. So every continuation that crosses a chance node is
valued at zero and the walk still returns a number.

Measured on the `3ffdae5` capture, root row `2c2d` of `turn_100bb_dry_rainbow`, actions
check / bet:4 / allin:195:

| source | action EVs |
|---|---|
| project oracle | 2.2255, 1.0665, 5.1029 |
| reference oracle | 14.6994, 14.7035, 5.1039 |
| the project capture's own `decision_values` | 14.9576, 14.6632, 11.4971 |

A reviewer running `review_combos.py` today would be writing reasoning from the first row.
Until the accessor exists, do not use the oracle on a project turn capture; the capture's own
`action_expected_values` are the trustworthy per-row numbers, and `compare.py` already
carries them for every differing row as `project_action_ev` and `project_action_gap`.

Closing it needs a solver-side accessor along the lines of

```rust
impl PostflopStrategy {
    /// Per-hand centered expected values for both players at any node.
    pub fn node_values(&self, node: NodeId) -> Result<PostflopNodeValues, SolveError>;
}
```

which is step 3's code and not step 5b's to add.

## Running it locally

The capture itself is Linux only, because it compiles a nightly `wasm32-unknown-unknown`
toolchain in a temp directory outside the repository; `capture.py` refuses to run anywhere
else. On Windows, use CI or WSL2. The guards run anywhere:

```bash
python -m unittest discover -s tests/reference/turn -p 'test_*.py'
node --test tests/reference/turn/capture.test.mjs
python tests/reference/turn/capture.py --validate-only --inputs tests/reference/turn/cases.json
```

The full capture, on Linux, with the temp root outside the repository:

```bash
python tests/reference/turn/capture.py \
  --inputs tests/reference/turn/cases.json \
  --temp-root /tmp \
  --output target/turn-reference/cases.json
```

Add `--finish-budget` to run the whole iteration budget instead of stopping at the target, and
`--raw-display` to remove the wrapper's display rounding. Both work exactly as they do for the
river, against the same three hash-verified spans of the same wrapper file.

## CI

`turn-reference` (ubuntu, 90 minutes) runs the guards, the capture and the reference-only
comparison, and uploads the result as the `turn-wasm-reference` artifact. On run 34284399884
the whole job took 2 minutes 17 seconds, of which the capture step, including cloning and
building the pinned WASM, was 1 minute 56 seconds. The 90-minute budget is headroom for a
slower runner or a deeper tree, not a measurement: the three pruned solves are about six
seconds each.

`turn-solve` (both platforms, 120 minutes) runs `turn_capture` and records peak resident set
size, with `/usr/bin/time -v` on Linux and a `Get-Process` poll on Windows. It is skipped
until step 5b lands the example: a `turn-solve-gate` job looks for the file and turn-solve
depends on its answer, so an absent example shows as a skipped job rather than a green one
that ran nothing.

`turn-compare` (ubuntu, 30 minutes) needs both of those jobs, downloads the reference and
both project captures, and runs the joint mode once per operating system with
`--expected-revision ${{ github.sha }}`. It passes `--review` when
`tests/reference/turn/per-combo-review.json` exists and says in the log when it does not.
Both reports upload as `turn-comparison` whether the job passed or failed, so a red gate can
be read rather than guessed at. Because it `needs` turn-solve, a skipped or failed solve
leaves it skipped: there is no path by which it reports success without having compared two
captures.

## Known limits

* **The two-point per-row rule does not survive a 0.25% target.** Measured on `3ffdae5`
  against the reference from run 34401787355, both sides under target: 139,495 of 233,567
  compared rows differ by more than two percentage points (43,542 / 53,536 / 42,417 by
  case), with individual differences up to 0.9999. The two solves nevertheless agree on the
  game value to between 0.0007 and 0.0034 chips in a pot of 11. The differing rows are
  near-indifferent: at the highest-weight ones the two actions are within a few hundredths
  of a chip of each other, so the mix between them is barely pinned down at all. The river's
  588-row review was possible because the accepted river record is a refined solve at
  4.6e-05% of pot, roughly four orders of magnitude tighter than this gate's target. Writing
  139,495 reasoning entries is not review; it is a file. Deciding what the turn gate should
  compare instead, whether a refined pass on both sides, a reach-weighted rule, or agreement
  on the game value plus the rows anybody actually reaches, is a plan decision and is open.
* Related: `max_available_action_ev_difference` reaches roughly 280 to 344 chips, always at
  rows with tiny reach. The counterfactual value of an action neither side ever takes is
  pinned down only by the opponent's play in a subtree that is itself barely determined at
  this target. So the size is expected. It does mean the metric bounds nothing until
  convergence is much tighter.
* The capture is 62.9 MB for the three cases, against the 64 MiB `compare.py` will read.
  That is 6% of headroom. `turn_capture` refuses to write a larger file and names the byte
  count, so the failure would be loud, but steps 6, 7 and 9 should expect to have to shrink
  it.
* The exported node set is three or four runouts per case, not 48. A difference confined to
  an unexported runout would not be seen. The exploitability comparison still covers the whole
  tree on both sides, which is the check that would catch it.
* Donk sizes are unsupported: `donk_option` must be false and every `oop_donk` empty. Note
  what upstream does in that case. With no donk sizes configured, the out-of-position player
  is not silenced on the river after calling a turn bet: it gets its ordinary river bet menu.
  Donk sizes only override that menu; they do not create the option. Our tree has to match
  that or the histories will not line up.
* Turn donk sizes are inert in a turn-start tree regardless, because the root's previous
  action is none rather than a chance deal.
* `add_all_in_threshold` and `force_all_in_threshold` are both zero, as in the river
  fixtures, so the all-in in the turn menu is the explicit `a` size and nothing is folded
  into an all-in by a threshold.
* The binding takes no raise cap, so `max_raises` is enforced by pruning: see "The raise cap"
  above. What the reference solves is our tree, but it is not a tree the reference would build
  on its own, and the pruning is only as good as `raise_cap.py`'s replay of upstream's amount
  arithmetic. A derived line that does not exist fails the capture; a forbidden branch the
  derivation missed fails the export check. Neither can be waved through, but both are checks
  on the tree's shape, not proof that a 100%-pot raise is the raise our own tree builds. Step
  5b's history comparison is what settles that.
* `chips_per_bb` is 2, not the river's 1. A 5.5bb pot is not an integer in chips otherwise.
