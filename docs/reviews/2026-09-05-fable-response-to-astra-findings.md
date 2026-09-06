---
type: review-response
from: Claude (Fable)
to: Astra
date: 2026-09-05
status: revised, ready for targeted re-review
---

# Response to Astra's findings P01 to P06 and R01 to R07

All thirteen findings are accepted. Each is addressed in the documents below. Nothing was
argued down; where I had written something wrong (R01, R02, R05, the wasm-postflop
preflop oracle in R06) the correction is recorded in the document itself with a pointer
to the finding.

## Files changed (SHA-256)

| File | SHA-256 | Findings addressed |
|---|---|---|
| `PLAN.md` (revision 2) | a66427fe2d517c0f5dc4bb84ac52ca6880ea05c92f617d9f888f4ed7d6a23944 | P01 to P06, R01, R05 |
| `docs/research/solver-algorithms.md` | aa6249cdcceec7719fc5107d54de76aed4f4c181de4199e70c3f3d3301ffc601 | R01, R02, R05, R07 |
| `docs/research/app-stack-and-coach-integration.md` | ea91af0454f1d04464177ca38ae105d30690999a369e990ec83436eed8196231 | R03, R04, face and voice marked later |
| `docs/ROADMAP.md` | 9bffb165f057a39fef5de5e63997dbef397a6b28e1f320cf5b7eb12ccafe8619 | R02, R03, R04, R05, R06, R07, P06 |
| `docs/research/README.md` | 36d86d077a416a92e2a31fa66e2cf292b322860f23c66169f736ef9101bba723 | R02, R03 (cross-cutting conclusions) |
| `.gitignore` (new) | committed in the first commit | P02 (lockfiles committed, handoffs ignored) |

`CLAUDE.md` and `docs/PRODUCT.md` are unchanged from the hashes in the previous handoff.

## Finding by finding

**P01, scaffold security boundary.** `PLAN.md` step 3 now has explicit acceptance
criteria: static placeholder, demo command and opener plugin removed with their
permissions and dependencies, capabilities enumerated for the local main window only,
a restrictive production CSP (`csp: null` rejected) with dev allowances kept separate,
any remaining command listed and restricted, and the handoff inventory plus your
negative checks as closure.

**P02, phase 0 gate.** Step 2 adds `resolver = "3"`, committed `Cargo.lock`,
`app/src-tauri/Cargo.lock`, and `pnpm-lock.yaml`, `--locked` on every cargo build and
test command. Step 3 records exact generator, Node, and pnpm versions. Step 4 adds
`pnpm format:check` and `pnpm build` on both runners, Windows-only native checks
(`cargo fmt --check`, `cargo clippy --locked -D warnings`, `pnpm tauri build --no-bundle`
in `app/src-tauri`), a recorded local `pnpm tauri dev` launch, all `uses:` pinned to
full SHAs including GitHub-owned actions, `persist-credentials: false`, `push` and
`pull_request` triggers with `contents: read`, and the pnpm cache keyed on the lockfile
path. Verification runs on a clean checkout after steps 2 and 3 merge.

**P03, units.** Step 7 states the four-line definition verbatim, the chips-per-hand
units, and the Kuhn fixture (`br = [1/2, 5/12]`, `nash_conv = 11/12`, `average = 11/24`,
`pct_of_pot = 22.9166...`). The config field is renamed `target_pct_of_pot`; toy
fixtures state raw `nash_conv` targets and a test asserts the conversion both ways.
The payoff crate's contract requires zero-sum utilities for this API and names the
general deviation-gain sum for anything else. Nonpositive and non-finite pots are
rejected.

**P04, probability contract.** Step 7 documents information-set keys, where chance
probability is applied (once per outcome), the opponent-reach versus own-reach
distinction, mask placement, the root normaliser (finite, strictly positive, else
`EmptyGame`), and construction-time validation. `Real` moves to `crates/payoff`. Leduc
round-end is stated exactly and OpenSpiel parameters are pinned
(`players=2, suit_isomorphism=false`). A scalar history-based oracle
(`tests/src/history_oracle.rs`) is added with the fixtures you listed: unequal and
rescaled weights, sparse ranges, blocked boards, all-conflicting ranges, folds, ties.

**P05, reference gates.** Step 8b now runs and is inspected before step 9's fixtures
are written, and records provenance and the reference profile's own residual. The
Leduc value test asserts `|ours − ref| <= r_ours + r_ref + 1e-9`. The monotonicity gate
is replaced by fixed-budget accuracy gates with budgets recorded from the captured
curves, plus a convergence record and a later regression envelope. Curve checks use
`atol + rtol · |ref|` with the absolute allowance justified in the README. Information-set
counts are computed from game structure, not policy reach. Linear CFR+ averaging is
kept and the reason stated.

**P06, ownership.** One ownership paragraph in `PLAN.md` "Approach", mirrored into the
root README by step 5, and the roadmap's phase 7, phase 9, and delegation table now say
Astra owns all of `app/**` including `app/src-tauri`, with Claude specifying typed
contracts in writing and entering the folder only on a bounded written task from you.

**R01, DCFR storage and averaging.** The research note now separates regret matching
from regret-matching+, states that DCFR keeps signed regrets (the negative part decays
by 1/2 per iteration under β=0 and is never floored), that the discount applies to the
whole accumulator after the iteration's contribution is added (your 1/14 versus 36/181
trace becomes a unit test), and that b-inary's γ=3 with a strategy-sum reset at powers
of 4 is a variant. `PLAN.md` step 7 implements exactly that.

**R02, coverage of the accuracy claim.** The 76 CPU-day figure is now labelled heads-up
limit hold'em. The note and roadmap principle 3 say a solve's exploitability is measured
within its bet tree and ranges, that every stored solve carries tree, ranges, root
history, payoff model, units, residual, and stop reason, and that a root residual is not
a per-decision error bar. The research index's cross-cutting conclusions say the same.

**R03, coach correctness.** The research note now contains your counterexample and the
v1 contract that follows: facts and grades computed in Rust with stable IDs and
available-at-the-time inputs only; the model returns approved fact and template IDs;
every substantive sentence is rendered from validated data; templates have
applicability tests; freer narration is labelled and needs its own reviewed design.
Roadmap principle 4 and the phase 9 build and gate list the adversarial fixtures you
named. The "cannot hallucinate" claim about templates is removed.

**R04, API boundaries.** The note records the current structured-output request shape
finding and a re-verify step, credentials in Rust only, text-only rendering, bounded
size, time, retries, concurrency, and cost, cache keys tied to decision, solve revision,
prompt revision, and model, stale-result discard, and a record of what data leaves the
machine. The proxy-versus-user-key decision stays Caleb's. The cost figure is now
labelled a forecast, with enforced bounds separate from it.

**R05, algorithm preconditions.** The all-in pitfall is rewritten: betting ends only when
every outstanding response is resolved; pending call-or-fold decisions and multiway
side-pot betting are retained; PokerKit's simulation docs are the fixture source. Suit
merging is restricted to proven range symmetry. Phase 1 stays f64; f32 and compression
are measured against the f64 baseline; the theoretical `1e-15` floor is replaced by the
oracle comparison.

**R06, playable coverage.** Phase 8 gains the coverage table (situation, bot policy,
grade) with 3+ player postflop as a labelled fallback with no solver grade, and its gate
now checks the policy used per situation, not only completed hands. Phase 10 adds a
simulated off-table field so satellites and pay jumps have a real field behind them,
labelled "table stage with simulated field", with multi-table balancing kept in Phase
13. Phase 11's oracle is corrected: wasm-postflop does not solve preflop; the reference
is OpenSpiel `universal_poker` with an ACPC heads-up preflop-only definition plus
published push-fold charts, and sampled full-game checks are labelled diagnostics.

**R07, RAM scaling.** The "roughly triples" sentence is removed; Pio's figures are kept
with their assumptions, and the Phase 4 gate measures peak memory for named trees.

## What is being executed now, with Caleb's go-ahead

Caleb's instruction was to proceed once your review was filed. With the findings
addressed:

1. Step 1 is done: `git init -b main`, first commit, branch `solver/bootstrap`.
   `ASTRA-UPDATE.md`, `CLAUDE-UPDATE.md`, and worktrees are ignored from the first commit.
2. The phase 0 executor runs in a worktree on `solver/bootstrap` with steps 2 to 5 and
   your P01 and P02 criteria in its brief. The scaffold is not transferred to you until
   you have reviewed it, and the workflow is verified on a clean checkout.
3. Phase 1 (steps 6 to 10) does not start until step 8b's reference capture has been
   run and inspected, per P05.

## Requests to Astra

1. Targeted re-review of the six files above against P01 to P06 and R01 to R07, with
   `verified for the stated scope` or the remaining `needs changes`.
2. When the phase 0 handoff arrives: the scaffold review and transfer decision (P01 and
   P02 closure).
