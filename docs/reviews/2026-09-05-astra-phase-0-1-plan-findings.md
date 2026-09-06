---
type: review-findings
from: Astra
to: Fable
date: 2026-09-05
verdict: needs changes
---

# Astra review: phase 0 and phase 1 plan

**Verdict: needs changes.** The proposed workspace and two-player public-tree design
are suitable starting points. Correct the scaffold acceptance criteria, numerical
contracts, and reference gates before using this plan as an executor brief.

## Snapshot and scope

Reviewed [Fable's handoff](2026-09-05-phase-0-1-plan.md), [PLAN.md](../../PLAN.md),
the standing instructions, and the current product and roadmap documents. No Git
repository or application exists. All five file hashes in this handoff match the
reviewed files. `PLAN.md` SHA-256:
`34d84ab664dfaf92bd166132f57885b6dff68510cb487eb0acf3f9d54f57437d`.
The complete [snapshot](../astra/2026-09-05-plan-and-research-review/snapshot.json)
records the other hashes. Locations below refer to that snapshot.

Astra read the plan and judged all findings. One read-only helper checked official
Tauri, Cargo, and GitHub Actions sources. Astra inspected the relevant security
documentation and numerical sources personally. No scaffold was generated, dependency
installed, workflow run, or solver built. This verdict concerns the plan; runtime
correctness and application security remain **not yet verified**.

## Answers to the five review points

| Point | Judgment |
|---|---|
| Scaffold and permissions | Keep step 3, with the explicit acceptance criteria in P01. |
| CI action pinning and permissions | The intended SHA pins and read-only token are sound. Complete P02 before writing the workflow. |
| Exploitability and units | Accept the zero-sum NashConv and half-NashConv definitions. Specify the percentage conversion and scope in P03. |
| `Game` trait | Accept vector form for the stated two-player scope, subject to P04's chance, payoff, and validation contract. |
| Uniform constants and OpenSpiel curves | The cited uniform constants are supported. Kuhn was independently reproduced. Runtime curves and budgets remain unverified; correct P05. |

## Findings

### P01. Medium: capability cleanup does not define the scaffold security boundary

**Location:** `PLAN.md:88`, step 3; `PLAN.md:290`, the no-network requirement.

**Evidence and impact:** Leaving the template page unchanged can retain its demo
bridge and opener behavior. The upstream template currently contains `csp: null`, an
opener permission/plugin, and a `greet` command. These are upstream observations, not
findings about a generated app in this repository. The eventual pinned generator must
be inspected separately. See the [config template](https://github.com/tauri-apps/create-tauri-app/blob/dev/templates/_base_/src-tauri/%25(v2)%25tauri.conf.json.lte),
[capability template](https://github.com/tauri-apps/create-tauri-app/blob/dev/templates/_base_/src-tauri/capabilities/%25(v2)%25default.json.lte),
and [Rust template](https://github.com/tauri-apps/create-tauri-app/blob/dev/templates/_base_/src-tauri/src/%25(v2)%25lib.rs.lte).

Tauri enables CSP only when configured. App-defined commands registered through
`invoke_handler` are available to app windows by default; narrowing plugin capabilities
alone does not restrict those commands. Removing an HTTP plugin also does not stop
ordinary web requests or Rust-side networking. [CSP documentation](https://v2.tauri.app/security/csp/),
[command permissions](https://v2.tauri.app/security/capabilities/).

**Correction:** Permit a static placeholder with unused demo commands, opener plugin,
permissions, and dependencies removed. Select local-window capabilities explicitly.
Define a restricted production CSP for bundled assets and required IPC; keep development
loopback/HMR allowances separate. If custom commands remain, enumerate and restrict
them through `AppManifest::commands` and explicit grants. Do not add remote capabilities.

**Closure:** The scaffold handoff inventories every retained command, plugin, permission,
and network allowance. Astra checks the generated files, the release app loads without
CSP errors, and a negative check confirms that an ungranted command and an external
web request cannot execute through the tested frontend path. This does not claim a
system-wide network sandbox or a completed app security audit.

### P02. Medium: the phase 0 gate can pass without building the desktop application

**Location:** `PLAN.md:67`, `PLAN.md:88`, `PLAN.md:98`, `PLAN.md:228`.

**Evidence and impact:** The Rust workspace excludes `app/src-tauri`, and its only
native validation is one local `cargo check`. CI omits both frontend formatting and the
production frontend build. Cargo check can miss failures discovered during a build.
[Cargo check](https://doc.rust-lang.org/cargo/commands/cargo-check.html).

**Correction:** Add `pnpm format:check` and a production frontend build on both runners.
Keep Linux CI limited to core Rust and frontend if desired, but add separate Windows
checks for Tauri formatting, clippy, and `tauri build --no-bundle`. Record a local Windows
launch smoke check. The exclusion must not leave the native shell unchecked until phase 7.
[Tauri build command](https://v2.tauri.app/reference/cli/#build).

Complete dependency reproducibility and workflow policy in the same plan revision:

- Use `resolver = "3"` in the virtual workspace; member edition 2024 does not select it.
  Commit root `Cargo.lock`, the excluded Tauri crate's own lockfile, and `pnpm-lock.yaml`.
  Use `--locked` for Cargo compilation and test commands. Record exact generator, Node,
  pnpm, and Rust versions. [Virtual workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace),
  [Cargo lockfiles](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).
- Pin every external `uses:` reference, including GitHub-owned actions, to a verified
  full commit SHA. Use ordinary `push` and `pull_request` checks with `contents: read`,
  no secrets, and no privileged execution of untrusted PR code or artifacts. Set checkout
  `persist-credentials: false`. Pinning does not establish that a dependency is safe.
  [GitHub secure use](https://docs.github.com/en/actions/reference/security/secure-use),
  [checkout configuration](https://github.com/actions/checkout).
- Specify the pnpm cache's lockfile path. Keep verification sequential after the scaffold
  and workspace changes are merged; their file edits can be prepared independently.

**Closure:** The revised commands cover the excluded desktop crate. After implementation,
clean-checkout CI passes without rewriting lockfiles, and the Windows launch is recorded.
No current dependency vulnerability or cache attack has been demonstrated in this review.

### P03. High: define percentage units and the zero-sum boundary before they become API fields

**Location:** `PLAN.md:82`, `PLAN.md:138`, `PLAN.md:164`, `PLAN.md:234`.

**Evidence and impact:** The plan defines `nash_conv` and `average` but leaves
`pct_of_pot` without a formula. Config uses percentage units, while toy stopping targets
use raw NashConv. Confusing the fields changes the stopping target by factors of two
or one hundred.

**Correction:** For this two-player, zero-sum, chip-utility phase, specify:

```text
br[i]       = max over player i's legal strategies of u_i(strategy_i, sigma_-i)
nash_conv   = br[0] + br[1]
average     = nash_conv / 2
pct_of_pot  = 100 * average / starting_pot
```

The first three numeric values are chips per hand; `pct_of_pot = 0.5` means half of
one percent of the fixed root pot. Raw NashConv targets must be named as such in toy
fixtures or explicitly converted. Displaying a percentage never changes the stored
utility units. [OpenSpiel exploitability tests](https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/python/algorithms/exploitability_test.py)
confirm its distinction between NashConv and half-NashConv.

Own calculation: uniform Kuhn gives `br = [1/2, 5/12]`, NashConv `11/12`, average
`11/24`, and a two-chip pot, hence `22.916666...%`. The
[independent check](../astra/2026-09-05-plan-and-research-review/checks.py) enumerates
every deterministic information-set policy against a uniform opponent using fractions.

Require terminal utilities to sum to zero for this API. For other payoffs, the general
deviation-gain sum is `sum_i(br[i] - u_i(sigma))`. Rake, tournament externalities, or
postflop payoff conventions that retain a constant starting-pot offset cannot silently
reuse `sum(br)`. Do not call tournament prize equity chips or normalize it by a chip pot.

**Closure:** Add exact conversion fixtures, raw-target conversion checks, and rejection
of nonpositive/nonfinite pots. State and test the utility baseline before any caller
consumes `Exploitability`. Later game classes need a separately reviewed metric contract.

### P04. Medium: the public-tree shape is acceptable, but its probability contract is incomplete

**Location:** `PLAN.md:114`, `PLAN.md:123`, `PLAN.md:143`, `PLAN.md:177`, `PLAN.md:184`.

**Evidence and impact:** A Leduc chance node enumerates six physical cards at `1/4`
each. Those values do not sum to one without the private-card masks. An implementer
could normalize them to `1/6`, apply chance twice, or omit the acting player's mask.
Aggregate uniform tests help, but do not specify behavior for unequal or empty ranges.

**Correction:** Keep a full public history and a player's observable private state as
the information-set key. Document whether returned vectors already contain prefix
chance reach, where the acting-player mask is applied, and where initial weights enter
root evaluation. Chance probability must contribute once to each complete deal's value.
Player reach for strategy averaging excludes opponent/chance probabilities; impossible
states are masked separately. The root normalizer must be finite and strictly positive.

For each compatible Leduc pair `(h0,h1)`, require
`sum_b q(b) * mask0(b,h0) * mask1(b,h1) = 1`. Our arithmetic check confirms `1/4`
over its four legal boards for all 30 private-card pairs. Add comparisons to a small
history-based oracle using unequal weights and sparse ranges, including rescaled weights,
blocked boards, all-conflicting ranges, and terminal folds/ties. Best response must not
observe the opponent's card when selecting an action.

Also make these small contract corrections:

- Reject NaN/infinite config values and define zero handling for `check_every`, iteration
  limits, and log intervals; preserve the documented `threads = 0` auto choice. Validate
  vector lengths, nonnegative weights, child indices, and nonempty legal action sets.
- Define the scalar used by `payoff` inside that crate or a dependency-neutral location.
  The only proposed `Real` currently lives in `postflop`, which depends on `payoff`.
  Do not introduce a reverse dependency just to obtain the alias.
- State Leduc's round-end rule precisely: two checks without a raise, or the remaining
  player's call after a raise. Matching contributions at the start of a round do not
  end that round. Pin explicit OpenSpiel parameters, including `suit_isomorphism=false`.
  [Leduc source](https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/games/leduc_poker/leduc_poker.cc).

**Closure:** The revised trait documentation and fixtures settle those cases before
phase 1 implementation. This accepts a two-player tree with compatible product weights;
it does not establish support for multiway play, arbitrary correlated ranges, bunching,
or every future tournament payoff. Those extensions need their own contracts.

### P05. High: the reference gates assume bounds and convergence behavior not yet established

**Location:** `PLAN.md:190`, `PLAN.md:230`, `PLAN.md:243`, `PLAN.md:249`, `PLAN.md:277`;
`docs/ROADMAP.md:97`.

**Evidence and impact:** The Leduc value gate compares against CFR+ at exactly 10,000
iterations with tolerance `2e-4`, without bounding that reference profile's residual.
The inequality cited in the plan does not establish this tolerance: a comparison needs
both residuals. Also, a convergence-rate theorem does not promise that measured DCFR
exploitability strictly falls at each doubling checkpoint.

**Correction:** Record the reference profile's NashConv with its EV. To justify the
stated EV tolerance by this bound, require candidate residual plus reference residual
to be at most that tolerance, with a numerical allowance. Alternatively capture a
tighter reference or use certified lower/upper game-value bounds from best responses.
Do not treat the approximate literature value as an independent certificate.

Replace unconditional monotonicity with a fixed-budget accuracy gate and recorded
convergence checkpoints. A measured regression envelope can be added after reference
runs establish it. Avoid choosing a new start checkpoint simply to hide a failure.
The [DCFR paper](https://arxiv.org/html/1809.04040) establishes a bound on the weighted
average profile, rather than strict step-by-step improvement.

Accept linear CFR+ averaging for comparison with OpenSpiel. Its source accumulates
player reach times the current action probability, weighted by iteration for CFR+.
Keep accumulation at information-set scope before regret matching, a frozen policy
during each player's traversal, and the documented alternating update order.
[OpenSpiel CFR implementation](https://raw.githubusercontent.com/google-deepmind/open_spiel/master/open_spiel/python/algorithms/cfr.py).

Record the exact reference version, game parameters, algorithm options, iteration
definition, and raw outputs. Express the curve check as `abs(error) <= atol + rtol *
abs(reference)`, with the absolute allowance justified at the f64 scale. Count legal
information sets independently of the learned policy's zero-reach states. Keep
`12`/`936` as oracle checks only for the specified game representation.

**Closure:** Before implementing against the fixtures, capture and inspect the reference
curves, pick documented finite budgets for every variant, and reconcile stop targets
with final assertions. Do not claim the proposed 20,000-iteration budget or Leduc value
tolerance has passed. The uniform Leduc constant is source-verified here; OpenSpiel and
the Leduc solver have not been run by Astra.

### P06. Medium: ownership instructions still authorize overlapping edits

**Location:** `PLAN.md:49`; `docs/ROADMAP.md:32`, `docs/ROADMAP.md:173`,
`docs/ROADMAP.md:266`, `docs/ROADMAP.md:275`.

**Evidence and impact:** `app/` is assigned to Astra, but a narrower sentence only protects
`app/src`, and the roadmap still sends executors into `app/src-tauri` or assigns them
the app shell. That can send two sessions into files Astra owns.

**Correction and closure:** Use the ownership decision below consistently in the plan,
roadmap, and bootstrap README. Fable owns the core implementation and contract design;
that does not grant unrestricted writes to bridge files inside Astra's folder.

## Scaffold choice and files reserved for Astra

**Keep step 3 with Fable's executor.** A bounded stock scaffold is appropriate delegated
work. I will review its files and checks before accepting the transfer, then personally
build the design and main UI. This review authorizes no scaffold execution today.

After scaffold acceptance, Astra owns all of `app/**`, including `src-tauri`, capabilities,
Tauri config, frontend package/config files, app tests, assets, and `app/README.md`.
Executors may repair their scaffold during review in their isolated branch; Astra will
not concurrently edit it. After transfer, an executor enters `app/` only for a bounded
file-specific task handed over by Astra in writing.

Astra also reserves `docs/astra/**`, `ASTRA.md`, `AGENTS.md`,
`.claude/skills/scroll-craft/**`, and `.agents/skills/scroll-craft/**`.
Fable's steps 1, 2, 4, 5 outside `app/`, and phase 1 files belong to Fable. Root pnpm
workspace files and lockfile remain tooling files maintained by Fable, with coordinated
edits when Astra adds app dependencies. `docs/reviews/` is shared by separate report
files: neither assistant overwrites the other's findings. Astra writes
`CLAUDE-UPDATE.md`; Fable writes `ASTRA-UPDATE.md`.

## Next action

Fable revises the documents to address P01 through P06 and the linked
[research findings](2026-09-05-astra-research-and-program-plan-findings.md), then supplies
new hashes for targeted re-review. Actual scaffold and numerical checks close their
implementation gates later. Git initialization and executors remain stopped until the
findings are addressed and Caleb says go.
