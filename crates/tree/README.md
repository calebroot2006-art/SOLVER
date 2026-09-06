# Postflop betting trees

`tree` builds immutable heads-up betting trees from explicit whole-chip settings.
Player zero acts first with no outstanding wager. There is no rake, no side pot,
and no translation of actions outside the configured menu. The crate uses only
the Rust standard library.

There are two trees. `RiverTree` models a single river: it has no chance nodes
and its wager amounts are contributions on that river. `PostflopTree` models a
flop, turn, or river tree: its wager amounts are contributions since the root,
its size menus are configured per street, and a `Chance` node stands for a
street transition. The river tree is frozen as the phase 3 regression baseline,
so a river-start `PostflopTree` is checked against it node for node.

`RiverTree::new` takes a `RiverTreeConfig` containing the root pot, effective
stack, minimum bet, both players' size menus, raise cap, all-in thresholds, and
node budget. It returns the entire checked tree or an error. Configuration has
no defaults. Nodes expose read-only kinds, contributions, actions, and child IDs.

## Size menus

Construct menus with `BetSizeOptions::try_from((bets, raises))` or checked
`BetSizeOptions::new` vectors. Both routes apply the same validation.

| Token | Meaning |
| --- | --- |
| `50%` | Half the pot; for a raise, half the pot after calling, added to the highest contribution |
| `2.5x` | Raise to 2.5 times the highest contribution; raise menus only |
| `20c` | Bet 20 chips, or raise 20 chips above the highest contribution |
| `a` | All-in |
| `e` | Geometric size, which is all-in on the river |

Commas separate entries; surrounding ASCII whitespace is accepted. Empty menus
are valid. Internal whitespace, bare numbers, non-ASCII syntax, complex geometric
sizes, per-option raise caps, nonfinite values, and nonpositive sizes are errors.
Multipliers must exceed one. Each text menu is bounded to 4096 bytes and each
menu to 64 entries before deduplication. Additive chip sizes are at most one
billion. Identical size options and final targets are deduplicated.

## Betting and accounting

Wager actions contain total river contributions, not incremental payments.
For a root pot `P`, highest contribution `b`, and outstanding call `d`, percentage
raises use the pot after calling, `P + 2*b`. Float calculations round nonnegative
halfway values upward, then clamp to the minimum legal target and effective stack.
The minimum raise target is `b + max(d, min_bet)`. A shorter effective all-in is
legal. A target at the effective stack becomes `Action::AllIn`.

An unopened node offers check and its configured bets. Two checks reach showdown.
Facing a wager always permits fold and call, including against an all-in. A call
ends the river. A fold refunds the winner's uncalled excess: both terminal river
contributions equal the folder's contribution. `Terminal::Fold` names the winner.
These contributions exclude the existing root pot; the solver supplies its
documented payoff baseline when converting them to net chip utilities.

The raise cap counts raises after the initial bet. Cap zero still allows bets
and fold/call responses. Once either player is all-in, no raise is offered.
Actions sort as fold, check, call, increasing bets, increasing raises, then all-in.
Distinct histories have distinct node IDs, assigned in depth-first order.

For a candidate target `a`, the force threshold replaces it with all-in when
`S-a <= round((P+2*a)*force_threshold)`. The add threshold introduces an extra
all-in when `S <= round(P*add_threshold)` unopened, or
`S <= b+round((P+2*b)*add_threshold)` facing a wager. Raise caps also prohibit
threshold-added raises. With zero thresholds, no extra wager is introduced.

## Resource and numerical bounds

Chip settings are at most one billion; pot and minimum bet must be positive.
Stack zero produces a single showdown terminal. The raise cap is at most 32,
the node budget is between 1 and 1000000, and construction refuses depth above
128 edges. Thresholds must be finite and nonnegative. Nonfinite intermediate
arithmetic is rejected before float-to-chip conversion. Large finite targets
are clamped before conversion so they cannot saturate through a cast.

Node and edge budgets are checked before their buffers grow, and construction
uses fallible vector reservations. `storage_bytes()` includes inline storage,
actual node/action/child capacities, and retained configuration menu buffers.
It excludes temporary construction storage and caller-owned objects.

## Postflop trees

`PostflopTree::new` takes a `PostflopTreeConfig`: the river config's fields plus
`start_street` and `sizes[street][player]`, indexed by `Street::index`. Menus for
streets before `start_street` are never read. Rules, syntax, rounding,
thresholds, and the raise cap are the river's, applied once per street, so a
config with `start_street: Street::River` builds the same tree `RiverTree` builds
from the same settings, node for node.

What changes between streets:

* `effective_stack` is what a player can commit across every remaining street,
  and `contributions()` counts chips committed since the root. No contribution
  ever exceeds the stack.
* `min_bet` and `max_raises` apply per street, measured from the level both
  players carried into it. A `2.5x` raise scales this street's wager, not the
  whole commitment: after a turn bet to 10 is called, a river bet to 25 is a
  15-chip wager, and raising it `2.5x` reaches 48, not 63.
* Percent sizes read the pot at the acting node, which already includes the
  chips both players committed earlier, so the existing formula `P + 2*b` holds
  unchanged with `b` the highest contribution since the root.

### The chance convention

Betting ends a street on a call or a second check. Before the river that
produces a `Chance` node: no actions, exactly one child, and that child is the
next street's root with both players at the same contribution. `Chance { next }`
names the street the child belongs to, while `node.street()` names the street
whose betting just ended.

One chance node stands for every runout of the deal. The tree does not know how
many cards there are, and never enumerates them; the solver expands a chance
node into its runouts and copies the subtree below it. This keeps a flop tree in
the low thousands of nodes instead of the low millions.

A call that puts both players all in is a street transition like any other, so
the remaining streets appear as chance nodes leading to one `Showdown` terminal,
with no decision node between them. An effective stack of zero does the same
thing from the root.

### Counting a street

`decision_nodes_per_street()` and `live_continuations_per_street()` return
`[usize; 3]`, indexed by `Street::index`, and feed the solver's memory estimate.

A live continuation is a history that finishes a street with both players
contesting the pot and chips still behind. Before the river each one is a chance
node, so the count is the branching factor into the next street's betting; on
the river each one is a showdown that no called all-in produced. Lines where a
call put both players all in are excluded, because they run the board out with
no further decision.

Take a street with two non-all-in bet sizes, one non-all-in raise, an all-in in
both menus, and one raise allowed. It has 16 decision nodes: 2 unopened, 2 x 3
facing a bet, and 2 x 2 x 2 facing a raise. It has 9 live continuations:
check-check, plus a bet-call and a raise-call for each side and each size. The
phase 4 plan's memory table used 18 as its decision-node anchor, so that table
is conservative by an eighth.

The counts are not uniform across a tree. On a deep street a raise target can
clamp to the stack and merge into the all-in, which costs a decision node and a
continuation. A memory estimate has to read the built tree instead of
multiplying one block's anchors.

## Verification

From the workspace root, the hosted Rust checks are:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p tree --locked
cargo test --workspace --locked
```

`tests/river.rs` checks menu validation, exact rounded targets, player-specific
menus, minimum raises, check/check and bet/call histories, fold refunds, all-in
responses, thresholds, raise caps, and exact node-budget boundaries. It replays
144 small configurations independently from action histories to check legality,
chip totals, unique reachability, contributions, terminal outcomes, and depth.

`tests/postflop.rs` adds the street-aware tree. The load-bearing check is river
equivalence: on the three phase 3 reference fixtures and on 60 more river
configurations, a river-start `PostflopTree` matches `RiverTree` node for node.
That is what catches drift between the two copies of the arithmetic helpers.

The rest covers chance-node shape, the all-in runout, a zero stack from every
street, per-street menus, street-relative raise multipliers, the node and depth
budgets, configuration and syntax rejection, and the per-street counters. It
replays 288 more configurations through the audit the river suite uses, extended
to follow deals, recompute both counters, and check that no contribution exceeds
the effective stack.
The implementation awaits hosted Rust verification; no local Rust compiler,
formatter, or linter was run because Smart App Control remains enabled.
