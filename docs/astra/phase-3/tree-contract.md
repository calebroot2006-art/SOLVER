# River betting tree contract

Executor scope: `crates/tree/src/**`, `crates/tree/tests/**`, and
`crates/tree/README.md` in its isolated worktree. Astra owns manifests, integration,
solver code and CI. Use only the standard library; no local Rust execution.

## Public data and construction

- `pub type Chips = u64`, `pub type NodeId = u32`.
- `BetSize` is a public enum: `Pot(f64)` (fraction, 0.5 means half pot),
  `PreviousBet(f64)` (raise-to multiplier), `Additive(Chips)`, `AllIn`.
- `BetSizeOptions` stores private bet/raise vectors, exposes `bets()` and
  `raises()`, and implements `TryFrom<(&str, &str)>` plus checked
  `new(Vec<BetSize>, Vec<BetSize>)`. Both paths enforce the same limits.
- `RiverTreeConfig` is a public struct with mandatory `starting_pot: Chips`,
  `effective_stack: Chips`, `min_bet: Chips`, `sizes: [BetSizeOptions; 2]`,
  `max_raises: u8`, `add_all_in_threshold: f64`,
  `force_all_in_threshold: f64`, and `max_nodes: usize`. No `Default`.
- `Action` is a public ordered enum in this declaration order: `Fold`, `Check`,
  `Call`, `Bet(Chips)`, `Raise(Chips)`, `AllIn(Chips)`. Wager amounts are the
  player's total contribution on this river, not chips added by this action.
  Display renders `fold`, `check`, `call`, `bet:<n>`, `raise:<n>`, `allin:<n>`.
- `Terminal` is `Fold { winner: u8 }` or `Showdown`.
- `RiverNodeKind` is `Decision { player: u8 }` or `Terminal(Terminal)`.
- `RiverNode` has private fields and read-only methods `kind()`,
  `contributions() -> [Chips;2]`, `actions() -> &[Action]`,
  `children() -> &[NodeId]`. Terminal contributions record chips actually retained
  in the pot after any refund. Terminal nodes have no actions/children.
- `RiverTree::new(RiverTreeConfig) -> Result<Self, TreeError>` owns validated
  config and nodes. Read-only `config()`, `root()`, `nodes()`, `node(NodeId)`
  (returns `Option<&RiverNode>`), `max_depth()`, and `storage_bytes()`.
- `TreeError` implements Debug, Display, Error; descriptive errors are sufficient.
  All public items must have rustdoc. No mutable tree or unchecked constructor.

## Syntax and limits

Implement the explicit river subset: `50%`, `2.5x`, `20c`, `a`, and `e`. Plain
`e` means geometric and equals all-in on the river. `x` is accepted only in raise
menus. `c` is an initial bet amount or an additive raise increment. Reject bare
numbers, complex geometric syntax and per-option raise caps in this version;
document unsupported tokens. The global raise cap is explicit configuration.
Empty menu text is legal. Comma-separated tokens may have surrounding ASCII
whitespace; reject empty comma items, internal whitespace, non-ASCII syntax,
nonfinite/nonpositive sizes, and multipliers at most one. Bound each input to
4096 bytes and each menu to 64 entries before expansion. Validate enum inputs too.

Validate pot and minimum bet as positive. Effective stack may be zero for a
terminal-only river. All chip settings must be at most 1000000000, keeping sums
and conversions exact. `max_raises <= 32`; `1 <= max_nodes <= 1000000`.
Thresholds must be finite and nonnegative. Percentages/multipliers may be large
finite values, but any arithmetic overflow must return an error before conversion.
Deduplicate identical size options and final action targets. Limits apply before
deduplication, preventing oversized input from evading the parser bound.

## Betting rules

Root: player 0 (out of position), zero river contributions, no outstanding wager.
If effective stack is zero, root is a showdown terminal. Otherwise an unopened
decision offers check and configured bets; two checks end in showdown.
Facing a wager offers fold and call even when the wager is all-in. No further
raise is legal once either player has reached the effective stack. A call equalizes
contributions and ends the river. A fold returns the winner's uncalled excess,
so both terminal contributions equal the folder's committed river contribution.
Winner/folder identity is explicit; never infer it from hand strength.

For highest contribution `b`, acting contribution `x`, call `d=b-x`, root pot `P`,
and effective stack `S`, the pot after calling is `C=P+2*b`:

- Unopened percent bet: `round(P*r)`; additive bet: its chip amount.
- Percent raise-to: `b + round(C*r)`.
- Multiplier raise-to: `round(b*r)`.
- Additive raise-to: `b + increment`.
- All-in target: `S`.

Use nonnegative round-to-nearest with halves upward, matching Rust `f64::round`.
Reject nonfinite intermediate arithmetic. Clamp initial bet targets to
`[min(min_bet,S), S]`; clamp raises to `[min(b+max(d,min_bet),S), S]`.
An effective all-in below the full minimum is legal and still gets a response.
Discard targets that do not exceed `b`. Convert targets equal to `S` to a single
`AllIn(S)` action. Sort by the declared Action order and deduplicate.

If a candidate target `a` leaves `S-a <= round((P+2*a)*force_threshold)`, replace
it with `S`. Add an extra all-in when `S <= round(P*add_threshold)` at an unopened
node, or `S <= b+round(C*add_threshold)` facing a wager. With both thresholds zero,
no extra wager is introduced and only an already-all-in target is merged.

`max_raises` counts raises after the initial bet; zero still permits an initial
bet and the subsequent fold/call decision. At the cap, offer no raises, including
threshold-added all-ins. A check does not count as a raise.

Build a real tree with unique node IDs for distinct histories. Bound allocations
before adding nodes/edges; use checked arithmetic and `try_reserve` with descriptive
errors. Exhausted `max_nodes` returns an error without a partial tree. Bound depth
at 128 before recursing. Every child ID is valid, every node reachable exactly
once, and `actions.len()==children.len()`. Account for actual vector capacities
and config menu buffers in `storage_bytes()`.

## Verification

Write meaningful integration tests for parser failures and constructor parity.
Cover
exact percent/multiplier/additive amounts; deterministic deduplication;
check/check, bet/call, check/bet/fold, multi-raise and minimum-raise histories;
all-in fold/call including a short initial all-in; cap zero and cap exhaustion;
refund chip accounting; terminal-only stack zero; and overflow/resource errors.
Walk a collection of small configurations independently to verify all histories,
action legality, contributions, cap/depth, terminal state, and chip conservation.
Do not lower numerical or existing workspace gates. Root runs GitHub Actions.

The pot-after-call, raise-to and threshold conventions were inspected in the
[pinned reference tree](https://raw.githubusercontent.com/b-inary/postflop-solver/9d1509fe5077d019825f833eed04b16d342dfda1/src/action_tree.rs).
Minimum bet and a global raise cap are explicit project controls; reference
fixtures must match them, with exact tree comparison before strategy comparison.
Implement from this contract and poker rules; copy no upstream implementation.
