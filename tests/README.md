# toygames

Kuhn and Leduc poker, a scalar per-history CFR that acts as an oracle for the
vector-form solver, and the tests that compare our convergence curves against
captured OpenSpiel runs. The directory is `tests` because `docs/ROADMAP.md` names it
that; the package is `toygames` so a command line says what it runs.

These are the known-solution tests `CLAUDE.md` requires before any CFR change ships.
A solver that passes them is not proved correct on hold'em, but a solver that fails
them is wrong.

Status: phase 0 skeleton. Phase 1 fills it in.

## What phase 1 adds

* `src/kuhn.rs` and `src/leduc.rs`: the two games in the same vector form over a
  public tree that `crates/postflop` uses, with OpenSpiel's rules and explicit game
  parameters (`players=2, suit_isomorphism=false` for Leduc).
* `src/history_oracle.rs`: a small scalar per-history CFR and best response over
  explicit deals. It is slow and obviously correct, so it is the reference the
  vector form is checked against on unequal weights, sparse ranges, blocked boards,
  folds, and ties.
* `src/nan_game.rs`: a two-node game whose terminal returns NaN, so the failure path
  has a test.
* `reference/openspiel/`: `capture.py`, its pinned `requirements.txt`, the captured
  JSON curves, and `provenance.json` recording the OpenSpiel version, Python version,
  date, game strings, solver options, and what one iteration means.
* `tests/`: the integration tests and the fixture files holding each variant's
  iteration budget, recorded from the captured curves rather than guessed.

## Run

There is nothing to run on its own: this is a library crate plus its integration
tests. Build it from the workspace root.

```
cargo build -p toygames --locked
```

## Test

```
cargo test -p toygames --locked
```

From phase 1, with the convergence checkpoints printed:

```
cargo test -p toygames --locked -- --nocapture
```

The workspace gate runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo test --workspace --locked`. See the root README for the pinned toolchain.
