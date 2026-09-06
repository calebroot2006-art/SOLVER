---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-06
---

# Finish the shared DCFR reference

## Progress

Fable left three untracked draft files on `solver/phase-1-review`, based on
`713c43e`: a shared DCFR module, upstream comparison tests, and a reference README.
The scripts now share one discount. Astra strengthened the tests and ran all
18 Python tests, 18 shared-state replays, and 54 policy-snapshot checks successfully.
All six original capture entries and both TOML fixtures remain byte-identical.
The independent agent reviewed the integration and agreed with the scoped result.
All five Windows/Linux CI jobs pass at `fc8fd1a` in run `34019066360`.
The scoped review is verified; phase 2 has its own plan.

## Task

Complete and verify the shared reference without changing the Rust solver's
algorithm, captured numerical data, or accepted accuracy budgets. Then prepare
the next roadmap phase under Caleb's renewed development instruction.

## Steps

1. Review the draft against installed OpenSpiel 2.0.2 source and strengthen its
   tests to compare normalized average policies and accumulator scaling.
2. Import the shared constructor and discount in `capture.py`, `sensitivity.py`,
   and `verify_snapshots.py`. Record all executed local source files in future
   outputs so extraction does not make provenance incomplete.
3. Preserve the six original captures and their historical hashes. Exercise the
   refactored capture path in temporary output and compare numerical checkpoints.
4. Run Python formatting, lint, unit tests, and full independent verification
   against the already verified Rust snapshots. Push and inspect Windows/Linux CI.
5. Save the scoped review and update the handoff before starting the phase 2 build.

## Tests and risks

- Regret signs, averaging normalization, and update order must remain unchanged.
- Compare per-action averages, not only scalar exploitability. Upstream DCFR's
  weighted sums use a different scale from the project's discounted sums.
- Tests must fail on a changed discount or average policy. Keep the existing
  verifier mutation tests and all fixed numerical acceptance gates.
- Do not overwrite reference captures during a short smoke run. Preserve Fable's
  downloaded wheel outside commits; no dependency installation is needed locally.
- Smart App Control stays enabled. Rust checks run in GitHub Actions.

## Decisions

- 2026-09-06: Caleb authorized continued project work, superseding the earlier
  instruction to stop after the handoff. He requires 10% account usage remaining,
  allowing a 1–2 percentage-point margin. Astra can read the installed Codex
  account endpoint and will stop at 12% remaining, polling between work steps.
  If the counter becomes unavailable, pause instead of estimating from tokens.
- Astra owns integration and final judgment. The reference agent reviews read-only;
  the core agent independently plans phase 2. No concurrent writers are assigned.
