# Cards, weighted ranges, and hand evaluation

`cards` represents a standard 52-card deck and all 1326 unordered hold'em combos.
Every card, combo, and card set is checked before it enters a range. Range weights
are finite inclusion weights in `[0,1]`; the crate does not normalize them.

## Identity and grid

`Rank` runs from `Two` to `Ace`. `Suit` runs through `Clubs`, `Diamonds`, `Hearts`,
and `Spades`. Card IDs are `4 * rank.index() + suit.index()`, so `2c` is zero and
`As` is 51. A card's mask is `1u64 << card.id()`.

`Combo::new` rejects a repeated card and stores the cards with IDs `a < b`.
Its ID is `b * (b - 1) / 2 + a`. `Combo::all()` visits these IDs from zero to
1325. Parsing accepts either card order; printing puts the higher card ID first.
`AsKh` and `KhAs` therefore identify the same combo and print as `AsKh`.
These IDs are this crate's contract. An external solver adapter must verify its
own array permutation before importing any 1326-entry values.

The grid has ace at row/column zero and deuce at twelve. Its diagonal contains
pairs, its upper half suited hands, and its lower half offsuit hands.
`combos_for_cell(0, 1)` returns the four suited ace-king combos;
`combos_for_cell(1, 0)` returns the twelve offsuit ace-king combos.
Each diagonal cell contains six combos. Individual weights within a cell stay
distinct; a display must decide how to present that detail.

`CardSet::new` rejects duplicates. `Range::without_cards` preserves all surviving
weights and zeroes overlapping combos. One dead card leaves 1275 possible combos;
a five-card board leaves 1081. Empty ranges and empty card sets are valid data.
A solve builder must separately reject an empty compatible joint range.

## Supported range text

Text uses uppercase ranks `23456789TJQKA` and lowercase suits `cdhs`.
Class ranks are descending: `AK` is accepted and `KA` is rejected.
Commas and ASCII whitespace separate expressions. Leading, trailing, or repeated
commas are errors; empty or ASCII-whitespace-only text represents an empty range.

| Expression | Expansion |
|---|---|
| `AA` | Six ace pairs |
| `AKs`, `AKo`, `AK` | Four suited, twelve offsuit, or all sixteen ace-king combos |
| `AsKh` | One physical combo |
| `99+` | `99 TT JJ QQ KK AA` |
| `ATs+` | `ATs AJs AQs AKs`, keeping the ace fixed |
| `99-JJ` | `99 TT JJ` |
| `A2s-A5s` | `A2s A3s A4s A5s`, keeping the ace fixed |
| `65s-T9s` | `65s 76s 87s 98s T9s`, keeping the rank gap fixed |
| `AKs:0.25` | Every suited ace-king combo with inclusion weight 0.25 |

Interval endpoints may appear in either order. Nonpair suffixes must match at
both ends; an omitted suffix means both suited and offsuit combos.
The parser rejects pair suffixes, mixed interval shapes, explicit-combo
intervals, percentages, nonfinite weights, and weights outside `[0,1]`.
This is a documented Pio-style subset, not complete PioViewer compatibility.

Overlapping equal assignments are idempotent. `AK:0.5,AKs:0.5` is valid;
`AK:0.5,AsKs:0.25` reports the conflicting token, physical combo, and both weights.
An explicitly assigned zero participates in that check. Use `set_weight` for an
intentional editor override; invalid edits leave the range unchanged.
Both signs of zero are stored as positive zero.

`Display` and `to_canonical_string` list only nonzero explicit combos in ID order.
Unit weights omit their suffix. Other weights use whichever roundtrippable
decimal or scientific representation is shorter. This preserves the smallest
positive `f64` without producing hundreds of decimal zeroes per combo.
Canonical text preserves weights, not the original shorthand or spacing.

Parsing accepts at most 131072 UTF-8 bytes and 4096 tokens. Both limits are checked
before expression expansion. Canonical text for every checked range fits these
limits, including a full range of subnormal weights.

## Validation

`evaluate_five` and `evaluate_seven` accept fixed-size arrays of distinct cards.
`evaluate_holdem` accepts five board cards and a checked hole combo.
`RiverEvaluator` caches an immutable board prefix for repeated hole evaluations.
Every path rejects board/hole overlap. `HandValue` is opaque and ordered from
weakest to strongest, with equality for ties; `category()` returns its hand class.
Vendor scores are not a serialized format and cannot construct a `HandValue`.

The private backend is `rs_poker` 5.1.0 with defaults disabled. Its suit IDs differ
from ours, so the adapter maps named rank and suit variants. Its static tables
occupy 312320 bytes, with no runtime initialization or table file. Preserve the
[upstream notices](../../docs/licenses/README.md) in distributions. Selection and
rejected-candidate findings are in the
[evaluator review notes](../../docs/astra/phase-2/evaluator-selection.md).

The crate has no dependency for card identity or range parsing. Run its checks
from the workspace root:

```text
cargo fmt --all -- --check
cargo clippy -p cards --all-targets --locked -- -D warnings
cargo test -p cards --locked
```

Tests exhaust all card IDs, unordered pairs, rejected ID values, and all 169 grid
cells. Parser checks cover class expansion, intervals, duplicate assignments,
input limits, malformed text, invalid weights, and dead-card filtering.
One hundred deterministic random ranges roundtrip every weight by its exact
`f64` bits; dedicated cases cover the smallest positive subnormal and the
smallest positive normal value. No external solver's parser serves as the oracle.

Smart App Control stays enabled on the development PC. Rust compilation and
execution run in GitHub Actions; a source review is not a passing test result.

Evaluator tests independently classify every five-card hand by category and
ordered kickers. They verify all 2598960 hands and 7462 strength classes.
The mandatory seven-card test checks ten million deterministic hands, each
against all 21 five-card subsets. Cached-board tests cover flush, paired, trips,
straight, and board-playing hands. Timings print with `-- --nocapture`; the test
profile uses optimization level 2 with overflow and debug checks enabled.
