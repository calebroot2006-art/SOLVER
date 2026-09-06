---
project: gto-solver-app
type: plan
status: complete
date: 2026-09-05
---

# Astra instructions and Scrollcraft setup

## Progress

Astra wrote the permanent operating file and the `AGENTS.md` entry point. A handoff found in `ASTRA.md` was preserved before replacement. One read-only helper located and researched Scrollcraft; Astra inspected the installed skill and selected scripts. Both complete copies match the pinned upstream tree, and each includes the original MIT license. Source notes and a Fable handoff are written. Final prose and local-link verification are recorded below.

## Task

Expand Astra's operating instructions around Caleb's delegated design and review authority, keeping Astra responsible for most of the work. Find and install Nate Herk's Scrollcraft skill and confirm access to `.claude`.

## Approach

Astra writes the instructions and verifies the installation. The helper only researches the source. Preserve existing handoff content before replacing it with the requested permanent file. Keep Fable's instructions and agent definitions intact.

## Steps

1. Read `CLAUDE.md`, `ASTRA.md`, the agent definitions, product brief, and incoming handoff.
2. Draft `ASTRA.md` and add `AGENTS.md` to point future Codex sessions to it.
3. Install the complete pinned `scroll-craft` skill in `.claude/skills/` and `.agents/skills/`, with the upstream license and source record.
4. Verify file integrity, references, and authored prose. Write `CLAUDE-UPDATE.md` with the result.

## Tests

Run `python .claude/skills/anti-ai-slop-writing/slopcheck.py ASTRA.md AGENTS.md docs/astra/PLAN.md docs/astra/scrollcraft-installation.md CLAUDE-UPDATE.md --internal`. Compare both skill copies against the pinned upstream tree and check local document links. App tests do not apply to this setup task.

Results: five authored documents passed with 0 banned and 0 review findings. All 45 checked local document and skill links resolved. Both installed copies contain the 24 upstream files plus the license, and every SHA-256 matches the source record. The permanent instructions and preserved incoming note were checked; the staging draft was removed.

## Risks and edge cases

- Another session is writing shared files. Preserve incoming content and check the observed file before replacement.
- The skill includes optional paid API helpers and a preview server with source-review concerns. Installing files does not run those helpers.
- Node and media-tool readiness must be distinguished from successful skill installation.

## Open questions

None for this setup. Product and stack decisions remain in the existing product brief and roadmap.

## Decisions

- 2026-09-05: Caleb gave Astra control of design, code review, and security, with Astra doing most work and helpers doing bounded research or straightforward tasks.
- 2026-09-05: Caleb requested Scrollcraft and confirmed the polished poker room with a friendly coach direction earlier in this session.
