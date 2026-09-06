# OpenSpiel reference

The Python side of the toy-game gate: captured OpenSpiel 2.0.2 curves, the
scripts that produced them, and the verifier CI runs against the Rust solver's
exported snapshots. `../../README.md` (the `toygames` package) explains what each
check proves and how the budgets were chosen; this file is the operating manual
for the folder.

## What is here

| File | Role |
|---|---|
| `requirements.txt` | The pinned reference runtime, installed with `--only-binary=:all:` |
| `capture.py` | Records `nash_conv` and player 0's value at fixed checkpoints for one game and variant |
| `dcfr_reference.py` | The one definition of the project's DCFR discount; every script imports it |
| `export_fixtures.py` | Turns completed captures into `tests/fixtures/*.toml` and `provenance.json`, refusing incomplete or off-target runs |
| `sensitivity.py` | OpenSpiel-only controls that change accumulation order; their outputs live in `diagnostics/` |
| `verify_snapshots.py` | Imports a Rust snapshot into OpenSpiel, advances one iteration, and compares; also evaluates the actual Rust policies |
| `test_verify_snapshots.py` | Mutation tests for the verifier's rejection paths |
| `test_dcfr_reference.py` | Checks early DCFR updates, per-action averages, scaled sums, mutations, and capture regression |
| `*.json` | The captures and their provenance |

## Run

From the workspace root on Windows, with the venv described in `../../README.md`:

```text
.venv/Scripts/python -m unittest discover -s tests/reference/openspiel -p "test_*.py"
.venv/Scripts/python tests/reference/openspiel/verify_snapshots.py target/cfr-traces --output target/reference-verification.json
```

The verifier needs the snapshots `cargo test -p toygames` writes when
`ASTRA_CFR_TRACE_DIR` is set; on this PC that means downloading the
`cfr-traces-<os>` artifact from a CI run, because the Rust compiler cannot start
here. The unit tests need no Rust output at all.

## DCFR comparison scope

The production reference remains OpenSpiel's `CFRSolver` plus the project's
whole-accumulator discount. It is independently compared with OpenSpiel 2.0.2's
`discounted_cfr.DCFRSolver` through 30 Kuhn iterations and five Leduc iterations.
Tests compare signed regrets, current and average probabilities for every legal
action, and averaging sums. After iteration T, the project's sum equals the
upstream sum divided by `(T + 1)^2` in exact arithmetic. Normalization removes
that common factor. Later floating-point trajectories can diverge, so the existing
common-state snapshot verifier remains the long-run acceptance check.

The upstream `discounted_cfr.py` canonical LF hash is pinned in the test. That
module warns that OpenSpiel has not verified reproduction of the paper's results;
agreement here is scoped to the measured implementation comparisons.
[Pinned upstream source](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/open_spiel/python/algorithms/discounted_cfr.py).

The seven DCFR tests include intentional average-policy and discount mutations.
They also run all six capture paths through iteration five and require exact
agreement with the historical checkpoints. Together with the 11 snapshot-verifier
tests, the local suite has 18 tests.

## Regenerate

The capture commands and their run times are in `../../README.md`. After a
capture completes, `export_fixtures.py` rewrites the fixtures and `provenance.json`;
commit both with the capture. Format and lint with the pinned tools before
committing:

```text
.venv/Scripts/python -m black tests/reference/openspiel
.venv/Scripts/python -m ruff check tests/reference/openspiel
```

Use `capture.py --output target/my-capture.json` for an isolated short run; without
that option it writes the named capture in this folder. Existing capture/control
JSON and their provenance entries retain the original executed source hashes.
The manifest's current reproducer hashes are updated separately. New captures record both
`capture.py` and `dcfr_reference.py`; sensitivity and verifier outputs also record
the helper. The exporter records current reproducer hashes separately from each
historically executed source. Extraction of the helper did not regenerate the
committed fixtures or change their budgets.
