# Claude agent prompt efficiency review

Verified for the stated scope on 2026-09-07: four agent prompt edits and the
related `CLAUDE.md` workflow. Base revision: `b82ae27`. No solver, application,
CI, model assignment, effort setting, or tool permission changes.

The old workflow required a reader before most planning and reviews, even with
current facts already available. The reader allowed 2,000 words by default;
the executor requested pasted test output. The planner required existing-file
citations even for proposed files. These instructions caused avoidable work.

The new workflow skips unnecessary stages, reuses current evidence, bounds
research, and returns concise commands, results, metrics, and log locations.
Required numerical checks and the main session's independent review remain.

## Measured size

Counts use Python `len(text.split())` on each entire UTF-8 file, including
frontmatter, compared with `git show b82ae27:<path>`.

| File | Before words | After words |
|---|---:|---:|
| `.claude/agents/reader.md` | 270 | 252 |
| `.claude/agents/researcher.md` | 364 | 282 |
| `.claude/agents/planner.md` | 732 | 371 |
| `.claude/agents/executor.md` | 635 | 442 |
| Four agents combined | 2,001 | 1,347 |
| `CLAUDE.md` | 4,121 | 3,862 |

The four agent definitions contain 32.7% fewer words and 19.5% fewer characters.
Their descriptions total 108 words, down from 306. The reader's character count
increased to make evidence reuse and completion explicit. These measurements
describe prompt text, not tokenization or measured account-usage savings.

## Checks performed

- Parsed all four YAML frontmatters with PyYAML. Compared every field except
  `description` against `b82ae27`; names, models, effort, and tools match exactly.
  Confirmed that the agent directory contains exactly the four expected definitions.
- Ran the project prose checker in strict internal mode on all four prompts and
  this task's documents. No findings. Checked `CLAUDE.md` in internal mode: no
  banned findings. Its two long-sentence notices concern unchanged project and
  coordination prose, which this prompt edit preserves.
- Ran `git diff --check`. No whitespace errors. Inspected the final changed
  instructions against the original model split and solver correctness standards.
- Checked the scenarios below by reading the instructions. These are a static
  consistency review, not agent execution tests.

| Scenario | Required behavior after the edit |
|---|---|
| Requested few-line edit in one file | Main session can finish directly. |
| Bounded build with current facts | Main plans and briefs executor; no compulsory reader or planner. |
| Unknown module or large logs | Reader returns scoped, cited answers within the default 450-word report. |
| Source changed after a fact sheet | Recheck affected evidence, including dirty edits. |
| Missing existing interface | Return the precise missing fact; block dependent steps only. |
| Proposed new file | Name its path and purpose without requiring a nonexistent citation. |
| Research still unresolved at its limit | Return partial findings and the next check; do not guess. |
| Numerical gate fails | Preserve the threshold, return the failure, and require independent review. |
| CI push is not authorized | Return the pending gate; do not poll for a nonexistent run. |
| Passing suite with unchanged inputs | Reuse it unless a required independent check applies. |
| Report budget is too small | Preserve material contracts, failures, and blockers. |
| Parallel work shares a contract or fixture | Treat that as a dependency; retain worktree isolation. |

Reproduce prose and whitespace checks from the repository root:

```text
python .claude/skills/anti-ai-slop-writing/slopcheck.py .claude/agents docs/astra/agent-prompt-efficiency docs/reviews/2026-09-07-claude-agent-prompt-efficiency.md --internal --strict
python .claude/skills/anti-ai-slop-writing/slopcheck.py CLAUDE.md --internal
git diff --check b82ae27
```

## Runtime limits and source

The Claude CLI was not available on PATH or at the two checked user install paths,
so `claude agents` and live before/after task comparisons were not run. Actual
usage savings and behavior in Caleb's running Claude session remain unmeasured.
No model calls were made to simulate agent behavior.

Claude's documentation describes fresh subagent contexts, loading project
instructions, and resuming an agent for follow-up work. Those documented behaviors
support avoiding duplicate briefings and startup reads. The documentation also
describes watching edits to existing agent directories for subsequent delegation;
this review does not establish the installed session's version or reload state.
Source: [Claude Code custom subagents](https://code.claude.com/docs/en/sub-agents),
read 2026-09-07.
