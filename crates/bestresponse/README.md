# bestresponse

The best-response calculator over the public-state tree, and the exploitability
metric built from it.

The top-level functions support the audited legacy `Game` and `Strategy` API.
`river::{expected_value, best_response, exploitability}` accepts an immutable
`RiverStrategy`; its retained game determines ranges, tree and terminal payoffs.
The river functions honor that game's shared memory budget.

`streets::{expected_value, best_response, exploitability}` is the same three
functions over a `PostflopStrategy`, the street-aware policy from
`postflop::streets`. Its game spans more than one street, so the walk covers every
dealt runout. It walks them in f64 whatever width the solve stored its accumulators
at, so the number measures the whole expanded tree rather than a sampled or merged
part of it. Like the river functions, these charge the retained game's shared memory
budget: a query refuses rather than exceeding the configured limit.

The traversal remains in the shared `postflop` numerical core so CFR and metric
queries use one binding and terminal boundary. This crate provides public metric
entry points without duplicating the implementation or introducing a dependency
cycle. Phase 3's [plan](../../docs/astra/phase-3/PLAN.md) records verification status.

Best response maximizes per own private hand and public history. It never chooses
an action after seeing the opponent's hand. The certificate reports both best
responses, their sum as NashConv, half that sum in chips per hand, and
`50 * NashConv / starting_pot` as a percentage. Its scope is the configured tree,
ranges and zero-sum payoffs. It does not supply a per-decision error bound.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p bestresponse --locked
```

## Test

```
cargo test -p bestresponse --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
