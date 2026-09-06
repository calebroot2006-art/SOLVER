# bots

Bot policies: range charts preflop, the solved-spot library postflop with
pseudo-harmonic translation for off-menu sizes, and a bounded live re-solve when
the library has no entry.

Phase 8 builds it against a coverage table that says, for every situation, which
policy the bot used and whether a solver grade is allowed. A three-handed
postflop decision is a labelled fallback and produces no solver grade until a
validated multiway model exists.

Status: phase 0 skeleton. Phase 8 builds the policy plumbing and the live re-solve path.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p bots --locked
```

## Test

```
cargo test -p bots --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
