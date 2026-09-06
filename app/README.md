# app

The desktop shell: Tauri 2 with React and TypeScript. Phase 0 renders a static
placeholder and calls nothing. Astra owns this folder once the scaffold is accepted,
and everything below is a starting point for that design work, not a design.

Generated with `create-tauri-app@4.7.4`
(`--template react-ts --manager pnpm --tauri-version 2`), then cut back to the
security boundary Astra's finding P01 asked for.

## Run

From the repository root, so pnpm resolves the workspace:

```
pnpm install --frozen-lockfile
pnpm dev              # Vite on http://localhost:1420, browser only
pnpm tauri dev        # the desktop window, with the dev CSP
pnpm tauri build --no-bundle    # the release binary, no installer
```

## Test

```
pnpm test          # vitest, from the root or from app/
pnpm lint          # eslint
pnpm format:check  # prettier
pnpm typecheck     # tsc --noEmit
pnpm build         # tsc --noEmit && vite build
```

`src/smoke.test.ts` covers two things: the placeholder renders something, and the
security boundary below still holds. The boundary tests read the checked-in files,
so they catch a plugin or a command creeping back in. They say nothing about
runtime behaviour; the release-build and ungranted-command checks are Astra's.

## Security boundary

The inventory Astra's P01 closure asks for. It is exhaustive: anything not listed
here is absent.

**Tauri commands: none.** `src-tauri/src/lib.rs` has no `invoke_handler` and no
`#[tauri::command]`. The generator's `greet` demo was deleted. With no command
registered, no command name is reachable over the IPC bridge.

**Plugins: none.** The generator's `tauri-plugin-opener` was removed from
`src-tauri/Cargo.toml`, its `.plugin(...)` registration removed from `lib.rs`, its
`opener:default` permission removed from the capability, and `@tauri-apps/plugin-opener`
removed from `package.json`. No HTTP, shell, filesystem, dialog, SQL, or updater
plugin was added. `src-tauri/Cargo.lock` contains no package matching `opener`.

**Capabilities.** One file, `src-tauri/capabilities/default.json`: `local: true`,
`windows: ["main"]`, no `remote` block. Permissions are listed one at a time rather
than through the `core:default` umbrella, so a reviewer can read the grant without
expanding a set:

| Permission               | Why it is here                                      |
| ------------------------ | --------------------------------------------------- |
| `core:app:default`       | App metadata the webview reads at startup           |
| `core:event:default`     | Tauri's own event channel between shell and webview |
| `core:image:default`     | Image handling the core API uses                    |
| `core:path:default`      | Path helpers, no filesystem access of their own     |
| `core:resources:default` | Lifetime management for core-created resources      |
| `core:webview:default`   | The webview the window hosts                        |
| `core:window:default`    | The main window itself                              |

`core:menu:default` and `core:tray:default` are deliberately absent: this shell has
neither a menu nor a tray icon.

**Production CSP** (`app.security.csp` in `src-tauri/tauri.conf.json`). Tauri adds
its own hashes for its initialisation script on top of this:

```
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none';
base-uri 'self'; form-action 'none'; frame-ancestors 'none'
```

`'self'` is the bundled-asset origin. `ipc:` and `http://ipc.localhost` are the two
forms of the Tauri IPC origin (the second is the Windows one). No remote host
appears in any directive. `csp: null`, which the generator ships, is gone.

`pnpm build` emits the stylesheet as a linked file and the app code as a module
script, with no inline `<style>` or inline `<script>` in `dist/index.html`, which is
why `script-src` and `style-src` need nothing beyond `'self'`.

**Development CSP** (`app.security.devCsp`, applied only under `tauri dev`). It adds
`http://localhost:1420` for the Vite dev server, `ws://localhost:1420` and
`ws://localhost:1421` for hot reload, and `'unsafe-inline'` for the scripts and
styles Vite injects while developing. Those allowances exist in `devCsp` only and
never reach a built app.

**Network allowances: none beyond the above.** The frontend imports nothing from
`@tauri-apps/api`, calls no `fetch`, and loads no remote font, script, or image. The
Rust shell has no HTTP client. `@tauri-apps/api` stays in `package.json` because the
first real command will need it; nothing imports it today, so nothing of it is
bundled.

This is a scaffold boundary, not a system network sandbox. It says what the app is
configured to allow. It does not stop the operating system, and it makes no claim
about a completed application security audit.

### Adding a command later

Three edits, in this order, or the command is either unreachable or ungoverned:

1. Register it in `src-tauri/src/lib.rs` through `invoke_handler`.
2. Grant it by name in `src-tauri/capabilities/default.json`.
3. Add it to the inventory above.

`src/smoke.test.ts` fails until step 3, which is the point.

## Layout

```
app/
├── index.html                     Vite entry
├── src/
│   ├── main.tsx                   React mount
│   ├── App.tsx                    the placeholder screen
│   ├── App.css                    enough style to prove the bundled CSS loads
│   ├── placeholder.ts             its content, so the test has real code to read
│   └── smoke.test.ts              placeholder test plus the boundary guards
└── src-tauri/
    ├── Cargo.toml                 excluded from the root Cargo workspace
    ├── Cargo.lock                 committed
    ├── src/lib.rs                 the window, and nothing else
    ├── capabilities/default.json  the grant above
    └── tauri.conf.json            window, CSP, dev CSP, bundle
```

`src-tauri` is excluded from the root Cargo workspace so a Linux CI runner never has
to install webkit2gtk. It is formatted, linted, and built natively on the Windows CI
runner instead; see `.github/workflows/ci.yml`.
