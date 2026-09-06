---
project: gto-solver-app
type: plan
status: proposed
date: 2026-09-05
revision: 2
---

# Phases 0 and 1: bootstrap and CFR on Kuhn and Leduc

Research: [[how-to-build-a-solver]], [[solver-algorithms]], [[open-source-libraries]]

Written by the `planner` subagent and the main session on 2026-09-05. Revision 2 the same
day addresses Astra's findings P01 to P06 in
`docs/reviews/2026-09-05-astra-phase-0-1-plan-findings.md` and R01 and R05 in
`docs/reviews/2026-09-05-astra-research-and-program-plan-findings.md`. "Decisions" holds
Caleb's answers, with the date each one was given. Nothing else is assumed.

## Progress (updated 2026-09-05)

Read this first when picking the work up. It says what is done and verified, what is half
done, and what was learned that the plan below did not know. The executor updates it after
every step it finishes; the main session updates it after review.

**Where it stands:** step 1 is done on `main`. Steps 2 to 5 are being built by the phase 0
executor on branch `worktree-agent-a5c19d7e711c4fc07`, off commit `6a1b7d6`.

**Blocker found on 2026-09-05, before any Rust could be compiled: Smart App Control is
on.** This machine has Windows Smart App Control enforcing
(`HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy\VerifiedAndReputablePolicyState = 1`).
It refuses to load unsigned DLLs, and every Rust compiler binary loads two of them
(`rustc_driver-*.dll` and `std-*.dll`). So `rustc`, `rustfmt`, and `clippy-driver` all die
at process start with `0xC0000142` (`STATUS_DLL_INIT_FAILED`), and `cargo test` reports
`rustc -vV` exiting `0xC0E90002`. `Microsoft-Windows-CodeIntegrity/Operational` event 3077
names the blocked DLL and event 3118 is the Smart App Control block record. `cargo` itself
runs (it is statically linked), so dependency resolution and lockfile generation work;
nothing that invokes the compiler does. Node, pnpm, and the whole frontend toolchain run
fine.

Consequence: **no Rust command in this plan's gate can be run on this machine until Caleb
turns Smart App Control off** (Windows Security, App and browser control, Smart App
Control, Off). That switch is one way: Windows does not allow re-enabling it without
reinstalling Windows, so the executor did not touch it. Everything that does not need the
Rust compiler was built and run; every Rust command is written down as unverified, not
assumed to pass.

**Step 2 (Rust workspace): files written, compiler gate blocked.** `Cargo.toml` (virtual,
`resolver = "3"`, `members = ["crates/*", "tests"]`, `exclude = ["app/src-tauri"]`,
`[workspace.package]` edition 2024 and rust-version 1.98, `[workspace.dependencies]` with
`=` pins for serde 1.0.229, toml 1.1.5, thiserror 2.0.20, log 0.4.34, env_logger 0.11.11,
chrono 0.4.45, and `[workspace.lints]` for `missing_docs` and `clippy::all`);
`rust-toolchain.toml` pinning 1.98.1 with rustfmt and clippy; `config/solver.toml`; eleven
crates under `crates/` and the `toygames` package in `tests/`, each with a `//!` doc
paragraph, one `#[test]`, and a README. `Cargo.lock` was generated with
`cargo generate-lockfile` (which needs no compiler) and is committed.
`cargo fmt --all --check`, `cargo clippy ... -D warnings`, and `cargo test --workspace
--locked` are **not verified**: see the blocker above.

**Step 3 (app scaffold): done, except the two checks that need the Rust compiler.**
`pnpm create tauri-app` was run at the pinned generator version `create-tauri-app@4.7.4`
with `--template react-ts --manager pnpm --tauri-version 2`, then cut back to the P01
boundary: no commands (the `greet` demo is gone), no plugins (`tauri-plugin-opener` gone
from the Cargo manifest, the builder, the capability, and `package.json`), capabilities
listed one permission at a time for the local `main` window with no `remote` block, and a
production CSP plus a separate `devCsp`. `app/README.md` carries the full inventory.
Everything is pinned to an exact version; `pnpm-lock.yaml` and `app/src-tauri/Cargo.lock`
are committed. `pnpm install --frozen-lockfile`, `format:check`, `lint`, `typecheck`,
`test` (7 tests), and `build` all pass. `cargo fmt --check` and `cargo clippy` in
`app/src-tauri`, and `pnpm tauri build --no-bundle`, are **blocked** by Smart App Control:
the Tauri CLI panics while probing `rustc -vV`, which cannot start.

**Learned in step 3:** the generated template is exactly what Astra described from
upstream (`csp: null`, an opener plugin plus its permission, and a `greet` command), so
P01's concerns were real for this generator version, not hypothetical. `pnpm build` emits
the stylesheet as a linked file and the app as a module script with nothing inline, which
is why `script-src 'self'` and `style-src 'self'` are enough for the production CSP. The
smoke test now guards the boundary itself: it fails if a plugin, a command, a `remote`
capability, a wildcard, or a remote host reappears.

**Learned that the plan did not know:** the machine had no toolchain at all. rustup, the
MSVC build tools, Node, and pnpm were all installed by this executor; the root README
records the versions and how each was installed. Node had to come from the official zip
into `%LOCALAPPDATA%\nodejs` rather than winget, because the winget MSI stopped on a UAC
prompt this session cannot answer.

## Task

Turn the folder into a buildable, CI-checked Rust plus Tauri workspace (Phase 0), then
build the CFR core (vanilla, CFR+, DCFR behind one `Solver` trait) with a best-response
calculator, validated on Kuhn and Leduc against known values and OpenSpiel (Phase 1).

## Approach

Phase 0 is scaffolding, executed by `executor` (Opus) in a worktree after the main
session runs `git init` itself (a worktree cannot exist before the repo does). Phase 1 is
built in the main session (Fable) because it is numerical solver code.

Phase 1's central design choice: the `Game` trait is **vector-form over a public tree**
from day one, the shape hold'em needs. Per-node arrays over each player's private
states; chance nodes as public nodes that zero out impossible private states; terminal
values as a reach-weighted sum over opponent states. Kuhn and Leduc fit this form with 3
and 6 private states, and their O(n²) terminal evaluation becomes the brute-force
reference that Phase 2's Johanson sweep is tested against. The alternative, a scalar
per-history CFR like the Neller and Lanctot tutorial, is easier to write but is thrown
away in Phase 3 and would not exercise the normalisation and blocker logic hold'em
depends on. The best-response calculator lives in `crates/postflop` for Phase 1; the
`bestresponse` crate takes it over in Phase 3 when the public-state tree exists.
Everything is `f64` in Phase 1. The alias `Real` lives in `crates/payoff` (the
dependency-neutral bottom of the stack) and is re-exported by `postflop`. Changing it
later changes accumulation and metric precision as well as storage, so the Phase 4 move
to `f32` storage is measured against the `f64` baseline, not assumed.

**File ownership (agreed with Astra, 2026-09-05).** Fable's side owns the Rust crates,
`tests/`, `config/`, CI, root workspace files (`Cargo.toml`, `package.json`,
`pnpm-workspace.yaml`, lockfiles, `.gitignore`, root README). Astra owns all of `app/**`
(including `app/src-tauri/**`, capabilities, Tauri config, app package files, app tests,
assets, `app/README.md`) once the step 3 scaffold is reviewed and accepted, plus
`docs/astra/**`, `ASTRA.md`, `AGENTS.md`, and the scroll-craft skill folders. During
scaffold review the executor may repair its own scaffold in its branch; Astra does not
edit it concurrently. After transfer, Fable's executors enter `app/` only for a bounded,
file-specific task Astra hands over in writing. Root lockfile changes that app work
needs are coordinated through the handoff files.

## Steps

Steps 1 to 5 are Phase 0. Steps 2, 3, and 4 touch disjoint files and their edits can be
prepared as parallel executors; verification of step 4 runs after 2 and 3 are merged.
Step 5 follows them. Steps 6 to 10 are Phase 1; step 8b (reference capture) is
independent of all Rust work and **must complete before step 9's fixtures are written**.

1. **Initialise git** (main session, not the executor): write `.gitignore` first, then
   `git init -b main`, commit everything present as
   `Bootstrap: seed from planning session`. Create branch `solver/bootstrap` for Phase 0.
   Ignored from the start: `.claude/worktrees/`, `ASTRA-UPDATE.md`, `CLAUDE-UPDATE.md`
   (transient handoffs stay out of commits per `ASTRA.md`), `spots/`, `target/`,
   `node_modules/`, `.env`, `*.bin`, `*.spot`, `app/src-tauri/target/`, `app/dist/`,
   `**/__pycache__/`, `.venv/`. Lockfiles (`Cargo.lock`, `app/src-tauri/Cargo.lock`,
   `pnpm-lock.yaml`) are **committed**, never ignored.

2. **Rust workspace** (`Cargo.toml`, `rust-toolchain.toml`, `config/solver.toml`,
   `crates/<name>/{Cargo.toml,src/lib.rs,README.md}` for cards, tree, postflop,
   bestresponse, payoff, preflop, engine, bots, spots, coach, wasm;
   `tests/{Cargo.toml,src/lib.rs,README.md}`). Root is a virtual manifest with
   `resolver = "3"` (a virtual workspace does not inherit the resolver from member
   editions), `members = ["crates/*", "tests"]`, and `exclude = ["app/src-tauri"]` so
   Linux CI needs no webkit2gtk packages; the excluded Tauri crate keeps its own
   committed `Cargo.lock` and is checked natively on Windows in step 4.
   Workspace-level `[workspace.package]` (edition 2024, license `MIT OR Apache-2.0`,
   rust-version) and `[workspace.dependencies]` pinning `serde`, `toml`, `thiserror`,
   `log`, `env_logger`, `chrono` at exact versions recorded when added.
   `rust-toolchain.toml` pins the current stable by exact version (`channel = "1.NN.M"`,
   components rustfmt and clippy); the executor records the version in the root README.
   Each crate: `lib.rs` with a `//!` doc paragraph and one `#[test]` that compiles;
   `README.md` with what it does, how to run, how to test. The `tests` package is named
   `toygames` (directory stays `tests` per the roadmap). `config/solver.toml` holds
   `[solve] target_pct_of_pot = 0.5, max_iterations, check_every, log_every_secs,
   threads = 0` and `[dcfr] alpha = 1.5, beta = 0.0, gamma = 2.0`; the 0.5 default is
   the roadmap's Phase 3 gate and is a percentage (see step 7 for the exact formula).
   Gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked` pass on Windows with the committed lockfile.

3. **App scaffold** (`pnpm-workspace.yaml`, root `package.json` with `packageManager`
   pinned to an exact pnpm version, `.npmrc`, `.nvmrc` or `engines.node` with the exact
   Node version, `app/**` from `pnpm create tauri-app` at a recorded exact generator
   version, React plus TypeScript template, `app/eslint.config.js`, `app/.prettierrc`,
   `app/vitest.config.ts`, `app/src/smoke.test.ts`, `app/README.md`). Root scripts
   `test`, `lint`, `format:check`, `typecheck`, `build` delegate to `pnpm -r`. Pin every
   dependency to an exact version and commit `pnpm-lock.yaml` and
   `app/src-tauri/Cargo.lock`.

   **Security boundary, the acceptance criteria for this step (Astra P01):**
   * Replace the template page with a static placeholder. Remove the demo `greet`
     command, the opener plugin, its permission, and its dependency. Remove any other
     plugin the empty shell does not use.
   * `capabilities/default.json` grants only what the local main window needs (core
     defaults), enumerated explicitly. No remote capabilities. No HTTP, shell, fs, or
     dialog plugins.
   * Set a restrictive production CSP in `tauri.conf.json` (`default-src 'self'`,
     scripts and styles from bundled assets only, `connect-src` limited to the Tauri IPC
     origin, no remote hosts). Keep the development loopback and HMR allowances in the
     dev config only. `csp: null` is not accepted.
   * If any custom command remains, list it in the handoff and restrict it through the
     app manifest's command permissions with an explicit grant.
   * The handoff inventories every retained command, plugin, permission, and network
     allowance. Astra checks the generated files, that the release app loads without CSP
     errors, and that an ungranted command and an external web request fail from the
     frontend. This is the scaffold boundary, not a system network sandbox.

   Independent of step 2. After Astra accepts the scaffold, `app/` is Astra's.

4. **CI** (`.github/workflows/ci.yml`). Triggers: `push` and `pull_request`. Top-level
   `permissions: contents: read`; no secrets; no privileged execution of pull-request
   code or artifacts. Every external `uses:`, GitHub-owned actions included, pinned to a
   verified full commit SHA with the version in a comment. `actions/checkout` with
   `persist-credentials: false`. `shell: bash` on every run step.
   * **Both runners** (`windows-latest`, `ubuntu-latest`): `git config core.longpaths
     true` on Windows; `rustup show` (installs the pinned toolchain); `Swatinem/rust-cache`;
     `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`;
     `cargo test --workspace --locked`; `pnpm/action-setup` and `actions/setup-node` with
     the pnpm cache keyed on `pnpm-lock.yaml` (explicit `cache-dependency-path`);
     `pnpm install --frozen-lockfile`; `pnpm format:check`; `pnpm lint`; `pnpm typecheck`;
     `pnpm test`; `pnpm build` (the production frontend build).
   * **Windows only, native Tauri checks (Astra P02):** in `app/src-tauri`,
     `cargo fmt --check`, `cargo clippy --locked -- -D warnings`, and
     `pnpm tauri build --no-bundle` so the desktop crate compiles for real, not only
     `cargo check`.
   * The executor also runs the app once locally on Windows (`pnpm tauri dev`), confirms
     the placeholder renders with no CSP errors in the WebView console, and records it in
     the handoff.
   File edits are independent of steps 2 and 3; the workflow is verified after both are
   merged, on a clean checkout, without rewriting any lockfile.

5. **Root docs and hygiene** (`README.md`, `.gitattributes` with `* text=auto eol=lf` and
   binary rules, `.editorconfig`, `.env.example` with a comment that nothing is needed
   yet). Root README: what the workspace is, the crate map from `docs/ROADMAP.md`, the
   exact local commands, the pinned toolchain, Node, pnpm, and generator versions, how
   the worktree flow works, and the ownership split from "Approach" verbatim. Run
   `python .claude/skills/anti-ai-slop-writing/slopcheck.py README.md crates/*/README.md tests/README.md`.

6. **Payoff crate** (`crates/payoff/src/lib.rs`, `crates/payoff/README.md`).
   `pub type Real = f64;` lives here.
   `pub trait Payoff { fn utilities(&self, stacks_before: &[Real], contributions: &[Real], shares: &[Real], out: &mut [Real]); }`
   where `shares` sums to 1 over the players who take the pot. `pub struct ChipEv;`
   implements `out[i] = shares[i] * pot - contributions[i]` with `pot = Σ contributions`.
   **Contract for this phase:** utilities sum to zero across players (chip EV in a
   two-player game). The `Exploitability` API in step 7 requires this and is documented
   as such; rake, tournament equity, and any payoff with a constant offset need a
   separately reviewed metric (the general deviation-gain sum `Σ_i (br[i] − u_i(σ))`),
   not a silent reuse of `br[0] + br[1]`. Unit tests: utilities sum to zero for any
   split; a fold and a chopped showdown by hand; a non-finite input is rejected.
   `stacks_before` is unused by `ChipEv` and exists for the Phase 10 ICM implementation.

7. **CFR core** (`crates/postflop/src/{lib.rs,game.rs,solver.rs,cfr.rs,best_response.rs,config.rs,progress.rs,error.rs}`,
   `crates/postflop/README.md`; `crates/postflop/Cargo.toml` depends on `payoff`).
   * `game.rs`: `pub use payoff::Real; pub type NodeId = u32;`
     `pub enum NodeKind { Player { player: u8, num_actions: u8 }, Chance { num_outcomes: u16 }, Terminal }` and

     ```rust
     pub trait Game {
         fn num_nodes(&self) -> usize;
         fn root(&self) -> NodeId;
         fn kind(&self, node: NodeId) -> NodeKind;
         fn child(&self, node: NodeId, index: usize) -> NodeId;
         fn num_private_states(&self, player: usize) -> usize;
         fn initial_weights(&self, player: usize) -> &[Real];            // unnormalised, nonnegative
         fn compatible(&self, p0_state: usize, p1_state: usize) -> bool; // card conflicts
         fn chance_prob(&self, node: NodeId, outcome: usize) -> Real;    // physical outcome probability
         fn chance_mask(&self, node: NodeId, outcome: usize, player: usize) -> &[Real]; // 0 or 1 per state
         fn terminal_values(&self, node: NodeId, player: usize, opp_reach: &[Real], out: &mut [Real]);
         fn starting_pot(&self) -> Real;                                  // > 0, finite
         fn info_label(&self, node: NodeId, player: usize, state: usize) -> String; // debugging
     }
     ```

     **Probability contract (Astra P04), documented on the trait and enforced by tests:**
     * An information set is `(public node, own private state)`. The public node encodes
       the full public history. Best response and strategy computation never read the
       opponent's private state when choosing an action.
     * `chance_prob` is the probability of the physical outcome (Leduc: 1/4 per board
       card among the four that are legal given both players' cards; the six physical
       cards are enumerated and the masks zero the two that conflict). Chance is applied
       exactly once per outcome on the path. Fixture: for every compatible private pair
       `(h0, h1)`, `Σ_b chance_prob(b) · mask0(b, h0) · mask1(b, h1) = 1`.
     * Reach vectors passed to `terminal_values` and to regret updates are **opponent
       reach**: the product of the opponent's strategy probabilities along the path,
       times chance probabilities along the path, times the opponent's initial weights,
       with the acting player's own mask applied at each chance node. Own reach used for
       strategy averaging contains only the player's own strategy probabilities (no
       chance, no opponent); impossible own states are masked separately.
     * `terminal_values` computes `out[h] = Σ_{h'} opp_reach[h'] · [compatible(h, h')] · u_player(h, h')`
       through the `Payoff`. Expected values and best-response values are normalised by
       the root normaliser `Z = Σ_{h,h'} w0[h] · w1[h'] · [compatible(h, h')]`, which must
       be finite and strictly positive; otherwise construction fails with
       `SolveError::EmptyGame`.
     * Validation at construction: vector lengths match `num_private_states`, weights
       are nonnegative and finite, child indices are in range, every player node has at
       least one action, every chance node has at least one outcome with positive
       probability, `starting_pot` is finite and positive.
   * `cfr.rs`: one `Cfr` struct with `enum Variant { Vanilla, Plus, Discounted { alpha, beta, gamma } }`;
     per player node, `regrets` and `strategy_sum` of length `states * actions`.
     **Regret storage (Astra R01):** cumulative regrets are stored **signed** for
     `Vanilla` and `Discounted`; the current strategy is regret matching over the
     positive part, `σ(a) = max(R(a), 0) / Σ_b max(R(b), 0)`, uniform when the
     denominator is zero. Only `Plus` floors the stored cumulative regret at zero after
     each of that player's updates (regret-matching+). One iteration `t` (1-based):
     traverse for player 0 with player 1's strategy frozen, accumulate regrets weighted
     by counterfactual value at information-set scope, accumulate
     `strategy_sum += own_reach · σ` (weight `t` for `Plus`), recompute player 0's
     current strategy; repeat for player 1; then for `Discounted`, in place over the
     whole accumulators: positive regrets `*= t^α/(t^α+1)`, negative regrets
     `*= t^β/(t^β+1)`, strategy sums `*= (t/(t+1))^γ`. The discount applies to the
     **entire accumulated sum after the iteration's contribution is added**, not to the
     new contribution alone; the README records Astra's three-iteration trace (first
     action probability 1/14 for the full accumulator versus 36/181 for
     contribution-only) as a unit test. The update order matches OpenSpiel's `cfr.py`
     for `Vanilla` and `Plus`, which is what makes the curve comparison in step 9 point
     for point. CFR+ uses linear (`t`) averaging as OpenSpiel does; the DCFR paper's
     quadratic framing is a variant, noted in the README. b-inary's γ=3 with a
     strategy-sum reset at powers of 4 is a further variant, not implemented here.
   * `solver.rs`: `pub trait Solver { fn iteration(&self) -> u64; fn run_iteration(&mut self, game: &dyn Game) -> Result<(), SolveError>; fn average_strategy(&self, game: &dyn Game) -> Strategy; }`
     and `pub fn solve(game, solver, cfg: &SolveConfig, on_progress: impl FnMut(&Progress)) -> Result<SolveReport, SolveError>`;
     `SolveReport { iterations, exploitability: Exploitability, elapsed, stop_reason: StopReason::{TargetReached, IterationCap} }`.
   * `best_response.rs` (Astra P03), for the two-player zero-sum chip game only:

     ```text
     br[i]       = max over player i's strategies of u_i(strategy_i, σ_{-i})   (chips per hand)
     nash_conv   = br[0] + br[1]                                                (chips per hand)
     average     = nash_conv / 2                                                (chips per hand)
     pct_of_pot  = 100 * average / starting_pot                                 (percent)
     ```

     `Exploitability { br_value: [Real; 2], nash_conv: Real, average: Real, pct_of_pot: Real }`.
     `nash_conv` equals OpenSpiel's `nash_conv`; `average` is Johanson et al.'s
     equation 3. Fixture: the uniform Kuhn profile gives `br = [1/2, 5/12]`,
     `nash_conv = 11/12`, `average = 11/24`, and with a two-chip pot
     `pct_of_pot = 22.9166...`. The constructor rejects a nonpositive or non-finite pot.
     Toy-game fixtures state their stop targets as **raw `nash_conv` in chips** and the
     config's `target_pct_of_pot` is converted explicitly; a test asserts the conversion
     both ways so the two are never confused.
   * `error.rs`: `SolveError::NonFinite { iteration, node, player }`,
     `SolveError::EmptyGame`, `SolveError::InvalidGame(String)`, `SolveError::Config(String)`.
     After every iteration the driver scans regrets and strategy sums for non-finite
     values and fails; a non-finite exploitability also fails.
   * `progress.rs`: log line via `log` at each `check_every` iterations and at least
     every `log_every_secs`: ISO-8601 timestamp, iteration, `nash_conv` in chips,
     `pct_of_pot`, elapsed seconds.
   * `config.rs`: `SolveConfig` and `DcfrParams` via `serde` and `toml`; `load(path)`.
     Rejected: missing fields, NaN or infinite values, negative values, `check_every = 0`,
     `max_iterations = 0`, `log_every_secs = 0`. `threads = 0` means "use all cores" and
     is the documented auto choice.

8. **Toy games and reference data** (`tests/src/{lib.rs,kuhn.rs,leduc.rs,nan_game.rs,history_oracle.rs}`,
   `tests/reference/openspiel/{capture.py,requirements.txt,kuhn_cfr.json,kuhn_cfr_plus.json,leduc_cfr.json,leduc_cfr_plus.json,provenance.json}`,
   `tests/README.md`).
   * 8a. `kuhn.rs`: 3 cards, ante 1, bet 1, pass or bet, OpenSpiel's `kuhn_poker` rules.
     `leduc.rs`: OpenSpiel's `leduc_poker` with explicit parameters
     `players=2, suit_isomorphism=false`: 6 cards (ranks J, Q, K, two each; states 2k and
     2k+1 share a rank), ante 1, raise 2 in round 1 and 4 in round 2, at most 2 raises
     per round, fold legal only when facing a raise, call always legal, pair beats high
     card by rank, ties split. **Round end:** a round ends after two checks with no
     raise, or when the remaining player calls after a raise; matching contributions at
     the start of a round do not end it. One board card with `chance_prob = 1/4` over
     the six physical cards and masks zeroing the two held. `nan_game.rs`: a two-node
     game whose terminal returns NaN. `history_oracle.rs`: a small scalar per-history
     CFR and best response over explicit deals (the Neller and Lanctot form) used only
     as an oracle for the vector-form implementation on unequal weights, sparse ranges,
     blocked boards, all-conflicting ranges, folds, and ties.
   * 8b. `capture.py`: `pip install open-spiel==<pinned>` (Windows wheels on PyPI; pin
     in `requirements.txt`), run `cfr.CFRSolver` and `cfr.CFRPlusSolver` on
     `kuhn_poker` and `leduc_poker(players=2,suit_isomorphism=false)`, record
     `exploitability.nash_conv(game, average_policy)` and `expected_game_score` for
     player 0 at iterations 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000,
     plus, for the final Leduc CFR+ profile, its own `nash_conv` (the reference
     residual). `provenance.json` records the OpenSpiel version, Python version, date,
     game strings, solver options, and the iteration definition. `tests/README.md`
     states this provenance and the 11/12 and 4.747222222222222 constants with their
     source URL. **This step runs and its outputs are inspected before step 9's
     fixtures are written** (Astra P05); the iteration budgets below are then set from
     the captured curves and recorded, not guessed.

9. **Tests** (`tests/tests/{kuhn.rs,leduc.rs,best_response.rs,oracle.rs,failure_paths.rs}`,
   `tests/fixtures/{kuhn.toml,leduc.toml}`): see Tests.

10. **Docs and review handoff** (`crates/postflop/README.md` with the numerical layout
    section: `f64` everywhere, signed regrets, in-place discounting, normalisation,
    units, the DCFR trace; `tests/README.md` with the measured iteration counts and the
    reference residuals; `docs/reviews/<date>-phase-0-bootstrap.md` and
    `docs/reviews/<date>-phase-1-cfr-toy-games.md` in the format of
    `docs/reviews/2026-09-05-phase-0-1-plan.md`). Run `slopcheck.py` on every authored
    document.

**Astra review points.** Revision 2 of this plan (targeted re-review of P01 to P06).
Then: the step 3 scaffold before transfer (P01 closure), the step 4 workflow on a clean
checkout (P02 closure), step 7's `Exploitability` fixtures and units (P03 closure), the
`Game` trait documentation and oracle fixtures (P04 closure), and step 9's captured
reference curves and budgets (P05 closure).

## Tests

Commands:

```
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
pnpm install --frozen-lockfile && pnpm format:check && pnpm lint && pnpm typecheck && pnpm test && pnpm build
cd app/src-tauri && cargo fmt --check && cargo clippy --locked -- -D warnings && cd ../.. && pnpm tauri build --no-bundle
cargo test -p toygames --locked -- --nocapture
RUST_LOG=info cargo test -p toygames --locked kuhn_dcfr -- --nocapture
python -m venv .venv && .venv/Scripts/pip install -r tests/reference/openspiel/requirements.txt && .venv/Scripts/python tests/reference/openspiel/capture.py
```

* **Phase 0 gate:** the first three command lines pass locally on Windows (line three is
  Windows-only in CI) and CI passes on both runners on a clean checkout without
  rewriting any lockfile. The local `pnpm tauri dev` launch is recorded in the handoff.
* **Best response, CFR-independent** (`best_response.rs`): a uniform-random strategy has
  `nash_conv` 11/12 on Kuhn and 4.747222222222222 on Leduc, within 1e-9 (OpenSpiel's
  `exploitability_test.py`); Kuhn `br = [1/2, 5/12]` and `pct_of_pot = 22.9166...`.
  Legal information-set counts, computed from the game structure independently of any
  policy's reach: 12 for Kuhn and 936 for Leduc, checked as oracle values for this
  representation.
* **Oracle comparison** (`oracle.rs`): vector-form expected value, best response, and
  one CFR iteration agree with `history_oracle.rs` within 1e-12 on: uniform weights,
  unequal weights, rescaled weights (results invariant), a sparse range, a blocked
  board, all-conflicting ranges (construction fails with `EmptyGame`), fold-only and
  tie-only terminals.
* **DCFR trace** (`kuhn.rs` or a unit test in `postflop`): the three-iteration
  averaging example yields 1/14, and a hand-computed signed-regret sequence with a
  negative regret decays by 1/2 per iteration under β=0 and is never floored.
* **Kuhn** (`kuhn.rs`): for each variant, solve to a **fixed budget recorded in
  `tests/fixtures/kuhn.toml` after step 8b**, then assert `nash_conv < 1e-3`, and for
  DCFR and CFR+ additionally `nash_conv < 1e-5` and
  `|expected_value(player 0) + 1/18| < 1e-4`. Structural equilibrium check on DCFR's
  average strategy, tolerance 0.02: player 2 bets K and calls with K always, folds J to
  a bet, bets J after a check 1/3, calls Q to a bet 1/3, never bets Q after a check;
  player 1 never bets Q and bets K three times as often as J. Curve: vanilla and CFR+
  `nash_conv` at every captured OpenSpiel checkpoint satisfy
  `|ours − ref| <= atol + rtol · |ref|` with `rtol = 1e-6` and `atol = 1e-9`, the
  absolute allowance justified in `tests/README.md` from the f64 scale of the values.
* **Leduc** (`leduc.rs`): DCFR and CFR+ to fixed budgets recorded after step 8b, then
  `nash_conv < 1e-4` for DCFR and `< 1e-3` for CFR+. Value check: with `r_ref` the
  captured residual of OpenSpiel's final CFR+ profile and `r_ours` our final residual,
  assert `|expected_value(player 0) − ref_value| <= r_ours + r_ref + 1e-9`. The
  literature value near −0.0856 is a sanity figure in the README, not an assertion.
  Vanilla and CFR+ curves match OpenSpiel at every checkpoint with the same
  `atol + rtol` rule.
* **Convergence record** (both games, replaces the monotonicity gate, Astra P05):
  `nash_conv` is recorded at checkpoints 1, 2, 5, 10, 20, 50, 100, ... and written to
  the test output; the gate is the fixed-budget accuracy above. Once reference runs
  exist, a regression envelope (each checkpoint within a recorded factor of the
  reference run) is added as a separate test. No checkpoint is dropped to make a test
  pass.
* **Failure paths** (`failure_paths.rs`): the NaN game makes `solve` return
  `SolveError::NonFinite` naming the iteration, never a strategy; an iteration cap
  yields `StopReason::IterationCap` and the log line says so; a config with a missing,
  NaN, negative, or zero field fails to load with the field named; an all-conflicting
  range fails with `EmptyGame`; a nonpositive pot is rejected; the payoff sum-to-zero
  test.
* **Most likely failure:** the curve comparison. If a checkpoint disagrees, the
  semantics differ (update order, averaging weight, uniform fallback, chance
  normalisation, or mask placement). Diagnose with `info_label` against OpenSpiel's
  `information_state_string`, align our implementation, and only if the difference is
  deliberate record it in `tests/README.md` with the experiment showing both curves
  reach the same limit. No tolerance is loosened without that record.

## Risks and edge cases

* **Game trait shape.** Too hold'em-specific: the vector form assumes two players with
  fixed private-state vectors and product weights; that is the postflop crate's contract,
  and the multiway preflop solver (Phase 11) uses its own MCCFR. It does not cover
  correlated ranges, bunching, or multiway; those need their own contracts. Too generic
  to be fast: everything hot is a flat slice per node; no trait objects inside the
  traversal loop except the `&dyn Game` boundary, which Phase 3 can make generic if
  profiling says so.
* **Normalisation and masks.** Product-of-marginals weights do not sum to 1 over
  compatible deals; the root normaliser, the chance masks, and where own versus opponent
  reach is applied are where plausible but wrong values come from. The uniform-policy
  constants and the history oracle catch this.
* **Signed regrets.** Flooring stored regrets in DCFR turns it into a different
  algorithm. The trace test pins the storage rule.
* **Kuhn tolerance and float precision.** `1e-4` on the value requires `nash_conv` well
  below `1e-4`; the DCFR and CFR+ budgets are set from the captured curves. Vanilla
  CFR's `1/√T` rate on Kuhn is the slow one; its budget is measured and recorded, and
  its gate is `1e-3` only. The best response's own float floor is measured by the
  oracle comparison, not asserted from theory.
* **Reference residuals.** A comparison against a converged-but-not-exact reference
  needs both residuals; the Leduc value test includes them. Nothing about the 10,000
  iteration reference or the budgets is claimed until step 8b has run.
* **Toolchain pinning.** An exact `channel` in `rust-toolchain.toml` plus `rustup show`
  in CI keeps both runners on the same compiler; `--locked` keeps dependencies fixed.
* **Windows in CI.** Long paths (`core.longpaths`), CRLF (`.gitattributes` forces LF so
  prettier and rustfmt agree), PowerShell versus bash (`shell: bash` everywhere), pnpm
  store on the Windows runner, the native `tauri build --no-bundle` needing WebView2 on
  the runner image (present on `windows-latest`).
* **Worktree flow.** The executor's worktree has no `node_modules` or `target`; it
  installs its own. `git init` must precede any executor.
* **Two assistants, one folder.** Ownership is stated in "Approach" and mirrored in the
  root README and `docs/ROADMAP.md`. Executors touch only the files their step names.
* **Secrets.** None expected. The Tauri scaffold must not gain network permissions; the
  CSP and capability checks in step 3 are how that is verified.

## Open questions

None. The defaults that are the planner's rather than Caleb's are named in the steps:
excluding `app/src-tauri` from the Rust workspace (with native Windows checks instead),
ignoring `CLAUDE-UPDATE.md` alongside `ASTRA-UPDATE.md`, CFR+ with linear averaging,
the checkpoint schedule, and `app/` passing to Astra after scaffold acceptance.

## Decisions

* 2026-09-05: Stack. Rust solver and engine, Tauri 2, React with TypeScript, PixiJS
  table, SQLite, Rive later; work hand in hand with Astra (plan review before execution,
  phase review after). Source: `docs/research/README.md`.
* 2026-09-05: Roles and ownership. Claude leads development; Astra owns design, UI
  implementation, `app/**` after scaffold acceptance, independent review, and security
  review; Caleb owns scope. Source: `ASTRA.md`, `CLAUDE-UPDATE.md`,
  `docs/reviews/2026-09-05-astra-phase-0-1-plan-findings.md`.
* 2026-09-05: Format order. 6-max cash at 100bb first; stack depth is a setting from
  10bb to 200bb, never a constant. Source: `docs/research/README.md`.
* 2026-09-05: Machine and audience. 16 GB development machine; shipped solves run on
  16 GB; public launch later; licences stay MIT and Apache clean, so nothing from
  b-inary/postflop-solver (AGPL) is copied or linked. Source: `docs/PRODUCT.md`,
  `docs/research/open-source-libraries.md`.
* 2026-09-05: Solver algorithm. DCFR(1.5, 0, 2) with alternating updates, signed
  regret storage, and in-place discounting of the whole accumulators is the default;
  vanilla CFR and CFR+ are kept as validated alternatives. Source:
  `docs/research/solver-algorithms.md` (corrected per R01).
* 2026-09-05: Go-ahead. Caleb: proceed once Astra's review is filed. Astra's findings
  are addressed in this revision; step 1 and the phase 0 executor proceed while Astra
  re-reviews, and the scaffold is not transferred until Astra accepts it.

## Sources for the reference numbers and rules

* OpenSpiel `exploitability_test.py` (11/12 and 4.747222222222222): https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/python/algorithms/exploitability_test.py
* OpenSpiel `leduc_poker.cc` and `.h` (rules): https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/games/leduc_poker/leduc_poker.cc
* OpenSpiel `cfr.py` (alternating-update semantics): https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/python/algorithms/cfr.py
* OpenSpiel Windows install and PyPI wheels: https://openspiel.readthedocs.io/en/latest/windows.html and https://pypi.org/project/open-spiel/
* Tauri CSP and capabilities: https://v2.tauri.app/security/csp/ and https://v2.tauri.app/security/capabilities/
* Cargo virtual workspaces and resolver: https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace
* GitHub Actions secure use: https://docs.github.com/en/actions/reference/security/secure-use
* Astra's independent arithmetic: `docs/astra/2026-09-05-plan-and-research-review/checks.py`
* Leduc first-player value near -0.0856 (sanity figure only): https://arxiv.org/pdf/2601.17131 and https://arxiv.org/pdf/1711.00832
