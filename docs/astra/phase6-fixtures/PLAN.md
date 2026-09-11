---
type: plan
status: in-progress
date: 2026-09-10
---

# Pinned PHH fixture corpus

Base `c2eddfe`, isolated branch `engine/astra-phase6-fixtures`. This prepares the
eleven licensed data files named in phase 6 step 5. It implements no game rule,
parser, replay engine or equity algorithm. Replay acceptance retains its dependencies.

Before copying, read the pinned repository's LICENSE and record its exact
revision, license hash and attribution in `crates/engine/tests/fixtures/SOURCES.md`.
The previously inspected license is MIT, copyright 2024-2025 Universal, Open,
Free, and Transparent Computer Poker Research Group; verify the downloaded bytes.
Preserve the full license beside the fixtures and each data file byte for byte.

Fetch only the eleven exact URLs in
`docs/reviews/2026-09-10-step6-review/phh-inventory.json`. Refuse checksum mismatch,
unexpected file count or any destination outside the fixture directory. Use
bounded downloads. Do not execute or copy upstream Python/engine code.

Verify file hashes, NT variant, known cards, integer starting/finishing stacks,
seat count, action counts and show actions against the saved inventory. Record
what this proves and the missing per-action/legal-action/finishing-stack replay.
Provide a small repeatable local inventory command if useful; it must inspect
only data and not import a third-party game engine.

The executor owns fixture files, SOURCES/license, its inventory verifier and this
note. No engine source, dependencies, lockfile, shared plans, push, merge or
nested agents. Astra independently inspects the source attribution and inventory
before integration. Run the prose checker on authored notes, preserving upstream
data and license text unchanged.
