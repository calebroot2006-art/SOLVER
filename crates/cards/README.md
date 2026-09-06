# cards

Card and combo types, the Pio range-string parser and printer, the 13x13 grid
mapping, card removal, and 7-card hand evaluation.

Phase 2 builds it, together with the Johanson three-bucket showdown sweep and
the brute-force reference the evaluator is property-tested against. Range
strings must round-trip through the parser and printer unchanged.

Status: phase 0 skeleton. Phase 2 builds the card types, the range parser, and the 7-card evaluator.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p cards --locked
```

## Test

```
cargo test -p cards --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
