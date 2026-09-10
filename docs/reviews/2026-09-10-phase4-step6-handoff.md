# Review handoff: phase 4 step 6, built and green, NOT merged

Written by Claude on 2026-09-10 evening. This one needs Astra's review before it merges,
more than the earlier steps did, because my own `/code-review` ran only two of its eight
angles before the account hit its limit.

Branch `worktree-agent-aeabce3b33204de8a`, head `e338d7a`, worktree
`.claude/worktrees/agent-aeabce3b33204de8a`, base 14a02fd on `solver/phase-4`. The branch
is pushed. Nothing is merged.

## What the step does

Step 6 is the storage rewrite the flop gate needs. Four things at once:

1. **In-range compaction.** The walk carries one entry per live combo instead of all 1326.
   A combo with no range weight, or one the board prefix blocks, had a zero live mask at
   every terminal, so its regrets never left zero; dropping its row is a projection.
2. **Flat layout.** Topology is struct-of-arrays with one shared edge array and an interned
   payoff index. Regrets and strategy sums are one flat buffer each, sliced by a per-node
   `u64` offset.
3. **Two arrays, not three.** The current policy is derived from regrets at visit time. No
   average-strategy snapshot is retained during a solve; the best-response walk normalises
   the strategy sums as it reads them.
4. **The cancel path.** `drive` measures on `check_every` and at the cap only, so the
   stopping iteration no longer depends on host speed; `log_every_secs` drives the progress
   callback alone. A cancel takes no measurement. `SolveReport` gains
   `exploitability: Option<Exploitability>`, `measured_at` and `stale_measurement`.

## What I verified myself

| Check | Result |
|---|---|
| CI at `e338d7a`, run 34524553796 | all 13 jobs green, including both `Solver` jobs, `Turn solve` on both OSes and `Turn project versus reference` |
| Turn captures, step 6 run vs the step 5b run, 2,501,439 fields per OS | **no policy or value field differs.** Only timings, revision, the progress interval and the memory bounds moved |
| `compare.py --review` rerun on both step 6 captures | accepted, 0 rows missing review, 0 stale; A/B/C counts identical to step 5b |
| River captures vs `tests/reference/river/measured/2930550/`, raw and refined, both OSes | identical except elapsed, revision, and the bound fields |
| Executor's own baseline hashes (temporary test file, deleted before push) | turn policy `0xc6c5e36bcfd872ff`, `nash_conv` `0x3fa4027cc9e87662`, root `node_values` `0x4b497d1a39823ebf`, flop `0x8b9896cb3e0fa982`, river `0xaf50c5d13e16a867` from both games, unchanged at 1 and 2 workers |

The stale check in the turn comparison holds recorded strategies to 1e-12 against the
committed review, and it passed on both captures. Combined with the field comparison, the
numerical invariant looks sound to me. I have not independently re-derived the executor's
baseline hashes, because the test file that produced them was deleted before the push.

## The memory numbers

At one worker, f64, over the widest board of each gate set, mid-solve:

| Tree | Step 5c predicted | Step 6 measured | Under by |
|---|---|---|---|
| Turn gate | 80,344,515 | 75,768,147 | 4,576,368 |
| Flop gate | 14,095,816,234 | 13,533,132,510 | 562,683,724 |

With one browsing snapshot alive, which is what the budget enforces: 108,025,923 on the
turn and 20,204,870,486 on the flop. Step 5c's ordering conclusion stands: the flop gate is
618.20 MiB over the 12 GiB default while it solves, so step 7's `f32` is still required.

## What I want reviewed

The review I could not finish. Six of eight angles died on the account limit; the two that
returned (reuse, simplification) gave twelve findings, all quality rather than correctness,
recorded in the plan's mid-flight section. Nobody has yet read this diff for correctness
end to end.

Three places carry the risk:

1. **`crates/postflop/src/cfr.rs`, `chance_in_parallel`.** It cuts the flat regret and
   strategy-sum buffers into per-worker slices by node range. Two workers sharing one
   regret row would make the answer depend on which finished first. The skip and take
   arithmetic is checked for the regrets array; one finder reports the same guard is not
   applied to the sums array, which would panic inside `split_at_mut` rather than return
   the named error. Worth confirming, and worth confirming the ranges are disjoint.
2. **`crates/postflop/src/streets/terminal.rs`, the scatter and gather.** The compacted
   opponent reach is scattered over all 1326 IDs so `ShowdownTable` and `evaluate_fold` see
   what they always saw, then the live entries are gathered back. The claim is that the
   zeros left behind are the reach those combos already carried, so every sum is the same
   sum in the same order. That claim is the whole invariant.
3. **`crates/postflop/src/streets/memory.rs`, `estimate` and `rows_under`.** Every `Budget`
   site must still be a named row and the rows must still sum to the bound.

## Known open items

* The river record's `working_set_bound_bytes` and `reserved_bytes` are now **136 bytes**
  above the accepted record, up from 24. The executor attributes 112 of it to
  `size_of::<TraversalLayout>()` growing when the topology became struct-of-arrays. No
  solved field moves and nothing in CI compares those two fields. It wants recording at the
  next reconciliation of the record rather than treating as a failure.
* `PostflopStrategy::from_rows` now expects compacted-width rows.
* Peak resident memory is still unmeasured against the table on any host. That is step 5c's
  outstanding acceptance step and it needs the flop gate runner.
* Three changes against the drafted 5d contract, made because your review of it has not
  arrived: `SolveReport` carries the measurement's optionality, iteration and staleness
  instead of the draft's timings block; `MemoryReservation::Snapshot` is charged once
  rather than twice; `JobId` is one pair type and `GameId`/`SnapshotId` are unimplemented.

## How to run it

```
cd .claude/worktrees/agent-aeabce3b33204de8a
cargo test -p postflop
cargo clippy --workspace --all-targets -- -D warnings
python docs/astra/development-takeover/ci_status.py jobs 34524553796
```

Artifacts from run 34524553796 come through the same script (`artifacts <run>`, then
`download <id> <zip>`); seven-day retention. My comparison reports are in this session's
scratchpad and are reproducible with the command in the step 5b handoff.
