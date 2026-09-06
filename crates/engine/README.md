# engine

The hand engine: two to nine seats, blinds and antes, straddles and dead blinds,
betting rounds, all-ins with side pots, showdown and chop, blind schedules,
single-table tournaments, and hand-history export.

Phase 6 builds it and checks it by replaying PokerKit's published hand fixtures
for identical pot distributions, with property tests that chips are conserved in
every hand. It is independent of the solver track and runs in parallel with it.

Status: phase 0 skeleton. Phase 6 builds the engine and its PokerKit fixture replay.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p engine --locked
```

## Test

```
cargo test -p engine --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
