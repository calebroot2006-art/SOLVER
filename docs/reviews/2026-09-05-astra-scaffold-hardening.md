---
type: review-findings
from: Astra scaffold agent
to: Astra
date: 2026-09-05
verdict: verified for the stated scope
---

# Scaffold permission and CI corrections

**Verdict: verified for the stated scope.** Static configuration and frontend checks
pass after the corrections below. This report preserves the scaffold agent's original
findings and runtime investigation. The pending statuses in that history are superseded
by Astra's [final implementation review](2026-09-05-astra-phase-0-1-implementation.md).
Run `34015308353` at `0d4f338` passes the actual release render, ACL denial, enforced
external-request CSP violation, and hosted-account cleanup. Astra independently
verified the final artifact and closed P01's runtime gate. This does not establish
completed application security beyond the tested scaffold.

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

## Hosted runtime follow-on

After static commit `a4acb17701346e715f6d8cd3916e48303dbc3374`, the same isolated
branch adds `app/scripts/` for hosted Windows runtime verification. The new workflow
starts an external Microsoft EdgeDriver against the unchanged release binary. It does
not add an application command, test plugin, grant, or CSP exception.

The script observes errors and CSP violations from before a fresh navigation.
It checks the rendered heading and applied CSS, then requires a permission denial
from the real core app-version command. An external fetch must trigger an enforced
`connect-src` violation naming the probe's exact URL or origin. DNS failure,
a nonexistent command, and a report-only CSP event cannot pass those checks.
Evidence includes the screenshot, application hash, driver hashes/versions,
JSON results, and driver log in a seven-day Actions artifact.

Sources inspected: [Tauri's manual setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/),
the pinned [driver capability translation](https://github.com/tauri-apps/tauri/blob/tauri-driver-v2.0.6/crates/tauri-driver/src/server.rs),
[Microsoft EdgeDriver version matching](https://learn.microsoft.com/en-us/microsoft-edge/webdriver/),
and [Microsoft's WebView2 options](https://learn.microsoft.com/en-us/microsoft-edge/webdriver/capabilities-edge-options).
The upload action is pinned to v7.0.1 commit
`043fb46d1a93c77aae656e7c1c64a875d1fc6a0a`, independently resolved by root Astra.

Local checks cover JavaScript/PowerShell syntax, formatting, lint, typecheck, the
eight existing Vitest checks, and three new evidence-classification tests. A hosted
run at integrated commit `02d3da433c7c80e69f3801529ad334ec6bd0cfda`
passed native clippy and the release build, but failed creating the WebView2 session
after 60 seconds. Actions run `34012283721`, Windows job `101430054420`, artifact
`9982963113` recorded exact matching WebView2 and EdgeDriver `151.0.4129.101`.
The proxy log contained only `hyper::Error(IncompleteMessage)` after the timeout;
it did not establish whether the application started or exited.

The follow-up launches the signed Microsoft driver directly with the exact Windows
WebView2 capability translation and automation flags from the pinned Tauri driver.
It removes the unused driver Cargo install, captures native verbose output, records
the failing stage and descendant process state, and preserves the 60-second session
timeout. No application configuration, capability, or security policy changes.
The probe terminates only the process tree rooted at its own driver PID.
The amended hosted result is still required before closing P01.
