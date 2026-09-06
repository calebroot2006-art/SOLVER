---
type: review-handoff
from: Claude (Fable)
to: Astra
date: 2026-09-05
status: ready-for-review
branch: solver/bootstrap
---

# Review handoff: phase 0 bootstrap and the Tauri scaffold

## Problem addressed and expected behaviour

Phase 0 of `PLAN.md` (steps 2 to 5): a buildable, CI-checked Rust workspace plus the Tauri
2 desktop scaffold, cut to the security boundary your finding P01 asked for, with the CI
policy from P02. Expected behaviour: the whole gate passes on a clean checkout on Windows
and Linux without rewriting a lockfile, and the scaffold exposes no command, no plugin, and
no remote origin. This handoff asks for two closures, P01 and P02, and for the transfer
decision on `app/`.

## What to review

Branch `solver/bootstrap` on `git@github.com:calebroot2006-art/SOLVER.git`. The main
checkout in this folder is on that branch now, so the files below are the ones on disk.

| Commit | What it holds |
|---|---|
| `8bba8c4` | Rust workspace: `Cargo.toml`, `rust-toolchain.toml`, `config/solver.toml`, eleven crates, `tests/` (`toygames`) |
| `111f91e` | Tauri scaffold with the boundary closed: `app/**` |
| `c9b791b` | CI: `.github/workflows/ci.yml` |
| `1d22834` | Root README, `.editorconfig`, `.env.example`, `.gitattributes`, `.gitignore` repair |
| `d77d84c` | Capability grant made a provable subset of `core:default` |
| `7c5e4a5`, `763faba` | `PLAN.md` progress notes |
| `8b7045e` | Merge of `main` (one CLAUDE.md line) so the branch carries everything |

`git diff main...solver/bootstrap --stat` is the full change: 93 files. The generated
scaffold is small enough to read whole; the skill folders under `.claude/` and `.agents/`
are unchanged from `main` and not part of this review.

### Where to look first

1. `app/src-tauri/src/lib.rs`, `app/src-tauri/capabilities/default.json`,
   `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml`, `app/package.json`. Five
   files carry the whole boundary.
2. `app/src/smoke.test.ts`. The boundary guards: they read the checked-in files and fail
   if a plugin package, an `invoke_handler`, a `.plugin(` call, a `remote` block,
   `core:default`, a wildcard, or a non-IPC host comes back.
3. `.github/workflows/ci.yml` against your P02 list.
4. `Cargo.toml` (`resolver = "3"`, exact pins) and the three committed lockfiles.

## Scaffold inventory (P01 closure)

Exhaustive: anything not listed is absent. Copied from `app/README.md` and checked
against the files at `763faba`.

**Tauri commands: none.** `lib.rs` has no `invoke_handler` and no `#[tauri::command]`.
The generator's `greet` demo was deleted. No command name is reachable over IPC.

**Plugins: none.** `tauri-plugin-opener` is gone from `src-tauri/Cargo.toml`, from the
builder in `lib.rs`, from the capability, and from `package.json`. `src-tauri/Cargo.lock`
has no package matching `opener`. No HTTP, shell, filesystem, dialog, SQL, or updater
plugin was added.

**Capabilities.** One file, `capabilities/default.json`: `local: true`,
`windows: ["main"]`, no `remote` block, six permissions listed by name:

| Permission | Why it is there |
|---|---|
| `core:app:default` | App metadata the webview reads at startup |
| `core:event:default` | Tauri's own event channel |
| `core:image:default` | Image handling the core API uses |
| `core:path:default` | Path helpers, no filesystem access of their own |
| `core:webview:default` | The webview the window hosts |
| `core:window:default` | The main window |

That is `core:default` minus `core:menu:default` and `core:tray:default`. The membership
of `core:default` was read from the `PLUGINS` table in `tauri` 2.11.5's `build.rs`, which
generates the umbrella from it. The smoke test pins the same list, so the grant can only
shrink without a test change.

**Production CSP** (`app.security.csp`):

```
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none';
base-uri 'self'; form-action 'none'; frame-ancestors 'none'
```

`ipc:` and `http://ipc.localhost` are the two forms of the Tauri IPC origin. No remote
host. `csp: null` is gone. `pnpm build` emits a linked stylesheet and a module script with
nothing inline in `dist/index.html`, which is why `'self'` alone is enough.

**Development CSP** (`app.security.devCsp`, `tauri dev` only): adds `http://localhost:1420`,
`ws://localhost:1420`, `ws://localhost:1421`, and `'unsafe-inline'` for Vite's injected
scripts and styles. None of it reaches a built app.

**Network allowances beyond the above: none.** The frontend imports nothing from
`@tauri-apps/api`, calls no `fetch`, loads no remote asset. The Rust shell has no HTTP
client. `@tauri-apps/api` remains in `package.json` for the first real command; nothing
imports it, so nothing of it is bundled.

**Versions.** `create-tauri-app@4.7.4` (`--template react-ts --manager pnpm
--tauri-version 2`); tauri crate 2.11.5, tauri-build 2.6.3, CLI 2.11.4, API 2.11.1;
Rust 1.98.1; Node 24.19.0; pnpm 12.3.4. Every dependency is an exact pin.

## Commands run and results

Nothing that invokes `rustc` can run on this PC (Smart App Control, see the root README),
so the gate ran in GitHub Actions on a clean checkout. Two runs so far:

| Run | Commit | Result |
|---|---|---|
| https://github.com/calebroot2006-art/SOLVER/actions/runs/34008667766 | `7c5e4a5` | both jobs green, 9 min 47 s |
| https://github.com/calebroot2006-art/SOLVER/actions/runs/34009162933 | `763faba` (a `PLAN.md` note) | both jobs green, 5 min 40 s with a warm cache |

What run 34008667766 did, from its logs (job `check (windows-latest)` 101420441296 and
`check (ubuntu-latest)` 101420441161):

| Step | Linux | Windows |
|---|---|---|
| `rustup show` installs the pinned toolchain | rustc 1.98.1 | rustc 1.98.1 |
| `cargo fmt --all --check` | pass | pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | pass |
| `cargo test --workspace --locked` | 12 test binaries, 1 test each, all pass | same |
| `pnpm install --frozen-lockfile` | pass | pass |
| `pnpm format:check`, `lint`, `typecheck` | pass | pass |
| `pnpm test` | 1 file, 7 tests pass | same |
| `pnpm build` (production frontend) | pass | pass |
| `cargo fmt --check` in `app/src-tauri` | skipped by design | pass |
| `cargo clippy --locked -- -D warnings` in `app/src-tauri` | skipped by design | pass |
| `pnpm tauri build --no-bundle` | skipped by design | pass, `app\src-tauri\target\release\app.exe` built |

No lockfile was rewritten: every cargo step ran with `--locked` and pnpm with
`--frozen-lockfile`, and each would have failed otherwise.

Run locally on this PC before the push: `pnpm install --frozen-lockfile`,
`format:check`, `lint`, `typecheck`, `test` (7 tests), `build`; `actionlint` 1.7.12 on
the workflow; `slopcheck.py` on the fourteen READMEs (0 banned, 0 review).

One line of noise in the Windows log: `pnpm/action-setup`'s own installer prints Node's
`DEP0190` deprecation warning about `shell: true`. It comes from the action, not from
this repository, and did not appear on Linux.

## The check that cannot be done on this PC

`pnpm tauri dev` and the release binary cannot launch here: the Tauri CLI dies probing
`rustc -vV`, and no machine that can launch the app exists yet. So the runtime half of
your P01 closure is **not verified**:

* the placeholder renders in the WebView with no CSP violation in the console;
* an ungranted command invoked from the frontend fails;
* an external web request from the frontend fails.

The static half is verified by CI: the release build links, and the boundary tests pass.
If you have a machine that can run `pnpm tauri dev`, those three checks are yours to run
and record. If not, the handoff stays open on that point and says so; nothing in phase 1
depends on it.

## Known limitations

* `core:webview:default` and `core:image:default` are granted because they are members of
  `core:default`. A static page may need neither. Trimming them wants a running build to
  confirm nothing breaks; the smoke test already allows a shorter list.
* The boundary tests are file reads, not a runtime probe. They catch a re-added plugin or
  command; they do not observe the WebView.
* `bundle.active: true` with `targets: "all"` is the generator's default and is unused
  because CI builds with `--no-bundle`. An installer is a phase 12 concern.
* `app/src-tauri` is outside the Cargo workspace, so `cargo test --workspace` never sees
  it. It has no tests today; CI formats, lints, and builds it natively on Windows.

## Open decisions

* Transfer of `app/**` to you, once you accept the scaffold. Until then the executor's
  branch is where any scaffold repair goes, and nobody else edits it.
* Whether to trim the two permissions above now or after the first real screen.

## Files still being edited

None on this branch. Phase 1 (CFR on Kuhn and Leduc) starts on `solver/cfr-toy-games`,
branched from here, and touches `crates/payoff`, `crates/postflop`, `tests/`, and
`config/`. It does not touch `app/`. The old executor worktree at
`.claude/worktrees/agent-a5c19d7e711c4fc07` is at the same commit and is removed after
the merge.

## Requests to Astra

1. P01 closure: the inventory above against the files, and the three runtime checks if you
   can launch the app.
2. P02 closure: the workflow and the two CI runs against your policy list.
3. The transfer decision: `verified for the stated scope` (with `app/` becoming yours) or
   `needs changes`, in this folder.
