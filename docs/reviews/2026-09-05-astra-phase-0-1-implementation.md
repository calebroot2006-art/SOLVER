---
project: gto-solver-app
type: review
status: needs-changes
date: 2026-09-05
---

# Astra review: bootstrap and toy-game CFR implementation

**Verdict: needs changes.** The implementation passes the fixed accuracy budgets,
but the Leduc reference-curve assertions and Windows runtime probe still fail.
This review is in progress; it does not approve the complete poker product.

## Version and scope

Astra reviewed the takeover changes after Fable's bootstrap `d256637`.
Hosted run [34013229097](https://github.com/calebroot2006-art/SOLVER/actions/runs/34013229097)
tested `14ceddc67e248f9725ebf1d43948f8cf2a9c6c24` on Windows and Ubuntu.
The next diagnostic revision, `b7d92f26feaa4100b321622413a742c8e27a9985`,
adds opt-in solver snapshots and corrects a test-helper Clippy finding.
Those snapshots are test data; they do not add a production state-import API.

Scope includes the payoff and postflop crates, independently implemented Kuhn and
Leduc histories, reference capture provenance, numerical tests, configuration,
CI, and the empty Tauri scaffold. Other workspace crates remain placeholders.
No hold'em, multiway, tournament, or coach implementation is certified here.

## N01: Leduc trajectory comparison needs a justified replacement

**Severity: high, verification blocker.** Location: `tests/tests/common/mod.rs`.
The original assertion compares every reference checkpoint's NashConv and EV with
`1e-9 + 1e-6 * abs(reference)`. All Kuhn checkpoints pass, including both
200,000-iteration extensions. Leduc first fails at iteration 100 for DCFR,
200 for CFR+, and 1,000 for vanilla CFR. The fixed accuracy gates still pass.

At iteration 200, Rust CFR+ reports NashConv `0.01007223119046846`, while the
captured OpenSpiel run reports `0.009926259100986573`. An independent experiment
using only OpenSpiel changes root counterfactual reach by a factor of 30 and
reports `0.01009236568360325`. Positive scaling cancels in regret matching and
commutes with CFR+ flooring. The exact-arithmetic algorithm is unchanged; this
control demonstrates sensitivity to floating-point normalization in the reference
itself. It does not, by itself, prove the Rust update correct.

**Correction under investigation:** compare one update from identical saved
states and independently evaluate Rust policies in OpenSpiel. Retain fixed
accuracy budgets, all diagnostic checkpoints, and a common early comparison
prefix. Any replacement of late curve identity must have its rationale,
measurement, and continuing regression checks recorded here before acceptance.
Vanilla Leduc needs an explicit final accuracy gate if its late curve assertion
is removed; the existing reference is `0.004084728965653733` at 10,000 iterations.

**Closure:** shared-state updates and external policy evaluation pass on the
integrated revision; revised assertions remain capable of rejecting incorrect
updates, incorrect metrics, and missed accuracy targets. No tolerance has been
relaxed at this review revision.

## B01: Windows native runtime is not yet verified

**Severity: high, verification blocker.** Location: `app/scripts/native-smoke.mjs`.
The Windows release executable builds and passes native formatting and Clippy.
The signed Microsoft driver and installed WebView2 both report `151.0.4129.101`.
Session creation fails with `DevToolsActivePort file doesn't exist`, before page,
CSP, denied-command, or screenshot checks execute.

Artifact `9983191202` from run `34013229097` has SHA-256
`11b9f69fdd63d6b81ee1f07bb5e8e67eb8f2b8a96d7dd8b76478af48d50b9160`.
It preserves the direct driver's native log and process diagnostics. This is
evidence of a failed automation session, not a passed runtime security test.

Microsoft documents that elevated WebView2 hosts ignore `WEBVIEW2_*` environment
overrides. Driver startup depends on those overrides. Runner integrity must be
measured before attributing this failure to elevation. A standard-user test
process is the intended correction if confirmed.
[Microsoft security guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags).

**Closure:** exercise the actual release page, inspect its screenshot, and pass
the CSP and denied-command probes. Keep the production permissions and CSP
unchanged. Caleb's local Smart App Control remains enabled.

## Verified evidence and implementation limits

- Run `34013229097` passes the 15 postflop unit tests, independent value and
  information-set best-response tests, sparse weighted-range CFR comparisons,
  and every Kuhn curve assertion. Its numerical failure is confined to the
  Leduc trajectory assertions above; a collapsed-if test-helper lint is corrected
  in `b7d92f2` and awaits that run's result.
- CFR+ Leduc at its 1,000-iteration budget measures NashConv
  `0.0005045426327558999`, below `0.001`. DCFR at 2,000 measures
  `0.00008158229155016961`, below `0.0001`.
- Both frontend jobs pass formatting, lint, type checking, tests, and production
  asset build. Native startup remains separately open under B01.
- The solver keeps signed regrets for vanilla/DCFR, floors only CFR+ regrets,
  alternates players, and discounts the whole DCFR strategy accumulator.
  Expected values use surviving private-deal mass. Best responses maximize after
  aggregating hidden opponent states. NashConv is the sum of the two best-response
  values; average exploitability divides by two, then percentage divides by the
  fixed starting pot and multiplies by 100.
- The immutable game binding snapshots topology, weights, masks, and terminal
  payoff kernels. Failed numerical iterations poison subsequent policy reads.
  Depth is bounded at 256. The trait still requires deterministic, linear
  terminal evaluation; basis-vector checks cannot prove arbitrary callback code
  satisfies that contract. There is no untrusted game-file import in this phase.
- Phase 1 is serial and checks progress between iterations. It does not claim
  parallel solving, asynchronous cancellation, or production-scale memory bounds.
  Terminal kernel snapshots cost `16 * H0 * H1` bytes per terminal and require
  profiling before hold'em-scale adoption.
- Dependency findings and platform scope are recorded in
  [the dependency review](2026-09-05-astra-dependency-review.md). An advisory scan
  is not a license audit or public-distribution security clearance.

## Review responsibility

Astra owns the final verdict, inspected integrated diffs, read hosted failures,
ran frontend checks and reference-export verification, and reviewed numerical
normalization and metric definitions. Helpers implemented bounded work in three
isolated worktrees and supplied independent numerical and native-runtime evidence.
Their reports do not replace the open closure criteria above.
