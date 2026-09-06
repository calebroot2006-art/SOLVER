---
type: research
status: draft
date: 2026-09-05
---

# App stack and coach integration

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

What stack should the app use end to end (desktop shell, table rendering, UI kit, local
data, coach backend, avatar and voice, build and CI), and how should the natural-language
coach be built, given a Windows-first, compute-heavy Rust solver?

## Answer

The overview's recommendation holds: Rust core, Tauri 2 shell, React and TypeScript UI.
Run the solver in-process as async Tauri commands on a Rayon thread pool, not as a
sidecar. Render the animated table on canvas with PixiJS and keep the surrounding chrome
(range grids, EV tables, coach panel) in React. Local data goes in SQLite through the
official Tauri SQL plugin; large solved-spot binaries stay as files. The coach calls the
Claude API (Sonnet 5 by default) with the solver's structured output as its only source
of numbers, constrained by structured outputs and checked against the solver data before
display, with prompt caching to keep cost down and a templated offline fallback that
needs no model. The avatar is built in Rive; voice defaults to the browser's built-in
speech synthesis with ElevenLabs as an optional paid upgrade.

## Findings

### Desktop shell

* Tauri v2.10.1 was current as of March 2026. One narrow Windows bug: WebView2 fails to
  start under the new "Administrator Protection" elevation mode because of how Tauri
  computes the WebView2 user-data directory. Matters only if run elevated.
  Source: https://github.com/tauri-apps/tauri/issues/13926
* Progress streaming: Tauri's channels are for fast ordered data, events for smaller
  updates. For a long solve, emit progress events (iteration, exploitability, elapsed)
  from the async Rust command and listen on the frontend. Matches CLAUDE.md's logging
  rule. Sources: https://v2.tauri.app/develop/calling-frontend/ and https://tauritutorials.com/blog/tauri-events-basics
* Tauri's process model splits WebView from the Rust backend precisely so expensive work
  does not freeze the UI. Run the solver as an async command on a blocking pool.
  Source: https://v1.tauri.app/v1/references/architecture/process-model/
* Sidecars suit short-lived external binaries; a long-lived, tightly coupled numerical
  core belongs in-process. Inferred from Tauri's sidecar framing.
  Source: https://v2.tauri.app/learn/sidecar-nodejs/

### Table rendering

* DOM wins for a handful of elements; anything with hundreds of animated elements
  belongs on canvas. PixiJS is WebGL-accelerated; using it directly beats React wrappers
  for graphics-heavy work. Secondary sources.
  Sources: https://blog.logrocket.com/getting-started-pixijs-react-create-canvas/ and
  https://dev.to/bkhebert/pixi-js-vs-pixi-react-3ggf
* No disclosure found of what GTO Wizard or PokerStars use for their tables. Unverified.
* GSAP is fully free including former paid plugins since Webflow's April 2025 acquisition
  and is the stronger tool for many-element timed sequences (deals, chip movement).
  Motion (formerly Framer Motion) fits React-state-driven UI transitions.
  Source: https://www.pkgpulse.com/compare/framer-motion-vs-gsap

### Frontend framework and UI kit

* Mantine is repeatedly named the best balance for data-heavy dashboards with built-in
  tables; shadcn/ui trades built-in density for full code ownership and Radix
  accessibility. Secondary sources. Sources: https://designrevision.com/compare/mantine-vs-shadcn
  and https://dualite.dev/blogs/best-ui-component-libraries
* Charts: Nivo and ECharts suit denser data and canvas rendering; Recharts is simplest
  for small charts. Cited bundle sizes (unverified): Recharts ~150 kB, Tremor ~200 kB,
  Nivo 500 kB+. Sources: https://querio.ai/articles/top-react-chart-libraries-data-visualization
  and https://www.pkgpulse.com/guides/recharts-v3-vs-tremor-vs-nivo-react-charting-2026

### Local data

* tauri-plugin-sql is official, built on sqlx, supports SQLite via
  `Database.load('sqlite:file.db')` with migrations. Rust 1.77.2 or later.
  Sources: https://v2.tauri.app/reference/javascript/sql/ and https://crates.io/crates/tauri-plugin-sql
* Hand histories, progress, and spot metadata go in SQLite; solved-spot binaries stay as
  gitignored generated files.

### The coach's brain

All Claude facts below were fetched from Anthropic's platform docs on 2026-09-05.

* Model lineup and pricing per million tokens: Claude Fable 5.1 at $10 in and $50 out
  (adaptive thinking, long-horizon reasoning), Claude Opus 5 at $5 and $25, Claude
  Sonnet 5 at $2 and $10 (best speed and intelligence balance), Claude Haiku 4.5 at $1
  and $5. All support text and image input, tool use, and multilingual output. Sonnet 5
  and above have 1M-token context at no premium.
  Sources: https://platform.claude.com/docs/en/models/overview and
  https://platform.claude.com/docs/en/about-claude/pricing
* Sonnet 5 is the right default: explanations are short and latency-sensitive, and the
  solver already did the reasoning. The model narrates, it does not analyse.
* Prompt caching: cache writes cost 1.25x base input (5-minute) or 2x (1-hour); cache
  reads cost 0.1x base input on all models except Fable 5.1, which gets 0.025x. A stable
  system prompt (persona, style rules, output schema) plus a per-spot solved-strategy
  block is exactly what caching is for. Source: the pricing page above.
* Structured outputs constrain the response to a JSON schema and make tool arguments
  strict. At the time of the researcher's fetch this was a beta with a dated header;
  Astra's re-check on 2026-09-05 found the current docs use `output_config.format` with
  no beta header. Re-verify the request shape against the pinned SDK and model before
  implementing. Source: https://platform.claude.com/docs/en/build-with-claude/structured-outputs
* **Why matching numbers is not enough (Astra R03).** A response can cite the right
  action, EV, and frequency and still be false. Example: bet and check both have EV
  0.4bb and the solver mixes 62/38; the response "Betting is mandatory; checking always
  loses" with `cited_action: bet, cited_ev: 0.4, cited_frequency: 0.62` passes a
  number-only check. Schema constrains shape; it does not prove a poker claim.
* **The v1 contract that follows.** Grading and facts are computed in Rust and are
  authoritative. Each fact has a stable ID tied to the hand, decision, actor, action and
  size, solve revision, units, and coverage. Only information available at the original
  decision goes in; later cards and runouts never enter decision-quality reasoning. The
  model receives the fact set and returns an ordered list of approved fact IDs and
  explanation-template IDs, optionally with connective phrasing; every substantive
  poker sentence the user reads is rendered from a validated fact or template, with
  numbers and comparisons filled from trusted data. EV loss and the grade come from
  code. Any freer narration is labelled as such and needs a separately reviewed
  claim-validation design; it is not advertised as guaranteed true.
* Adversarial fixtures the coach must pass before phase 9 ships: equal-EV mixes, swapped
  actions, percent versus fraction and bb versus chip confusion, stale or mismatched
  solves, unsupported multiway spots, fabricated blockers, future-card leakage, and
  prompt instructions embedded in untrusted hand text. Every rejected, refused,
  truncated, or unavailable response falls back to a deterministic template that has
  its own applicability test. Templates can be wrong too when applied to the wrong
  situation, so they are tested, not assumed safe.
* Boundaries (Astra R04): credentials and provider calls live in the Rust layer, never in
  the frontend or a distributed executable; returned text is rendered as text with no
  HTML, links, tools, or filesystem access; requests are bounded in size, time, retries,
  concurrency, and cost; request and cache keys include the decision, solve revision,
  prompt and schema revision, and model; stale completions are discarded on context
  change or cancel; what hand data leaves the machine is recorded. The public
  proxy-versus-user-key choice stays Caleb's launch decision and the phase 9 client
  must keep both possible.
* Offline fallback: llama.cpp runs quantised 3 to 7B models without a GPU, but no source
  benchmarks a small local model for grounded poker explanation. The offline path is the
  same validated-template renderer used above, with no model at all.
  Source: https://www.sandgarden.com/learn/llama-cpp
* GTO Wizard lists "translating solver's output into human language" as a future goal.
  No competitor has a shipped reference implementation to copy.
  Source: https://blog.gtowizard.com/gto-wizard-ai-explained/

### Coach character

* Rive renders via WebGL2 at about 60 fps on mobile where Lottie manages about 17 fps
  on the same hardware; Rive's state machine supports blended interactive states suited
  to real-time viseme blending. There is no off-the-shelf audio-to-lip-sync for Rive,
  Spine, or Lottie; visemes are driven from audio yourself. Duolingo ships Rive-driven
  lip-synced characters. Sources: https://hooman.com/blogs/boost-user-engagement-rive-lottie
  and https://templates.mascot.bot/lip-sync-api-2d-characters
* Lottie stays for lightweight non-interactive flourishes only.
* Voice: browser SpeechSynthesis is free, offline, and licence-free. ElevenLabs: free
  tier has no commercial rights; Starter at $6 a month (about 30 minutes) is the minimum
  commercial tier; Creator at $22 adds voice cloning. Source (secondary):
  https://bigvu.tv/blog/elevenlabs-pricing-2026-plans-credits-commercial-rights-api-costs/

### Build, test, packaging on Windows

* tauri-action builds Windows, macOS, and Linux binaries in one GitHub workflow and can
  publish a release. Source: https://v2.tauri.app/distribute/pipelines/github/
* Windows signing: Tauri v2 supports Azure Trusted Signing directly; its older OV
  guide applies only to certificates issued before June 2023. Microsoft removed EV
  code-signing OIDs from its Trusted Root Program in August 2024, so EV no longer buys
  automatic SmartScreen reputation; standard or Azure Trusted Signing is likely enough
  for a personal app. The EV change is from a community Q&A thread, not an official
  Microsoft page. Sources: https://v2.tauri.app/distribute/sign/windows/ and
  https://learn.microsoft.com/en-us/answers/questions/1850140/new-ev-code-signing-certificate-stored-in-azure-ke

## Unverified

* The state of WebView2 rendering parity in 2026 beyond the one elevation bug.
* Table rendering technology of any commercial trainer.
* Whether a small local model gives trustworthy poker explanations.
* Microsoft's current EV versus OV SmartScreen policy from an official source.
* Bundle sizes and performance of the UI and chart libraries against our real data shapes.

## Also worth knowing

* The coach renders substantive advice from applicable validated facts and templates.
  A schema and a number check alone cannot establish that an explanation is true.
* Re-check structured-output support and request shape against the pinned SDK/model
  before implementation; the current documented shape is `output_config.format`.

## What this means for our plan

* Stack, approved by Caleb on 2026-09-05: Rust workspace, Tauri 2, React with
  TypeScript, PixiJS for the table, Mantine or shadcn/ui at Astra's choice, GSAP for
  table motion, SQLite via the Tauri plugin. Coach is text only in version 1; the Rive
  avatar and any voice are later versions.
* The coach is a separate module with a strict contract: input is the Rust-built fact set
  for one decision (available-at-the-time information only); output is a
  schema-validated selection of fact and template IDs rendered from trusted data. Grade
  and EV loss come from code. Tiers: validated templates (offline, always available),
  Sonnet 5 composing from approved facts (default), and a richer model for
  post-session reviews if wanted, under the same contract.
* Budget note for Caleb: at Sonnet 5 prices with caching, a few hundred explanations a
  day is cents, not dollars, as a forecast. A forecast is not a spending limit: the
  client enforces per-request and per-day bounds, and the real numbers come from
  measured token counts and cache hit rates once the prompt exists.
