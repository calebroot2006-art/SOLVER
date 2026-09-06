# GTO Solver APP

A no-limit hold'em GTO solver and a trainer built on top of it, for Caleb. The solver
is a Rust workspace; the app is Tauri 2 with React and TypeScript. `docs/PRODUCT.md`
says what it is for, `docs/ROADMAP.md` says what gets built in what order, and each
phase has its own `PLAN.md`.

This file is the cold start: what is in the repository, how to run it, and which
versions everything is pinned to.

## Read this before running anything on Caleb's machine

**Smart App Control is on, and it stops the Rust compiler from starting.** Windows
Smart App Control refuses to load unsigned DLLs. Every Rust compiler binary loads two
of them, `rustc_driver-*.dll` and `std-*.dll`, so `rustc`, `rustfmt`, and
`clippy-driver` all fail at process start with `0xC0000142`
(`STATUS_DLL_INIT_FAILED`), and `cargo` reports `rustc -vV` exiting `0xC0E90002`.
`Microsoft-Windows-CodeIntegrity/Operational` records it as event 3077 with a Smart
App Control block (event 3118) beside it.

`cargo` itself is statically linked and runs, so dependency resolution and
`cargo generate-lockfile` work. Nothing that invokes the compiler does: no `cargo
build`, `cargo test`, `cargo clippy`, `cargo fmt`, or `tauri build`. Node, pnpm, and
the whole frontend toolchain are unaffected.

**Caleb's decision: Smart App Control stays on.** Use GitHub Actions for Rust
compilation and tests. Astra independently verified the baseline `d256637` in
[CI run 34009574830](https://github.com/calebroot2006-art/SOLVER/actions/runs/34009574830).
Both platform jobs passed, including the Windows native Tauri build. Desktop runtime
checks remain separate from compilation. The current implementation and review status
is recorded in `PLAN.md`; local compiler failures do not invalidate observed CI results.

## Layout

```
/
├── Cargo.toml           virtual Cargo workspace: crates/* and tests
├── rust-toolchain.toml  the exact compiler both CI runners install
├── config/solver.toml   solve targets, iteration caps, logging, DCFR parameters
├── crates/              the solver, one crate per concern (map below)
├── tests/               the toygames package: Kuhn, Leduc, reference comparison
├── app/                 the Tauri 2 desktop app (Astra's, after scaffold acceptance)
├── docs/                PRODUCT.md, ROADMAP.md, research/, reviews/, astra/
├── package.json         pnpm workspace root; scripts delegate to pnpm -r
└── .github/workflows/   CI: the phase 0 gate on Windows and Linux
```

### The crates

The map is `docs/ROADMAP.md`'s architecture section. Phase 0 supplied the crate
skeletons. Phase 1 is filling in `payoff`, `postflop`,
and `tests`; `PLAN.md` records which numerical gates have actually passed.

| Crate | What it holds | Filled in by |
|---|---|---|
| `cards` | Cards, decks, combos, 7-card evaluation, range parsing | Phase 2 |
| `tree` | Bet-size DSL, action tree, all-in threshold, bet-size translation | Phase 3 |
| `postflop` | The CFR solver, public-state tree, terminal sweep, isomorphism, compression | Phases 1, 3, 4 |
| `bestresponse` | Exploitability on the public-state tree, as a percentage of pot | Phase 3 |
| `payoff` | Terminal payoffs: chip EV, then ICM and bounty-adjusted ICM | Phases 1 and 10 |
| `preflop` | Multiway MCCFR with bucketed postflop; the range charts we ship | Phase 11 |
| `engine` | The hand engine: 2 to 9 seats, blinds, antes, side pots, tournaments | Phase 6 |
| `bots` | Policies from the spot library, translation, bounded live re-solve | Phase 8 |
| `spots` | The solved-spot and chart formats, the library index, the generator CLI | Phase 5 |
| `coach` | Grading, explanation facts, templates, the model client, leak tracking | Phase 9 |
| `wasm` | The wasm-bindgen wrapper, kept compiling from the start | Phase 13 |

`tests/` is the package `toygames`. The directory keeps the roadmap's name; the
package is named for what it runs.

`crates/spots` is the contract between the solver and the app. The UI reads solved
spots through that format and never reaches into solver internals.

## Commands

Run these from the repository root.

### Rust

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

`--locked` fails rather than quietly updating `Cargo.lock`, which is committed.
`app/src-tauri` is excluded from the workspace, so those three commands never touch
it; it has its own committed lockfile and is checked on its own:

```
cd app/src-tauri
cargo fmt --check
cargo clippy --locked -- -D warnings
```

### The app

```
pnpm install --frozen-lockfile
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

```
pnpm dev                      # Vite alone, in a browser, no desktop shell
pnpm tauri dev                # the desktop window, dev CSP, hot reload
pnpm tauri build --no-bundle -- --locked  # release binary, without an installer
```

The root scripts delegate through `pnpm -r`, so the same command works from the root
and from `app/`.

### The CI gates

CI runs Rust formatting separately from the solver and app jobs. Solver and frontend
checks run on Windows and Ubuntu. The native Tauri checks and release WebView tests
run on Windows. A failed formatter produces a patch artifact; the check still fails
until the formatted source is committed and passes a new run.

The test profile uses optimization level 2 for the long CFR reference gates, with
overflow checks and debug assertions retained. No accuracy test is ignored.
Each solver job uses pinned Python/OpenSpiel to replay generated Leduc states and
independently evaluate their actual policies. The original reference captures,
rounding diagnosis, and fixed accuracy gates are documented in
[tests/README.md](tests/README.md).
See [app/README.md](app/README.md) for the external-driver runtime checks and their
screenshot and diagnostic artifacts.

## Pinned versions

Every version below is pinned in a file, not just recorded here. Change the file and
this table together.

| Thing | Version | Pinned in |
|---|---|---|
| Rust toolchain | 1.98.1 | `rust-toolchain.toml` |
| Node | 24.19.0 | `.nvmrc`, `engines.node` |
| pnpm | 12.3.4 | `packageManager`, `engines.pnpm` |
| Python reference runtime | 3.12.10 | `.github/workflows/ci.yml` |
| OpenSpiel and reference dependencies | 2.0.2 and exact requirements | `tests/reference/openspiel/requirements.txt` |
| Cargo dependencies | exact `=` requirements | `[workspace.dependencies]`, `Cargo.lock` |
| npm dependencies | exact, no carets | `app/package.json`, `pnpm-lock.yaml` |
| Tauri | crate 2.11.5, build 2.6.3, CLI 2.11.4, API 2.11.1 | `app/src-tauri/Cargo.toml`, `app/package.json` |
| CI actions | full commit SHAs | `.github/workflows/ci.yml` |

The app was generated with `create-tauri-app@4.7.4`
(`--template react-ts --manager pnpm --tauri-version 2`), then cut back to the
security boundary in `app/README.md`. The generator is not a dependency and is not
run again.

Lockfiles are committed on purpose: `Cargo.lock`, `app/src-tauri/Cargo.lock`, and
`pnpm-lock.yaml`. Update a lockfile only for an intentional dependency change, review
the resolution, and commit it. CI uses locked resolution and rejects lockfile drift.

### What was installed on this machine, and how

This machine had no toolchain at all before phase 0. For the record, and so a second
machine can be set up the same way:

| Tool | Version | How |
|---|---|---|
| Visual Studio 2022 Build Tools | 17.14.39, MSVC 14.44.35207 | `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| rustup, Rust stable | rustup 1.29.1, Rust 1.98.1 | `rustup-init.exe` from `https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe`, SHA-256 checked against the published `.sha256`, run with `-y --default-toolchain stable --default-host x86_64-pc-windows-msvc --profile default --no-modify-path`, then `%USERPROFILE%\.cargo\bin` added to the user PATH |
| Node | 24.19.0 | `node-v24.19.0-win-x64.zip` from nodejs.org, SHA-256 checked against `SHASUMS256.txt`, extracted to `%LOCALAPPDATA%\nodejs` and added to the user PATH |
| pnpm | 12.3.4 | `corepack enable pnpm` then `corepack prepare pnpm@12.3.4 --activate` |
| WebView2 runtime | 152.0.4191.62 | already present |

Node came from the zip rather than `winget install OpenJS.NodeJS.LTS` because the
winget MSI stops on an elevation prompt, which a non-interactive session cannot
answer. The zip is the same 24.19.0 build, in a per-user directory, and needs no
administrator.

## Two assistants, one folder

**Current assignment:** Caleb asked Astra to take full control while Fable is
unavailable. Astra leads development across the project and assigns isolated work
through [the takeover plan](docs/astra/development-takeover/PLAN.md). The standing
folder split below applies when Fable returns; current task ownership takes priority.

**File ownership (agreed with Astra, 2026-09-05).** Fable's side owns the Rust
crates, `tests/`, `config/`, CI, root workspace files (`Cargo.toml`, `package.json`,
`pnpm-workspace.yaml`, lockfiles, `.gitignore`, root README). Astra owns all of
`app/**` (including `app/src-tauri/**`, capabilities, Tauri config, app package
files, app tests, assets, `app/README.md`) once the step 3 scaffold is reviewed and
accepted, plus `docs/astra/**`, `ASTRA.md`, `AGENTS.md`, and the scroll-craft skill
folders. During scaffold review the executor may repair its own scaffold in its
branch; Astra does not edit it concurrently. After transfer, Fable's executors enter
`app/` only for a bounded, file-specific task Astra hands over in writing. Root
lockfile changes that app work needs are coordinated through the handoff files.

## How the worktree flow works

A multi-file build is handed to an `executor` subagent with an agreed `PLAN.md`. The
executor runs in its own git worktree so it cannot collide with the main session or
with another executor:

1. The main session writes the plan into `PLAN.md` in the folder the work targets,
   from `templates/plan.md`, and records Caleb's answers under Decisions. An executor
   cannot ask a question, so anything unanswered is an assumption it will make alone.
2. The executor is spawned with `isolation: "worktree"`. Git creates
   `.claude/worktrees/agent-<id>/` on branch `worktree-agent-<id>`. That directory is
   gitignored.
3. The worktree contains no gitignored files: no `.env`, no `node_modules`, no
   `target/`, no generated tables, no solved spots. The executor installs or
   regenerates what it needs from the main checkout, whose path its brief names, and
   commits none of it.
4. The executor keeps `PLAN.md`'s Progress section current as it goes, so the next
   session can pick the work up cold.
5. The main session reads the diff, runs the tests itself, and runs `/code-review`
   before Caleb hears that anything is done. For solver work it also runs the
   accuracy checks itself. The executor's own report is evidence for that review, not
   the verdict.
6. On acceptance the branch is merged into the task branch and the worktree is
   removed with `git worktree remove`.

Branch names are `solver/`, `app/`, `trainer/`, or `docs/` plus a short description.

## Secrets

There are none, and `.env.example` is empty of variables for that reason. When one
arrives it goes in `.env`, which is gitignored, with a fake value in `.env.example`.
The app currently needs no secret and CI receives no application secret. The optional
read-only Actions helper in `docs/astra/development-takeover/ci_status.py` uses an
existing GitHub environment token or Git credential helper in memory. It does not
print or store credentials and removes authorization on cross-host redirects.
