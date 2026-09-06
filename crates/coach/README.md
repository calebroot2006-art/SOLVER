# coach

Grading, the explanation fact builder, the validated template library, the
language-model client, and the leak tracker.

Every poker claim the user reads is rendered from validated data computed in
Rust. The model selects and orders approved fact and template IDs; it never
writes a frequency or an EV of its own, and a template fallback always exists.
Phase 9 builds it, with adversarial fixtures for equal-EV mixes, swapped actions,
unit confusion, stale solves, and prompt text embedded in hand data.

Status: phase 0 skeleton. Phase 9 builds the grader, the fact builder, the templates, and the model client.

## Run

There is nothing to run on its own: this is a library crate. Build it from the
workspace root.

```
cargo build -p coach --locked
```

## Test

```
cargo test -p coach --locked
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
