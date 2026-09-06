---
project: gto-solver-app
type: plan
status: proposed
date: {{date}}
---

# PROJECT-NAME: the plan

Research: [[how-to-build-a-solver]]

Written with Caleb on {{date}}. The plan below is what was agreed; "Decisions" holds his
answers to the open questions, with the date each one was given. Nothing else is assumed.

## Progress (updated {{date}})

Read this first when picking the work up. It says what is done and verified, what is half
done, and what was learned that the plan below did not know. The executor updates it after
every step it finishes; the main session updates it after review.

**Where it stands:** nothing started.

## Task

One or two lines. What gets built, for whom, and what it replaces.

## Approach

A short paragraph on the shape of the solution and why this shape and not the obvious
alternative.

## Steps

Each step names the files it touches and what changes in them. Mark steps that do not
touch the same files as independent, so they can run in parallel.

1. **STEP-NAME** (`path/to/file.py`): what changes.
2. **STEP-NAME** (`path/to/other.py`, `path/to/README.md`): what changes. Independent of
   step 1.

## Tests

What gets tested and how. At minimum the happy path and the most likely failure path.
Name the command that runs them.

## Risks and edge cases

* Solver core: which known-solution tests, exploitability targets, and reference comparisons
  prove it correct.
* Long solves: what happens on out-of-memory or NaN, and how progress is logged so a stuck
  solve is visible.
* Paths on Windows versus a browser or CI runner, memory limits, thread counts.

## Open questions

Only real ones. Each gets Caleb's answer, recorded under Decisions, before an executor
starts. Leave the list empty if there are none.

## Decisions

* {{date}}: QUESTION. Answer: ANSWER.
