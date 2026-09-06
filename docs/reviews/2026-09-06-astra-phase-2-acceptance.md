# Phase 2 acceptance

**Verdict: verified for the stated scope.** Cards, combos, weighted ranges,
checked hold'em evaluation, and terminal sweeps pass the phase 2 gates at
`620ea855c60ccbf21df713c62f3930dcc7c248f2`. All five jobs passed in
[run 34041359029](https://github.com/calebroot2006-art/SOLVER/actions/runs/34041359029).
This supersedes the open findings in the earlier cutoff checkpoint.

## Corrections and review

P2-01 is closed: vertical tab is accepted both as empty whitespace and between
range tokens, with the regression passing on Windows and Linux. P2-02 is closed:
the reviewed hosted formatter patch was applied and the formatting job passes.
P2-03 is closed: both platforms executed the corrected parser and actual Rust
replay of all 1000 exact-arithmetic fixtures. CI also regenerates those fixtures
through the independent Python Fraction oracle in check mode.

Astra inspected card/combo bijections, strict bounded parsing, arbitrary-weight
roundtrips, duplicate-card rejection, vendor rank/suit conversion, independent
five-card classification, sample generation, and seven-card subset enumeration.
The terminal review covered blocker inclusion-exclusion, strict sweeps, equal-rank
groups, exact integer capacity and rounding, signed utility accumulation, atomic
outputs and failure recovery. A read-only helper independently reviewed terminals
and reproduced the Fraction fixtures; it found no demonstrated defect. Astra
triggered CI and inspected both numerical logs and the native runtime artifact.

## Measured gates

Both platforms passed every one of 2598960 five-card hands, all 7462 strength
classes, and ten million deterministic seven-card hands checked against the
maximum of their 21 independently classified five-card subsets. Both passed
full/sparse pairwise terminal comparisons, chops, blockers, extreme cancellation,
linearity, unequal contributions, zero-sum weighting, and invalid-input tests.
All existing Kuhn/Leduc and OpenSpiel gates remain unchanged and passed, including
the 18 Python verification tests and actual Rust policy/update comparisons.

| Measurement | Linux x86_64 | Windows x86_64 |
|---|---:|---:|
| Exhaustive five-card seconds | 0.162 | 0.218 |
| Ten-million seven-card oracle seconds | 16.936 | 17.017 |
| 100000 checked seven-card calls, median seconds | 0.005073 | 0.003623 |
| 108100 cached-board calls, seconds | 0.001322 | 0.000914 |
| 108100 rebuilt-board calls, seconds | 0.004174 | 0.004876 |
| Single sweep, observed microseconds | 568.971 to 750.574 | 657.2 to 971.2 |

These are hosted-runner observations with Rust 1.98.1 and the test profile at
optimization level 2, overflow checks and debug assertions enabled. Concurrent
tests affect timing; these are not measurements of Caleb's 16 GB machine.
The evaluator has 312320 static table bytes and a 24-byte board prefix. Tested
showdown tables occupy 6598 to 8582 bytes; reusable scratch occupies 85680 bytes.
No dense private-pair payoff matrix is allocated by these terminal operations.

## Security and application limits

Parser input is bounded before expansion; weights must be finite and in range.
Checked card types prevent duplicate or invalid cards from reaching the vendor
evaluator. Reach and utility checks reject invalid numbers and nonrepresentable
arithmetic without changing caller outputs. Dependency source, notices and the
resolved evaluator closure were inspected in the recorded phase 2 audit; its
seven new registry packages had no reported yanks or OSV entries at that read.
This is a scoped dependency/input review, not a whole-application security claim.

Frontend formatting, lint, types, tests and build pass on both platforms. The
Windows native release build and standard-user WebView2 smoke test pass: page
diagnostics are clean, the ungranted Tauri command is rejected, and CSP blocks
the external request. Smart App Control remained enabled; no Rust ran locally.

The app still displays its starter shell. River solving, playable poker and
coaching are not delivered by this phase. Phase 3 must replace the legacy dense
callback binding for real ranges while retaining its toy-game mutation audits.

Durable evidence: [job steps](../astra/phase-2/final-ci-jobs.json),
[numerical and native summary](../astra/phase-2/final-evidence.json), and
[dependency audit](../astra/phase-2/dependency-evidence.json).
