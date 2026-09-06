# spots

The solved-spot and range-chart file formats, the library index, and the CLI that
generates a library.

This crate is the contract between the solver and the app. A solved spot carries
its own provenance: the bet menu, the ranges, the root history, the payoff model,
the units, the residual exploitability, and the stop reason. Any grade built from
it shows that provenance. Phase 5 fixes the format and generates the first
25-flop library end to end.

Status: phase 0 skeleton. Phase 5 fixes the formats and builds the library generator.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p spots --locked
```

## Test

```
cargo test -p spots --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
