# River betting trees

`tree` builds an immutable heads-up river tree from explicit whole-chip settings.
Player zero acts first with no outstanding wager. There are no chance streets,
rake, side pots, or translations of actions outside the configured menu.
The crate uses only the Rust standard library.

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
The implementation awaits hosted Rust verification; no local Rust compiler,
formatter, or linter was run because Smart App Control remains enabled.
