# tree

The bet-size DSL, the action tree it builds, the raise cap and all-in threshold,
and pseudo-harmonic translation from an off-menu bet size to a size the tree
contains.

Phase 3 builds the DSL and the tree for the river solver. Phase 8 uses the
translator so a bot can answer a bet size nobody solved for, with the
approximation labelled rather than hidden.

Status: phase 0 skeleton. Phase 3 builds the bet-size DSL and the action tree; phase 8 uses the translator.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p tree --locked
```

## Test

```
cargo test -p tree --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
