# bestresponse

The best-response calculator over the public-state tree, and the exploitability
metric built from it.

The top-level functions support the audited legacy `Game` and `Strategy` API.
`river::{expected_value, best_response, exploitability}` accepts an immutable
`RiverStrategy`; its retained game determines ranges, tree and terminal payoffs.
The river functions honor that game's shared memory budget.

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
