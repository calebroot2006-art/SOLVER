---
type: review-findings
from: Astra scaffold agent
to: Astra
date: 2026-09-05
verdict: verified for the stated scope
---

# Scaffold permission and CI corrections

**Verdict: verified for the stated scope.** Static configuration and frontend checks
pass after the corrections below. Native runtime behavior is **not yet verified**.
This report supplies evidence for Astra's independent review; it does not close P01's
runtime gate or establish completed application security.

## Snapshot

Baseline: `d256637dcba80f93046977d2673d207cdd159750`, reviewed in the isolated
`app/astra-scaffold-hardening` branch at `.claude/worktrees/astra-scaffold`.
The change touches `app/`, `.github/workflows/ci.yml`, and this report. The original
[bootstrap handoff](2026-09-05-phase-0-bootstrap.md) remains unchanged.

## Findings and corrections

### S01. Medium: unused core grants contradict the command inventory

**Location:** baseline `app/src-tauri/capabilities/default.json:7`,
`app/README.md:44`, `app/src-tauri/src/lib.rs:3`.

**Evidence and impact:** Six `core:*:default` sets granted built-in APIs while the
inventory claimed no IPC command was reachable. The static React frontend calls none
of them. The claimed umbrella expansion also omitted `core:resources:default` from
Tauri 2.11.5's actual list. Being a subset did not establish necessity.
[Pinned permission source](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri/build.rs).

**Correction:** The selected capability now has `permissions: []`. The window label
is explicit, and `app.security.capabilities: ["default"]` prevents automatic
activation of additional capability files. Inventory wording distinguishes app
commands, granted APIs, and Tauri's internal IPC machinery.

**Verification:** The empty-grant and selection assertions pass. Temporarily adding
`core:image:default` or removing explicit capability selection makes the tests fail.
Rendering and rejection through a native WebView remain open.

### S02. Medium: future command instructions omitted the application manifest

**Location:** baseline `app/README.md:112`, `app/src-tauri/src/lib.rs:8`.

**Evidence and impact:** Registering a custom command makes it available to all app
windows by default. The previous three-step procedure omitted
`AppManifest::commands`, so following it could leave a command outside the intended
capability checks. No such command exists today.
[Tauri application command manifest](https://v2.tauri.app/security/capabilities/).

**Correction:** The README requires the complete manifest list, a named permission,
its selected-window grant, backend validation, and positive/negative runtime tests.
The Rust entry-point comment points to that procedure. No command was added.

**Verification:** Read against the official manifest instructions. This is a
documentation correction; future implemented commands require their own review.

### S03. Medium: CSP regression guards accepted extra sources

**Location:** baseline `app/src/smoke.test.ts:125`.

**Evidence and impact:** The hostname-prefix regex accepted `http://ipc.localhost.evil`
and did not inspect scheme-only sources such as `https:`. Five directives were
not asserted individually. The checked-in production CSP itself was restricted.

**Correction:** The tests compare the complete production directive map and the
separate development map. Capability selection and the single local window are
also asserted. The CSP allowances themselves remain unchanged.

**Verification:** Both deliberately widened `img-src` variants fail the actual test
suite. All eight tests pass after restoring the checked-in configuration. These
guards inspect files; they do not substitute for runtime CSP enforcement tests.

### S04. Low: the native release build did not request a locked resolution

**Location:** baseline `.github/workflows/ci.yml:152`.

**Evidence and impact:** Cargo clippy used `--locked`, but the Tauri release command
did not. The prior handoff's claim that every Cargo step was locked was broader than
the workflow. Tauri forwards arguments following `--` to the Cargo runner.
[Pinned CLI forwarding implementation](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/interface/rust/desktop.rs).

**Correction:** Windows now runs `pnpm tauri build --no-bundle -- --locked`.
A final `git diff --exit-code` checks all three committed lockfiles on each runner.

**Verification:** Source inspection confirms argument forwarding. The three lockfiles
are unchanged locally. The amended native build still requires a GitHub Actions run.

## Checks run

With `C:\Users\Caleb\AppData\Local\nodejs` on the process PATH:

- `pnpm.cmd install --frozen-lockfile --offline --store-dir
  C:/Users/Caleb/AppData/Local/pnpm/store --package-import-method=copy`: pass;
  182 packages reused, no downloads, no lockfile change.
- `pnpm.cmd format:check`, `lint`, `typecheck`: pass.
- `pnpm.cmd test`: eight tests pass.
- `pnpm.cmd build`: pass; 17 transformed modules and emitted HTML/CSS/JavaScript.
- Four temporary configuration mutations: hostname suffix, scheme source, omitted
  capability selection, and added image grant. Each returned test exit code 1;
  files were restored in `finally`, then all eight tests passed again.
- `git diff --check` and the lockfile diff check: pass.

The first package install hit the sandbox's unavailable network proxy. Offline copy
installation reused the existing store. Vite's Windows path helper initially failed
with sandbox `spawn EPERM`; the tests and build passed under tool escalation without
changing Windows security settings. No Rust compiler or desktop window ran locally.

## Remaining gate

Astra must review the integrated diff and new CI result. P01 still needs a release
WebView launch proving that the placeholder loads without unexpected CSP failures,
an ungranted core API fails, and an external request is blocked specifically by CSP.
The follow-on runtime checks will run in hosted Windows CI so Smart App Control on
Caleb's PC remains enabled.
