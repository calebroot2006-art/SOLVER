# Step 6 memory corrections

Executor checkout: `astra-step6-memory`, branch `solver/astra-step6-memory`,
based on `4dac7af` (the reviewed implementation plus red reproduction tests).
This correction changes R1, R2, R4 and R7. It changes no solver module or
production traversal arithmetic. Astra must review and integrate it before merge.

R1 now charges the complete `NodeBuild` array and its child, probability and
mask-index buffers while the flat layout holds its copies. Recursive child
buffers are included before expansion installs them in their parent. The bound
also prices copied decks, growing deal-table headers and interning maps, and
bounds payoff storage by the compact node count. Validation charges tuple
padding, scoped-index capacity, masks, columns and its terminal workspace.

R2 reserves every consumed row buffer's capacity before flattening, together
with the new flat snapshot. The input lease drops after the copy; only the
snapshot lease survives. Flat imports reserve the retained vector capacity
before validation. Valid imports with spare capacity and failed imports have
charge/release tests. The river excess-capacity test keeps valid row lengths
and requires `MemoryLimit`, restoring the check weakened in step 6.

R4 charges both boxed 1326-entry payloads, `Mutex<TerminalWorkspace>`, the
worker vector header and the lease. On this Windows build scratch is 106,960
bytes per worker/query; inline workspace plus payload is 106,912 bytes. The
verification alias row no longer includes a snapshot: measurement reads sums.
R7 checks the sums length alongside regrets before either split. Its malformed
private-state test remains a defensive check, not a supported-input panic case.

## Allocation evidence

`astra_memory_probe` now checks construction against shared plus construction
bytes, excluding solver/snapshot slack. It also supports `dense-turn` and
`dense-flop`; dense ranges assign positive weight to every unblocked combo.

| Case | Complete bound | Shared | Construction | Observed incremental peak |
|---|---:|---:|---:|---:|
| Sparse flop | 379,826,606 | 178,385,232 | 160,737,150 | 263,339,447 |
| Dense turn | 246,570,595 | 5,008,856 | 5,925,835 | 2,630,636 |
| Dense flop | 47,657,427,406 | 179,362,832 | 160,737,150 | 67,480 before refusal |

Sparse flop retains 110,810,362 additional bytes after construction. Dense turn
retains 1,876,808. Dense flop receives the named refusal at the 12 GiB default.
The counter measures requested allocation sizes, excludes allocations preceding
construction (including the compact tree), and does not measure RSS. These cases
test the conservative formula; they are not a proof for every accepted tree.

The one-worker, full-width gate bounds are 288,017,539 bytes on the turn and
53,692,865,806 on the flop. Over the widest Decision 9 ranges, f64 flop is
13,685,414,694 without a snapshot or 20,357,152,670 with one. Projected f32 is
7,013,677,014 or 10,349,546,150 respectively. Step 7 remains required; its host
measurement and numerical gate remain outstanding. README tables are updated.

## Validation

- The six original probes: R2/R4/R7 pass; R3/R5/R6 remain deliberately red in
  this isolated checkout. Their implementations belong to Astra's other work.
- `cargo test -p postflop --lib --offline -- --skip astra_review_current_rows
  --skip astra_review_cancel_during_iteration --skip astra_review_successive_attempts`:
  58 passed. The four memory unit tests passed again after final validation
  accounting changes.
- `cargo test -p postflop --test streets --offline`: all 15 passed, including
  row sums, reservation mapping, worker equivalence and independent all-in EVs.
- `cargo test -p postflop --test river --offline`: all seven passed, including
  the restored valid-capacity refusal.
- `cargo run -p postflop --example astra_memory_probe --profile test --offline`:
  passed; the same executable passed `dense-turn` and `dense-flop`.
- `cargo run -p postflop --example memory_table --profile test --offline -- 1`:
  passed. Fixture pins are turn 8,446,701 (construction 4,336,525), flop 85,924,582.
- `cargo fmt --all -- --check`, `cargo clippy -p postflop --all-targets --offline
  -- -D warnings`, and `git diff --check`: passed.

Logs remain under this checkout's ignored `target/astra-memory-*` paths. Exact
pins must be rechecked after lifecycle and diagnostic-row integration. No merge,
push, accepted-capture replacement or Fable-checkout edit was performed here.

The memory correction's standalone validated commit is `1e8e6f4`. A separate
compatibility commit changes the river cancellation predicate from poll three
to poll five, preserving its two-completed-iteration assertion with Astra's new
before/after driver polling. That one test needs rerunning after driver
integration; the standalone old driver intentionally does not match that count.
The prose checker reported zero banned terms. Five README sentence-length
review flags are existing policy/menu descriptions, retained for their context.

## Follow-up: reservation lifetime during failed import cleanup

Astra's independent review found that the input lease was a local while `rows`
remained a function parameter. When the destination reservation failed, Rust
dropped that local lease before freeing the parameter's buffers. The budget
therefore advertised available bytes while the consumed input still existed.

Both street and river imports now bind `(rows, input_lease)` in one tuple.
Tuple field order frees rows before releasing their charge if the destination
reservation fails. Passing the first field to the checked strategy constructor
keeps the second field alive through copying and validation.

The allocator example's new `import-failure` mode deterministically fills the
game budget so input fits and the destination snapshot does not. It watches one
large input row's pointer and samples `reserved_bytes` at that deallocation.
The hook reads an already initialized `OnceLock<PostflopGame>` and atomics; it
allocates nothing and takes no lock. No timing or second thread is involved.

The limit was 10,657,939 bytes, baseline reservation 10,061,664, input charge
546,800 and destination charge 68,856. The old code reported 10,061,664 during
payload deallocation and failed the assertion (exit 101). The corrected code
reported exactly 10,608,464 and passed. After return the reservation was back
to baseline. The named refusal's required bytes also verify that the failure
occurred at destination reservation, after input reservation succeeded.

Commands: `cargo run -p postflop --example astra_memory_probe --profile test
--offline -- import-failure`; `cargo test -p postflop --lib
imports_charge_capacity --offline`; `cargo test -p postflop --test river
memory_reservations_bound --offline`. All passed after the fix. The allocator
hook covers streets; river's matching ownership change was inspected and its
existing capacity/release integration test passed. Logs are
`target/astra-memory-import-order-before.log`, `-after.log`, `-unit.log` and
`-river.log` (the latter two use the `astra-memory-import` prefix).
