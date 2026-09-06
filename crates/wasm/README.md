# wasm

The wasm-bindgen wrapper around the solver core.

A browser build is a phase 13 goal. The crate exists from phase 0 so that the
solver core never quietly picks up a dependency that cannot cross to
`wasm32-unknown-unknown`. Until the wrapper is written it holds nothing but the
skeleton below.

Status: phase 0 skeleton. Phase 13 ships a browser build; until then this crate only has to keep compiling.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p wasm --locked
```

## Test

```
cargo test -p wasm --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
