# payoff

Terminal payoff models. This is the bottom of the solver stack and depends on
nothing else in the workspace, so the scalar type every other crate accumulates
in can live here without a cycle.

Phase 1 adds `pub type Real = f64`, the `Payoff` trait, and `ChipEv`, whose
utilities sum to zero across both players. Phase 10 adds the Malmuth-Harville
ICM implementation, satellite and bounty payoffs. A payoff that does not sum to
zero needs its own exploitability metric; that boundary is stated on the trait
rather than left to the caller.

Status: phase 0 skeleton. Phase 1 puts the `Real` alias and the `Payoff` trait here; phase 10 adds ICM.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p payoff --locked
```

## Test

```
cargo test -p payoff --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
