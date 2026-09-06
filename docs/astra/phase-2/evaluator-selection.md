# Evaluator selection

`rs_poker` 5.1.0 passed the phase 2 acceptance gates. Its direct dependency disables
default features and uses an exact version requirement. The checked adapter maps
rank and suit names explicitly and rejects duplicate cards before vendor calls.
Raw vendor strengths stay private; equality means a tie and larger means stronger.

The published crate's parser accepts trailing text and its numeric conversions
clamp invalid input. Neither is used by the adapter. Its evaluator also assumes
valid distinct cards. `Card`, `Combo`, and `CardSet` enforce that boundary.
The generated lookup tables occupy 312320 bytes. The build script uses only local
standard-library computation and writes to Cargo's output directory. Evaluation
requires no runtime initialization, allocation, network, or table file.
These observations come from the checksum-verified
[published 5.1.0 archive](https://static.crates.io/crates/rs_poker/rs_poker-5.1.0.crate).

The dependency contains unsafe code outside the selected evaluation path:
clamped enum conversions and a conditional BMI2 intrinsic. The project does not
enable CPU-specific target features. Static inspection does not certify the
whole dependency as memory-safe. Correctness, timing, and portability require CI.
Preserve the [Apache and ISC notices](../../licenses/README.md) in distributions.

## Rejected candidate

The exact `deuce` 1.1.0 archive fails source inspection before a speed comparison.
Its default `Ranking` enum derives ordering with `FullHouse` before `Flush`, so
the latter compares stronger. Its flush ranking stores only the highest rank and
requests zero kickers, tying ace-high flushes whose second card differs.
For example, `As Ks 9s 6s 2s 3h 4d` must beat `As Qs 9s 6s 2s 3h 4d`.
The relevant files are `src/{ranking,evaluator,strength}.rs` in the
[published 1.1.0 archive](https://static.crates.io/crates/deuce/deuce-1.1.0.crate),
SHA-256 `b2d0f690f3d4bae6ae449c45f26fd2873ce73bbb9d168147fa76763397d539ed`.

These are static findings, without a compiled deuce reproduction. We do not add
the rejected dependency to the project or claim a measured speed comparison.
Our regression fixtures cover both hand-ordering cases independently.

## Validation gates

The test oracle classifies five cards by rank counts and ordered kicker tuples.
It contains no vendor evaluation call. Exhaustive comparison checks all 2598960
hands, category totals, 7462 strength classes, ties, and ordering. Each of ten
million deterministic seven-card samples checks the maximum over its 21 subsets.
Both Windows and Linux must complete these mandatory tests.

Cached-board checks cover mixed, flush, paired, trips, straight, and royal boards.
Timing reports include checked seven-card calls, cached-board calls, rebuilt-board
calls, platform, test optimization level, and storage. They are runner measurements
without a speed threshold or a claim about Caleb's consumer hardware.

The [acceptance review](../../reviews/2026-09-06-astra-phase-2-acceptance.md)
records the CI commit, results, timings, and resolved dependency audit.
