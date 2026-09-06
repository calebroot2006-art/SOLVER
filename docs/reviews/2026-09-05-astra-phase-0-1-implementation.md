---
project: gto-solver-app
type: review
status: verified
date: 2026-09-05
---

# Astra review: bootstrap and toy-game CFR implementation

**Verdict: verified for the stated scope.** Phase 0's scaffold and Windows runtime
checks pass. Phase 1's numerical gates pass on Windows and Linux. N01 and B01 are
closed below. This review does not approve the complete poker product or distribution.

## Version and scope

Astra reviewed the takeover changes after Fable's bootstrap `d256637`.
Final hosted run [34015308353](https://github.com/calebroot2006-art/SOLVER/actions/runs/34015308353)
tests `0d4f338d5e1b62bd8af25ce3580a6f7c3c252a26` on Windows and Ubuntu;
all five jobs pass. The working tree was clean at that commit. The final save
adds this closure documentation and preserved CI evidence without code changes.
Opt-in solver snapshots are test data, with no production state-import API.

Scope includes the payoff and postflop crates, independently implemented Kuhn and
Leduc histories, reference capture provenance, numerical tests, configuration,
CI, and the empty Tauri scaffold. Other workspace crates remain placeholders.
No hold'em, multiway, tournament, or coach implementation is certified here.

## N01: Leduc trajectory comparison, resolved with direct checks

**Original severity: high; resolved.** Location: `tests/tests/common/mod.rs`.
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

**Diagnosis reproduced by Astra:** snapshots from run `34013757158` at `8d24ed6`
agree with OpenSpiel for all 18 shared-state update comparisons on each operating
system. Each comparison covers 936 information sets. Maximum current-policy
difference is `1.23e-15`; maximum signed-regret difference is `1.82e-12` and
maximum cumulative-strategy difference is `2.92e-10`, at the larger vanilla-regret
and CFR+ averaging scales. Independently evaluating the actual Rust averages
agrees with all 39 Leduc checkpoint EVs and NashConv values on each platform;
the maximum metric difference is `1.34e-15`. Details and per-snapshot hashes are
in `docs/astra/development-takeover/shared-state-evidence.json`.

These results distinguish a rounding-sensitive learning trajectory from an update
or evaluator defect at the tested states. Astra also independently reproduced all
eight checkpoints of the OpenSpiel-only factor-30 experiment, with exact equality
to its saved control. The normalization is explicit: uniform private chance is
`1/30`; OpenSpiel regret units are Rust regret units divided by 30. Its averaging
accumulator visits five hidden histories before the board and four after it.
Those constant information-set factors cancel in average-policy normalization.

**Reviewed replacement:** retain the original tight Kuhn
checks and a common Leduc prefix through iteration 50. Keep every later checkpoint
as diagnostic data, and keep existing absolute budgets unchanged. Add the explicit
vanilla Leduc gate `<0.005` raw NashConv at 10,000 iterations; the pre-existing
external reference measures `0.004084728965653733` there. Require independent
OpenSpiel checks of the actual generated policies and shared-state updates in CI.
Replay uses `1e-12 + 1e-12 * abs(reference)` for regret/averaging entries and
absolute `1e-12` for current policies and independently evaluated metrics.
These tolerances accommodate measured accumulation scale; they do not assert
that different long-running learning trajectories must remain identical.

**Closure evidence:** run `34014953971` at
`c11d0aac886ca845b5a3a6af532137c62c3d28bc` passes both solver jobs. Each job runs
all Rust tests, 11 verifier mutation tests, 18 shared-state replay pairs, and
independent evaluation of current and average policies at all 54 required
snapshots. Missing snapshots, altered accumulators/metrics, and changed reference
source are rejected. Targets remain enforced at and after their fixed budgets.

An initial Linux verifier failure came from line endings in the official wheels:
Windows `cfr.py` has 528 CRLF endings; Linux uses LF. Astra verified the Linux
wheel digest and exact byte equality after CRLF-to-LF conversion. The guard pins
that canonical content and reports both raw and canonical hashes. Original capture
hashes are preserved. The source comparison is recorded in
`tests/reference/openspiel/diagnostics/source-line-endings.json`.

## B01: Windows native runtime, resolved with hosted verification

**Original severity: high, verification blocker; resolved.**
Location: `app/scripts/native-smoke.mjs` and `run-native-smoke.ps1`.
The original Windows release executable built and passed formatting and Clippy.
The signed Microsoft driver and installed WebView2 both report `151.0.4129.101`.
Session creation failed with `DevToolsActivePort file doesn't exist`, before page,
CSP, denied-command, or screenshot checks execute.

Artifact `9983191202` from run `34013229097` has SHA-256
`11b9f69fdd63d6b81ee1f07bb5e8e67eb8f2b8a96d7dd8b76478af48d50b9160`.
It preserves the direct driver's native log and process diagnostics. This is
evidence of a failed automation session, not a passed runtime security test.

Microsoft documents that elevated WebView2 hosts ignore `WEBVIEW2_*` environment
overrides. Driver startup depends on those overrides. Run `34013757158` at
`8d24ed6` confirms the runner's administrator role and High Integrity SID
`S-1-16-12288`. Artifact `9983339224` has SHA-256
`adb53e49b962f77eb72cdc2bbab0cb2a06af54800eb3c1bd3b45b3f3c1ae6235`.
Astra inspected the saved token evidence and reviewed the standard-user launcher
integrated as `701fc9d`. It stages byte-identical app/driver binaries, grants the
temporary account access only to its staging directory, and requires Medium IL
in the child. Independent cleanup attempts remove its processes, profile,
account, and verified staging path; a cleanup failure rejects the run.
[Microsoft security guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags).

Two subsequent test-driver defects were corrected: alternate credentials discarded
environment configuration, and the WebDriver client misclassified a successful
script result containing an application `error` field. Non-secret configuration
now passes through a staged JSON file. HTTP status determines protocol failure;
a regression test preserves the actual ACL-denial object returned with HTTP 200.
No production permissions, CSP allowances, or local security settings changed.

**Closure evidence:** final run `34015308353`, Windows job `101437927726`,
passes the release build, native probe, evidence upload, and unchanged-lockfile check.
The page renders at 1028 by 749 with applied CSS and no unexpected errors or
load-time CSP violations. The actual `plugin:app|version` invocation returns
`Command plugin:app|version not allowed by ACL`. The external request produces
an enforced `connect-src` violation naming
`https://astra-csp-probe.invalid/scaffold-runtime-check`; a fetch failure alone
cannot satisfy this probe. The child proves Medium Integrity Level, and cleanup
reports the disposable account removed with no errors.

Astra independently downloaded artifact `9983770900` and matched its SHA-256
`8490f827552a20aae94dc0e7eff77be95aa11f3a4ba58bab84c11a8bacbc98f3`
to GitHub's digest. Astra inspected the final screenshot and JSON results.
The [evidence manifest](../astra/development-takeover/final-ci-evidence.json),
[native result](../astra/development-takeover/native-smoke.json),
[cleanup result](../astra/development-takeover/standard-user-launch.json), and
[screenshot](../astra/development-takeover/release-placeholder.png) are preserved.
The executable SHA-256 is
`72a6268cfffbb7581b772a67ef59a25fed3723e8a98712482036865d522f8378`.
This verifies the tested frontend boundary, not a machine-wide network sandbox.

## Verified evidence and implementation limits

- Final run `34015308353` passes the 15 postflop unit tests, independent value and
  information-set best-response tests, sparse weighted-range CFR comparisons,
  and every Kuhn curve assertion. Rust formatting and Clippy pass. Both solver
  jobs also pass the revised OpenSpiel gates described under N01.
- CFR+ Leduc at its 1,000-iteration budget measures NashConv
  `0.0005045426327558999`, below `0.001`. DCFR at 2,000 measures
  `0.00008158229155016961`, below `0.0001`.
- Both frontend jobs pass formatting, lint, type checking, eight Vitest tests,
  five runtime-evidence tests, and the production asset build. The Windows native
  release and security probes pass as recorded under B01.
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
Astra personally inspected the final CI results and native artifact, verified its
digest, and inspected the rendered screenshot before closing the findings.
