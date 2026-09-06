# toygames

These tests check the public-tree CFR core against independent poker histories
and captured OpenSpiel results. It covers two-player Kuhn and physical-card Leduc.
Passing these checks establishes evidence for these games and weighted-range
fixtures; hold'em remains a later phase with its own validation.

## Run the Rust checks

From the workspace root:

```text
cargo test -p toygames --locked -- --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
```

The workspace test profile uses optimization while retaining overflow and debug
assertions. The Kuhn accuracy tests require up to 200,000 iterations; all three
Leduc variants run through the 10,000-iteration reference checkpoint. No numerical
test is ignored. On Caleb's Windows PC Smart App Control prevents the Rust compiler
from starting, so Astra runs these commands in the approved GitHub Actions workflow.
Python reference capture runs locally without changing Smart App Control.

## Game and probability contract

Kuhn has three distinct cards, one-chip antes and one possible one-chip bet.
Leduc has six physical cards: states 0/1 are jacks, 2/3 queens and 4/5 kings.
It uses one-chip antes, raises of two chips before the board and four afterwards,
and at most two raises per round. Player 0 starts each round. Folding is legal
only while facing a bet. Two checks, or a call after a raise, end a round.
These are OpenSpiel's `players=2,suit_isomorphism=false,starting_player=0,
action_mapping=false` Leduc rules. [Rule source](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/open_spiel/games/leduc_poker/leduc_poker.cc).

The public tree has three or six private-state rows. Private weights are
unnormalized; equal physical cards are incompatible. For each compatible pair,
four Leduc board cards are legal. The tree enumerates six outcomes at probability
1/4 and uses masks to remove the two private cards. Tests check that their
conditional chance mass sums to one for every compatible pair. They also check
936 legal Leduc information sets and 12 Kuhn information sets without consulting
a strategy's reach.

Expected values and best responses are chips per hand. For a two-player zero-sum
game, `nash_conv = br[0] + br[1]`, average exploitability is `nash_conv / 2`, and
`pct_of_pot = 100 * average / starting_pot`. Uniform Kuhn has best responses
`[1/2, 5/12]`, NashConv `11/12`, and percent-of-pot exploitability
`22.91666666666667` for the two-chip root pot. Uniform Leduc has NashConv
`4.747222222222222`. These constants are independently checked against
[OpenSpiel's tests](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/open_spiel/python/algorithms/exploitability_test.py).

## Independent checks

`src/toy.rs` builds the public tree and uses the chip payoff crate for terminal
values. `src/history_oracle.rs` separately expands explicit private deals and
legal public histories. Its action-string state machine and scalar terminal
payoffs do not call the public tree's child traversal, chance masks, terminal
evaluation or payoff crate. It only looks up strategy rows by public history.
Its best response aggregates hidden histories at an information set before
selecting an action, so it cannot choose differently after seeing the opponent's
card. A known Kuhn equilibrium has value -1/18 and zero NashConv in both paths.

The scalar CFR reference aggregates regret across explicit histories, updates
players alternately, and keeps signed regrets for vanilla/DCFR. It compares
current and average strategies over three iterations on uniform, unequal and
sparse ranges, including blocked boards. Value and best-response tests also
rescale both players' weights independently. Fold, tie, conflicting-card,
empty-range, invalid-weight, invalid-strategy, invalid-config, cyclic-tree,
nonpositive-pot and NaN-terminal cases have separate tests.

One early fixture exposed a roundoff-sensitive exact tie. On Leduc history
`cc/4/rr`, player 0 holding card 1 faces live opponent weights 0.4 on card 0 and
0.5 on card 3. Both fold and call have counterfactual value -0.1875. Normalizing
a private deal and then dividing the normalization back out produced tiny signed
regret in the scalar oracle. It now starts each explicit CFR deal with the raw
opponent weight, as the counterfactual definition requires. EV and best response
retain independent root normalization. Regret matching has a discontinuity at a
zero positive-regret sum; a strict policy comparison there requires examining
the actual regrets, not treating a frequency mismatch as proof of a solver defect.

## Reference captures and budgets

The committed JSON files were produced by the official OpenSpiel 2.0.2 Windows
CPython 3.12 wheel, running Python 3.12.10. `requirements.txt` pins its runtime
dependencies. `provenance.json` records hashes, game parameters, per-file commands,
capture timestamps, the executed upstream `cfr.py` hash and the final reproducer
script hash. The script gained a DCFR option and was formatted during earlier
captures; the reproducer hash does not claim those earlier processes executed
identical script bytes. Native CFR/Plus update calls stayed unchanged.

Vanilla and CFR+ use the unmodified Python `CFRSolver` and `CFRPlusSolver`.
One iteration updates player 0 then player 1. CFR+ floors accumulated regret at
information-set scope and uses linear iteration weighting. The DCFR captures
use OpenSpiel's scalar CFR traversal plus this project's small discount extension:
after each alternating iteration, discount all positive regrets by
`t^1.5/(t^1.5+1)`, negative regrets by 1/2 and the whole strategy accumulator by
`(t/(t+1))^2`. This is an additional independently traversed reference, not an
upstream OpenSpiel DCFR implementation. [CFR source](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/open_spiel/python/algorithms/cfr.py).

Budgets were selected after inspecting the captured residuals. Existing accuracy
targets were retained:

| Game | Variant | Fixed budget | Captured NashConv at budget | Required NashConv |
|---|---|---:|---:|---:|
| Kuhn | Vanilla | 10,000 | 2.2664891573703771e-4 | < 1e-3 |
| Kuhn | CFR+ | 200,000 | 2.6302653812759758e-6 | < 1e-5 |
| Kuhn | DCFR | 200,000 | 3.1177622322187126e-6 | < 1e-5 |
| Leduc | Vanilla | 10,000 | 4.084728965653733e-3 | < 5e-3 |
| Leduc | CFR+ | 1,000 | 5.143032323129126e-4 | < 1e-3 |
| Leduc | DCFR | 2,000 | 7.792069059131546e-5 | < 1e-4 |

The final original Leduc CFR+ reference at 10,000 iterations has player-0 value
`-0.08560634170621867` and its own residual `1.2912961660394018e-5`.
The value test allows the sum of this residual, the candidate's measured residual
and `1e-9`. It does not treat the reference as an exact equilibrium. Kuhn CFR+
and DCFR also require value within `1e-4` of -1/18 and DCFR's equilibrium
frequency relations within 0.02.

All 80 captured checkpoints remain in the fixtures and test output. Kuhn retains
the original `1e-9 + 1e-6*abs(reference)` comparison for every checkpoint,
including both 200,000-iteration endpoints. Leduc uses that same comparison
through iteration 50 for all three variants. Later trajectory differences remain
visible diagnostics. At every captured checkpoint at or after a variant's fixed
budget, its average NashConv must remain below the table's absolute target.
This allows local increases below the target; it does not assert monotonicity.

This change followed measured numerical sensitivity, not a relaxed threshold to
hide an unexplained mismatch. `diagnostics/` contains four OpenSpiel-only controls.
Reversing only chance enumeration gives CFR+ NashConv `0.01016600906510387` at
iteration 200 versus the original `0.00992625910098657`. Multiplying only root
counterfactual reach by 30 gives `0.01009236568360325`; uniform positive regret
scaling cancels in exact regret matching. Both controls preserve the game and
algorithm but change floating-point accumulation. Each result records its exact
executed script and upstream source hashes. Reproduce them with `sensitivity.py`
using `--reverse-chance` or `--root-counterfactual-scale 30`.

The replacement gate checks the same solver state, rather than requiring two
independently accumulated trajectories to remain identical. Rust exports current
policies, signed regrets, averaging accumulators, and current/average EVs, both
best-response values and NashConv. `verify_snapshots.py` imports a Rust state into
OpenSpiel, advances one alternating iteration and checks all entries. Regrets
and sums use `1e-12 + 1e-12*abs(reference)`; current probabilities use absolute
`1e-12`. It independently evaluates each actual Rust current/average policy and
compares all four metrics within absolute `1e-12`. Average accuracy targets also
apply to every exported snapshot at or after its budget.

The full capture requires 18 iterations for each variant (54 strategy snapshots,
each with a metric file), including 2,000 and 10,000. Eighteen pairs are replayed
from common states. Missing files, wrong iterations, malformed rows, changed
game/action metadata, nonfinite values, and a changed OpenSpiel version or
`cfr.py` hash fail the verifier. Ten mutation tests check these rejection paths,
including altered regrets, averaging, each metric, and missing snapshots.

Astra's Linux diagnostic run observed maximum one-step differences of `1.22e-15`
in current probabilities, `1.82e-12` in large signed regret accumulators and
`2.91e-10` in large CFR+ averaging accumulators. Independent final policy metrics
agreed with Rust to about `1e-16`. These measurements support the scaled entry
tolerances; they do not claim identical rounding for arbitrary future games.

After installing the pinned reference requirements, the full gate is:

```powershell
$env:ASTRA_CFR_TRACE_DIR = Join-Path (Get-Location) "target/cfr-traces"
cargo test -p toygames --locked -- --nocapture
.venv/Scripts/python -m unittest discover -s tests/reference/openspiel -p "test_*.py"
.venv/Scripts/python tests/reference/openspiel/verify_snapshots.py target/cfr-traces --output target/reference-verification.json
```

CI sets an absolute trace directory because Rust integration tests run with the
test crate as their working directory. Rust-only local checks still run the
known solutions, independent history comparisons, strict early curves and
absolute accuracy gates; complete acceptance also requires the Python verifier.

## Reproduce the references

From the workspace root on Windows:

```text
python -m venv .venv
.venv/Scripts/python -m pip install --only-binary=:all: -r tests/reference/openspiel/requirements.txt
.venv/Scripts/python tests/reference/openspiel/capture.py --game kuhn --variant cfr
.venv/Scripts/python tests/reference/openspiel/capture.py --game kuhn --variant cfr_plus --max-iterations 200000
.venv/Scripts/python tests/reference/openspiel/capture.py --game kuhn --variant dcfr --max-iterations 200000
.venv/Scripts/python tests/reference/openspiel/capture.py --game leduc --variant cfr
.venv/Scripts/python tests/reference/openspiel/capture.py --game leduc --variant cfr_plus
.venv/Scripts/python tests/reference/openspiel/capture.py --game leduc --variant dcfr
.venv/Scripts/python tests/reference/openspiel/export_fixtures.py
```

Leduc's original Python captures took about 16 minutes for CFR and 18 minutes for
CFR+ on this PC. Separate games/variants may run in separate processes; do not run
two writers for the same JSON file. Each checkpoint is written immediately and
the exporter rejects incomplete captures or a budget whose residual misses its
gate. Timing and timestamps change on regeneration; the numerical fields are
the regression data. CI uses the committed TOML, then installs the pinned
OpenSpiel reference for the common-state and independent metric checks. It does
not regenerate the long-running reference curves on every build.

OpenSpiel is [Apache-2.0 licensed](https://github.com/google-deepmind/open_spiel/blob/v2.0.2/LICENSE).
It is a development reference dependency installed from
[its official PyPI release](https://pypi.org/project/open-spiel/2.0.2/).
The Rust game and oracle code were written for this repository; no external solver
implementation is copied into them. Reference capture scripts were checked with
Black 25.1.0 and Ruff 0.11.13.
