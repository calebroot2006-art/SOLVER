# Review handoff: phase 4 step 5b merged (turn gate capture, rule-based review, `node_values`)

Written by Claude on 2026-09-10 for Astra's independent review. Merge commit `114b815` on
`solver/phase-4`; the executor branch `worktree-agent-a9ca9d74a78331558` (head `d0830ca`)
is deleted after the merge, so review the merged tree.

## The problem and the expected behaviour

Step 5b makes the turn gate a measured, comparable thing. Before it, the turn solver had
a reference tooling job (5a) but no capture of our own solve and no comparison. The step
adds:

* `crates/postflop/examples/turn_capture.rs`: solves the three gate cases in f64 on CI,
  reads the memory limit, worker count and precision from `config/solver.toml`, stops on
  each case's target or cap (never on a clock, so the stopping iteration is reproducible),
  and writes one TOML capture with policies, action EVs, per-hand values at chance nodes
  and turn showdowns, timings, host facts and cancellation probes.
* `PostflopStrategy::node_values(node)` in `crates/postflop/src/streets/strategy.rs`:
  per-hand net-chip values for both players at any node, sharing the reach walk with
  `decision_values` through a new `path_reaches` helper. `None` means no range weight, a
  blocked hand or no compatible opponent, never zero for missing. It reserves its own
  named memory row (`MemoryReservation::NodeReport`, 85,376 bytes), which the working-set
  bound now counts beside the decision report.
* `tests/reference/turn/compare.py` joint mode, gating on: both sides converged, same
  inputs, every history matched, root EV agreement, and a rule-based per-combo review.
* The review rule (`review_rule.py`, thresholds in `review_rules.json`): each differing
  row is A (indifferent: adopting the other side's mix costs at most 1% of pot on both
  sides' own action EVs), B (unreached: an EV absent or reach under 1e-6, with the bound
  reach × max gap recorded), or C (a real gap, listed with its reach-weighted loss). The
  gate passes when the C rows' loss sums to under 0.5% of pot per case. Every C row also
  carries `oracle.py`'s independent recomputation of the project's action values, and
  `compare.py` fails a row whose recomputation disagrees with the capture beyond
  `oracle_agreement_chips` (1e-9). The committed `per-combo-review.json` holds the rule,
  its file hash, the counts, the C rows only, and two threshold-free totals.
* `tests/reference/turn/measured/643d803/`: the small measured record and how to
  regenerate it.

The three thresholds are the Decision 14 candidate. Caleb has not confirmed them.

## What was verified, by whom

| Check | Where | Result |
|---|---|---|
| CI at `d0830ca` | run 34510476253 | all 13 jobs green, including `turn-solve` on both OSes and `turn-compare` |
| Joint comparison rerun with the committed review | main session, on that run's captures, both OSes | accepted; A/B/C 10,998/32,193/351, 17,029/35,561/189, 9,913/32,227/277; C loss 4.10e-05, 1.35e-05, 1.32e-05 of pot; threshold-free all-row loss 1.08e-03, 4.96e-04, 8.36e-04; root EVs within 0.00337, 0.00091, 0.00067 chips of the reference on an 11-chip pot |
| Oracle agreement on the C rows | `review_combos.py`, recorded per row | worst 6.86e-13 chips over 817 rows; 700 walked end to end, 117 through chance-node values |
| River regression against `tests/reference/river/measured/2930550/` | main session, artifacts of run 34510476253, both OSes, raw and refined captures | identical except `elapsed_seconds`, `project_revision`, and the +24-byte bound known since step 3 |
| Turn capture stability across the last two runs | main session, 2,501,439 fields per OS | only timings, revision and the +85,376-byte bound differ |
| Convergence | both captures | project 0.208 / 0.166 / 0.208% of pot at 150 / 200 / 150 iterations; reference 0.159 / 0.178 / 0.185% |
| `/code-review high` on the branch | main session | eight findings, all closed in round three and re-verified (memory row, staleness evidence, zero action gap, oracle recomputation, aggregate totals, strict rules loader, output refusal, README provenance) |
| Local unit tests | executor | `cargo test -p postflop --lib` 47 passed; Python suite 134 passed (`unittest`) |

## How to run it

```
python tests/reference/turn/compare.py --reference <turn-wasm-reference>/cases.json \
  --project <turn-project-ubuntu-latest>/cases.toml \
  --review tests/reference/turn/per-combo-review.json \
  --expected-revision <40-char sha> --output <new file>
python -m unittest discover -s tests/reference/turn
```

Artifacts come from run 34510476253 through `docs/astra/development-takeover/ci_status.py`
(`artifacts <run>` then `download <id> <zip>`); seven-day retention.

## Known limitations and open decisions

* Decision 14 thresholds unconfirmed. The review noted that 1% of pot per row is four
  times the 0.25% convergence target, and that nothing caps the A rows in aggregate; the
  record now reports the A and all-row totals so Caleb can decide with the number.
* The committed review is generated from the Linux capture and checked against both OSes
  at 1e-12; a Windows-only stale failure is a determinism failure, not a review to
  regenerate. The README says so.
* The capture is 65,101,178 bytes against `compare.py`'s 64 MiB ceiling (3% headroom), and
  the size guard runs before the file is written. A fourth case or runout waits for step 6
  or 7 to shrink the capture.
* After a called all-in on the turn our tree deals 48 river runouts where the reference
  ends at a turn showdown; `compare.py` excludes those nodes by shape without a numeric
  check. Logged for step 6.
* `drive` in `crates/postflop/src/solver.rs` still uses one measurement for both the
  progress log and the stop test; the capture works around it with a very long progress
  interval. Step 6 owns that file.

## Files still being edited

None on this step. Step 6 starts next on a new branch and touches
`crates/postflop/src/{game,strategy,cfr,solver}.rs` and `src/streets/*`.
