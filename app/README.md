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

This boundary does not provide a system network sandbox or a completed security
audit. The remaining native runtime checks are: the release placeholder renders
without CSP errors, an ungranted core API call is rejected, and an external web
request is rejected through the tested frontend path. CI compilation alone does
not establish any of these results.

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
