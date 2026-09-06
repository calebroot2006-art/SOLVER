# postflop

The solver core: vanilla CFR, CFR+, and Discounted CFR(1.5, 0, 2) behind one
`Solver` trait, over a public-state tree.

Phase 1 builds the `Game` trait, the three CFR variants, the driver with its
progress logging and non-finite checks, and the best-response calculator used
for the toy games. Phase 3 replaces the toy best response with the `bestresponse`
crate and adds the river tree. Phase 4 adds chance nodes, suit isomorphism,
parallel runouts, and 16-bit compression measured against the f64 baseline.

Numbers from this crate carry units. A solve reports its exploitability and its
stop reason; the README gains a numerical-layout section in phase 1 that records
the precision, the regret storage rule, and the normalisation.

Status: phase 0 skeleton. Phase 1 builds the CFR core here; phases 3 and 4 grow it into the real solver.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p postflop --locked
```

## Test

```
cargo test -p postflop --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
