# bestresponse

The best-response calculator over the public-state tree, and the exploitability
metric built from it.

Phase 1 keeps a toy-game best response inside `postflop` so the CFR variants can
be validated on Kuhn and Leduc. Phase 3 moves the calculator here once the real
public-state tree exists, and reports exploitability as a percentage of the fixed
root pot. The percentage is a property of the solved tree, not a per-decision
error bar.

Status: phase 0 skeleton. Phase 3 takes the calculator over from postflop and runs it on the real tree.

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
