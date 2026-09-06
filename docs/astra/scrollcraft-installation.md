# Scrollcraft installation

Installed 2026-09-05 at Caleb's request. The author is **Nate Herk**, and the skill's current name is `scroll-craft`. The upstream plugin manifest identifies version **0.3.0**. [Official repository](https://github.com/nateherkai/scroll-craft)

## Source and locations

The installed revision is `0b816225945e45380397d6a0487efa3c98916858`. The complete source folder is `plugins/nateherk-design/skills/scroll-craft/`. The author documents project-local Codex installation and direct skill-file use. [Pinned source](https://github.com/nateherkai/scroll-craft/tree/0b816225945e45380397d6a0487efa3c98916858/plugins/nateherk-design/skills/scroll-craft)

- Claude: [.claude/skills/scroll-craft/SKILL.md](../../.claude/skills/scroll-craft/SKILL.md).
- Codex: [.agents/skills/scroll-craft/SKILL.md](../../.agents/skills/scroll-craft/SKILL.md).
- Integrity record: [scrollcraft-source.json](scrollcraft-source.json).

Each copy contains all 24 upstream skill files, plus the repository's original MIT `LICENSE`. The skill, references, templates, engine, and scripts are unchanged. Astra compared every file against its Git blob hash in the pinned upstream tree and checked the two copies for byte equality. The source record also contains SHA-256 hashes.

## Using it here

Ask for `Scrollcraft` or `scroll-craft`, or read the linked `SKILL.md` directly. Use its workflow for scroll experiences and its references when their design guidance applies. Keep the poker table's controls and dense study tools suited to their tasks.

Caleb has delegated creative direction to Astra. The installed skill supports writing a brief under explicit creative delegation; another interview is unnecessary when the requirements are already known.

## Source review before running helpers

Astra read the installed `SKILL.md` and inspected the preview server and API helper. A read-only research helper also inspected tool behavior. These are source-review findings, not a completed runtime security audit.

**Preview server:** `scripts/serve.mjs` binds without a loopback host restriction. Its path check uses a string prefix, which fails to distinguish the intended root from a sibling with the same prefix. Dotfiles are not filtered, and malformed URI decoding is unhandled. Use a checked project dev server bound to `127.0.0.1`, serving only the intended public build directory, with path containment and error handling. Do not run this bundled server unchanged. [Inspected source](https://github.com/nateherkai/scroll-craft/blob/0b816225945e45380397d6a0487efa3c98916858/plugins/nateherk-design/skills/scroll-craft/scripts/serve.mjs)

**Optional asset API:** `scripts/kie.mjs` can read a key from an ancestor `.env`. It sends generation jobs to `api.kie.ai`; reference-image uploads send image data and the bearer key to `kieai.redpandaai.co`. Treat using this helper as an external service operation with its own authorization and cost context. Installation did not invoke it or inspect local secrets. [Inspected source](https://github.com/nateherkai/scroll-craft/blob/0b816225945e45380397d6a0487efa3c98916858/plugins/nateherk-design/skills/scroll-craft/scripts/kie.mjs)

Before later use, inspect the current helper, resolved workspace, and command arguments. `doctor.mjs --probe` calls the external balance API. The workspace helper can select an output directory through environment or ancestor configuration. Apply the skill's pointer-lock safeguards to every browser verification context.

## Runtime readiness

Installation and integrity verification passed. No downloaded script was executed, no preview server was started, and no paid API was called.

Node, ffmpeg, and ffprobe were not found on this session's PATH; Node was also absent from the three standard installation paths checked. This is not a claim that the machine has no copies elsewhere. Full rendering readiness, Playwright, and browser/media verification remain untested. The upstream workflow needs Node 18+, a full ffmpeg build, Playwright, and a supported browser. [Upstream requirements](https://github.com/nateherkai/scroll-craft#requirements)

When a design build requires those tools, resolve and verify the missing prerequisites as part of that build. Installation of the skill itself is complete.

## Updating

Review the new upstream revision, update both complete copies together, retain the license, and regenerate the integrity record. Recheck the helper findings before changing these usage notes. Keep project-specific adaptations outside the upstream package.
