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
| `compare.py` | Reference-only validation, or project versus reference. |
| `review_combos.py` | Per-row evidence for every policy difference over two percentage points. |
| `oracle.py` | An independent scalar evaluator, used by `review_combos.py`. |
| `_fixture.py` | Synthetic captures for the unit tests. Not used by anything above. |
| `test_*.py`, `capture.test.mjs` | The guards: 57 Python tests and 12 Node tests. They need no WASM build and run in a second. |

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

## The exported runouts

The reference solves every dealable runout. It exports only the ones a case names in
`export_runouts`. Exporting all 48 would be about 12,000 nodes per case, and at the measured
7 KB a node that is roughly 90 MB per case: past the capture's own 64 MiB ceiling, and past
what anyone would read. The exploitability, the iteration count and the stop reason all come
from the whole tree; only the node dump is scoped.

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
Every policy cell must then equal its twin under the swap, and any difference fails the run. On the
committed cases this compares 105,391 cells for `2c`/`2d` and finds a maximum difference of
exactly zero, while comparing the same cells without the swap differs by up to 0.249.

Our solver does not merge anything in phase 4 (isomorphism is step 9), so `compare.py` records
both merge counts side by side and requires only that the two `possible_cards` sets agree.

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

**Project versus reference.** Available once `crates/postflop/examples/turn_capture.rs` lands
in step 5b.

```bash
python tests/reference/turn/compare.py \
  --reference target/turn-reference/cases.json \
  --project target/turn-project/cases.toml \
  --review tests/reference/turn/measured/<sha>/per-combo-review.json \
  --output target/turn-compare/report.json
```

Every exported public history must exist on both sides with the same kind, street, runout,
contributions and action labels. Every dealable-runout set must match, and every combo's
policy row is compared. A row whose frequency differs by more than two percentage points on
any action is listed in `differences`. With `--review`, each such row must appear in the
committed `per-combo-review.json` with a non-empty `review_reasoning`. Any row that does not
fails the run with exit code 1. There is no aggregate waiver: one unexplained row fails.

`review_combos.py` builds the review file from an initial and a refined capture of each side,
the same four-input shape the river uses.

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

## What step 5b must emit

`turn_capture.rs` takes the cases file and writes a TOML capture:

```
cargo run --release --locked -p postflop --example turn_capture -- \
  tests/reference/turn/cases.json target/turn-project/cases.toml
```

Top level: `schema_version = 1`, `street = "turn"`, `project_revision`,
`execution_stop_policy`, `os`, `architecture`. Each `[[cases]]` carries `input` (the case
verbatim), `iterations`, `stop_reason` (`TargetReached` or `IterationCap`),
`exploitability_pct_of_pot`, `root_centered_expected_values`, `best_response_values`,
`compatible_weight` and `nodes`.

Each `[[cases.nodes]]` carries `history_labels`, `kind`, `street`, `runout` (`""` off a
runout), `contributions`, `terminal` (`""` on a decision node), `fold_winner` (`-1` when
there is none), `player` (`-1` when there is none) and `actions` as labels.

* A decision node adds `hands`, one entry per live combo, with `cards`, `strategy`,
  `action_expected_values` (centered: our own convention, not the wrapper's display origin),
  `ev_available`, `own_reach` and `opponent_mass`.
* A chance node adds `possible_cards`, `isomorphic_merged_cards`, `exported_runouts` and a
  `hands` list whose entries also carry `player` and `expected_value`.
* A turn-street showdown terminal, meaning an all-in called before the river, adds the same
  `hands` list. River terminals carry no values; the oracle recomputes them.

`_fixture.py` builds exactly this shape, so it doubles as the worked example.

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
comparison, and uploads the result as the `turn-wasm-reference` artifact. The budget is
larger than the river job's because a turn tree with these ranges is minutes of
single-threaded WASM per case.

`turn-solve` (both platforms, 120 minutes) runs `turn_capture` and records peak resident set
size, with `/usr/bin/time -v` on Linux and a `Get-Process` poll on Windows. It is skipped
until step 5b lands the example: a `turn-solve-gate` job looks for the file and turn-solve
depends on its answer, so an absent example shows as a skipped job rather than a green one
that ran nothing.

## Known limits

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
* `chips_per_bb` is 2, not the river's 1. A 5.5bb pot is not an integer in chips otherwise.
