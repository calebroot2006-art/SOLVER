# preflop

External-sampling MCCFR over the multiway preflop tree with bucketed postflop
rollouts, and the chart generator built on top of it.

Phase 11 builds it: equity-histogram bucketing with earth-mover k-means, the
sampler, then charts for 6, 8, and 9-max at chosen stack depths. It is the
hardest research problem in the repo, so it gets its own research pass before a
line is written. Until then the app's chart viewer stays dark rather than showing
ranges we did not solve.

Status: phase 0 skeleton. Phase 11 builds the bucketing, the MCCFR, and the chart generator.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p preflop --locked
```

## Test

```
cargo test -p preflop --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
