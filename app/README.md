# app

The desktop shell uses Tauri 2, React, and TypeScript. Phase 0 renders a static
placeholder. Astra owns `app/**`, including the native shell, capabilities, and
frontend configuration. This scaffold is the starting point for the poker interface.

Generated with `create-tauri-app@4.7.4`
(`--template react-ts --manager pnpm --tauri-version 2`), then restricted to the
boundary below.

## Run and test

From the repository root:

```text
pnpm install --frozen-lockfile
pnpm dev                              # browser preview on localhost:1420
pnpm tauri dev                        # native window with development CSP
pnpm tauri build --no-bundle -- --locked
pnpm test
pnpm lint
pnpm format:check
pnpm typecheck
pnpm build
```

On this PC, use `pnpm.cmd` in PowerShell. Smart App Control prevents the Rust
compiler from starting and remains enabled by Caleb's decision. Frontend checks
run locally; GitHub Actions builds the native Windows app. A build does not verify
that a WebView renders or enforces its runtime boundary.

`src/smoke.test.ts` renders the component to static markup and checks the selected
capability, empty API grant, registered-command inventory, and exact production and
development CSP directives. These are regression checks on the checked-in files.
They do not inspect the running WebView or parse arbitrary future Rust source.

## Security boundary

**Application commands: none.** `src-tauri/src/lib.rs` registers no
`invoke_handler` and defines no `#[tauri::command]`. The generated `greet` command
was removed. Tauri retains its internal IPC machinery; absence of application
commands does not mean that the framework has no internal commands.

**Added plugins: none.** The generated opener plugin and all its registrations,
dependencies, and grants were removed. There are no filesystem, shell, HTTP,
dialog, SQL, or updater plugins in this scaffold.

**Granted API permissions: none.** The single selected capability is `default`:
`local: true`, `windows: ["main"]`, `permissions: []`, with no `remote` block.
`app.security.capabilities: ["default"]` selects it explicitly, so adding another
capability file does not activate it. The configured window's label is explicitly
`main`. Tauri otherwise enables every capability file by default.
[Capability selection](https://v2.tauri.app/security/capabilities/).

The static frontend calls no Tauri API. It needs no `core:*:default` grants to draw
HTML and CSS. Those sets grant callable APIs, rather than enabling the existence of
a native window: for example, the core event default grants listen and emit calls,
and the image default grants image construction and loading calls.
[Tauri 2.11.5 permission source](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri/build.rs).

**Production CSP** in `src-tauri/tauri.conf.json`:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none';
base-uri 'self'; form-action 'none'; frame-ancestors 'none'
```

`'self'` permits bundled assets. The two IPC origins support Tauri's transport.
Tauri adds hashes and nonces for bundled scripts and styles during compilation.
The frontend loads no remote fonts, scripts, or images and makes no web requests.
[Tauri CSP behavior](https://v2.tauri.app/security/csp/).

**Development CSP** is separate. It adds `http://localhost:1420`,
`ws://localhost:1420`, `ws://localhost:1421`, and the inline script/style allowance
Vite uses during development. The production policy contains none of those Vite
allowances. The tests compare complete directive maps, so a bare `https:` source,
an `ipc.localhost.evil` hostname, or an extra directive cannot pass a prefix check.

The native runtime checks pass at `0d4f338`: the release placeholder renders
without unexpected CSP errors, an ungranted core API call is rejected, and an
external request is blocked through the tested frontend path. This verifies the
scaffold boundary; it does not provide a system network sandbox or a completed
application security audit. The exact evidence is recorded below.

## Adding a command

Registering a custom command alone makes it callable by all app windows by
default. A capability grant is not sufficient to change that default. Follow
Tauri's [application command manifest instructions](https://v2.tauri.app/security/capabilities/)
when adding the first real command:

1. Define and validate the backend command, including resource limits and its
   input/output contract. Register it through `invoke_handler` in `src/lib.rs`.
2. In `build.rs`, use `tauri_build::Attributes::new().app_manifest(...)` with
   `tauri_build::AppManifest::new().commands(&["command_name"])`. Keep that list
   complete for every registered application command.
3. Define a named permission under `src-tauri/permissions/` whose
   `commands.allow` contains that command. Grant the permission identifier only
   to the intended local window in the selected capability.
4. Update this inventory and the static guards. Add positive and negative runtime
   tests: the intended caller succeeds, an ungranted caller fails, and invalid
   inputs fail without starting privileged work.

No application manifest command list is needed while there are no application
commands. The smoke test intentionally rejects any registration until this
procedure and its replacement tests have been reviewed.

## Layout and build ownership

`src/` contains the React placeholder and tests. `src-tauri/` contains the native
window entry point, capability, configuration, and its own committed Cargo lockfile.
It is excluded from the root Rust workspace; Windows CI formats, lints, and builds
it separately. Core Rust and the frontend are checked on both Windows and Linux.

CI passes `--locked` to Cargo through the Tauri CLI's argument separator and checks
that all three committed lockfiles remain unchanged after the build.
[Tauri CLI argument forwarding](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/interface/rust/desktop.rs).

## Native runtime checks in Windows CI

**Verified:** [run 34015308353](https://github.com/calebroot2006-art/SOLVER/actions/runs/34015308353)
at `0d4f338d5e1b62bd8af25ce3580a6f7c3c252a26` passes native build, runtime
probes, account cleanup, and unchanged-lockfile checks. Astra inspected the release
screenshot, actual ACL denial, and enforced `connect-src` event for the external
probe URL. The [final implementation review](../docs/reviews/2026-09-05-astra-phase-0-1-implementation.md)
records the scoped verdict and links to preserved evidence. Earlier failures below
explain the test-driver corrections and are closed by this result.

After building the release binary, CI runs `scripts/setup-webdriver.ps1` and
`scripts/run-native-smoke.ps1` with PowerShell. The setup uses a Microsoft-signed EdgeDriver
matching the selected installed WebView2 build. The probe starts it directly with
the WebView2 capabilities that `tauri-driver` 2.0.6 translates on Windows and the
same Tauri automation environment flags. Direct startup preserves verbose native
driver diagnostics, including failures before a session exists.
If the runner's driver does not match, setup downloads that exact runtime version's
driver from Microsoft's HTTPS distribution endpoint. The tools live in runner
temporary storage. The application receives no test plugin, added capability,
configuration override, or browser security exception.
[Tauri manual WebDriver setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/),
[Microsoft version matching](https://learn.microsoft.com/en-us/microsoft-edge/webdriver/).

The launcher records the runner token. On an elevated hosted runner, it creates a
disposable standard account, stages a byte-identical release executable and the
test tools under `RUNNER_TEMP`, and uses `Start-Process -Credential` to run the
probe. Only that staging directory receives an account-specific filesystem grant.
The child must prove Medium Integrity Level before starting the driver. The
launcher removes its account, profile, processes, and staging after collecting
evidence; cleanup failure fails the job. Its password exists only in memory.
This launcher refuses execution outside a GitHub-hosted Windows runner.
[PowerShell alternate credentials](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process).

This addresses the observed High Integrity Level in run `34013757158`, commit
`8d24ed6324fde70957bd3af354b199617080117b`: the runner's token contained
`S-1-16-12288`. Both that run and the preceding direct-driver run failed session
creation with `DevToolsActivePort file doesn't exist`. Microsoft documents that
elevated WebView2 hosts ignore the environment overrides used by external drivers.
The standard-user launcher resolves this prerequisite in the final passing run.
[WebView2 privilege behavior](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags).

Run `34014455779` proved the new account runs at Medium Integrity Level and that
account cleanup completes. The probe stopped before driver startup because the
alternate-credential launch discarded its environment overrides. Test paths and
the commit now pass through `native-launch.json` in the disposable staging folder;
the file contains no credentials. The final run verifies the native security probes.

Run `34014953971` then rendered the release placeholder with its CSS and no load
or CSP errors, captured a screenshot, and received the expected ACL denial. The
test client incorrectly interpreted that returned application's `error` field as
a WebDriver failure. Response classification now uses HTTP status, preserving
successful script return values. Run `34015308353` then passes the external-request
CSP probe and closes the runtime gate.

The external driver launches the release executable, then observes a fresh page
load with error and CSP listeners installed before the page's scripts run. It
checks the heading and computed CSS layout, requires an authorization rejection
from the real `plugin:app|version` command, and requires an enforced `connect-src`
violation naming `https://astra-csp-probe.invalid/scaffold-runtime-check`. A fetch
failure without that CSP event fails the test.

`app/test-results/` contains the screenshot, binary/tool hashes, runtime versions,
page diagnostics, individual probe results, and verbose driver logs. Failures also
record the stage and the test driver's descendant process command lines and state.
CI uploads them as
`native-runtime-windows` for seven days. Tests for evidence classification run
locally with `pnpm test`; only the hosted native run proves the WebView behavior.
See the [scaffold report](../docs/reviews/2026-09-05-astra-scaffold-hardening.md)
for the actual verification status.
