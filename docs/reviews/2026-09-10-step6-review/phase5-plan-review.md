# Phase 5 plan review: needs changes; independent work can proceed after amendment

Astra reviewed `docs/phase-5/PLAN.md` at `2f81339` on 2026-09-10, against the
accepted roadmap decisions and the incoming `e338d7a` solver API. The read-only
helper identified provenance, allocation, and coverage gaps; Astra inspected the
plan and API and assessed them below. No phase 5 implementation exists yet.

**P5-1, high: the format can mislabel an unfinished policy as measured.** Header
fields at plan lines 69–73 require scalar accuracy values; line 187 promises to
store cancelled solves. Step 6 can return no measurement, or a measurement from
an earlier iteration. `Spot::capture(strategy, report, inputs)` also accepts three
separate objects without a stated check that they describe the same solve.

Store the policy iteration, optional measurement with its own iteration, stop
reason, and bound game/attempt provenance. Capture must reject mismatched inputs
or reports. If these cannot be established through the public API, name a bounded
solver-contract request to Fable; the claim that no accessor is missing is too
strong. Test cancellation before any measurement, after an unmeasured iteration,
and after measurement, plus a report or input from another solve. Index absent
accuracy as absent; do not substitute zero or put it among target-reached spots.

The source policy's residual certifies that source policy. Quantizing and omitting
later streets produces a different, partial payload. Label source-solve accuracy
and quantization error separately. Do not imply a measured exploitability for the
decoded cut unless its complete continuation policy is available and measured.
The proposed capture test equating header accuracy with a fresh measurement of
the strategy needs this distinction and cannot apply unchanged to stale reports.

**P5-2, high: decoder and capture memory are not bounded by the solver limit.**
Binary caps at lines 102–106 bound individual counts, but four million nodes times
1326 combos, action vectors, strings, and allocation overhead can exceed practical
memory before `Spot::validate`. JSON at lines 110–113 has no equivalent byte,
collection, string, or nesting limits. `deny_unknown_fields` does not provide them.
The plan's `Spot { nodes: Vec<NodeRecord> }` is another retained copy outside the
solver budget, even if the writer streams the encoded bytes.

Specify checked aggregate decoded-allocation and capture budgets, along with
per-field limits, before allocating. Apply comparable limits to binary, the JSON
header, JSON import, index rebuild, and CLI input. Test valid small files declaring
large counts, deeply nested/oversized JSON, arithmetic overflow, and retained
snapshot plus capture overlap. Define whether capture streams records to a writer
or reserves a bounded in-memory `Spot`; do not promise both a complete `Vec` and no
second copy. These are design requirements, not claims of an existing importer
vulnerability.

**P5-3, medium: accepted coverage measurement is missing from step 8.**
`docs/ROADMAP.md:383` records Caleb's acceptance of representative-session coverage
as a phase 5 step 8 requirement. Plan lines 151–157 only gate generation, residual,
and file size; line 181 still calls coverage a proposal. Add decisions graded
exactly, approximately, or not at all, by cause, plus solve wait and bot fallback
counts. Name the sample scenario and how a deterministic session trace supplies
the inputs before the live app exists. Passing 25 exports is not evidence that
players receive useful advice. The coverage target remains Caleb's decision.

**P5-4, medium: quantization and lookup need precise boundary rules.** The plan
names widths but leaves behavior undefined at zero EV scale, a scale rounded to
zero in f32, signed limits, and a positive reach rounded to zero. Specify error
against the value decoded with the stored scale, not an unrounded temporary scale.
Use a deterministic probability quantizer whose integer row sums exactly to
65535, with stable action-order tie breaking and the stated per-action error.
Test all-zero EVs, mixed missing/present values, endpoint probabilities, many-action
rows, tiny reaches, and blocked combos on a later runout. Preserve "unavailable"
separately from zero and state how lossy reach affects applicability.

History, board/runout, player, combo, action labels, and units must identify a row
unambiguously. Validate duplicate histories/IDs/combos, malformed suit permutations,
and mappings that refer outside coverage. A start-street cut needs an explicit
boundary response when lookup reaches an omitted street. Original range strings
can contain board-blocked hands; distinguish that legal input from an illegal
stored live combo. Do not reject ordinary original ranges as conflicting data.

**P5-5, medium: generation and index recovery are underspecified.** A failed write
must not leave a file that a rebuild indexes as complete, and `--continue-on-error`
promises a failure record that the proposed successful-spot table cannot clearly
represent. Specify temporary output, validation, final publication, and index
transaction ordering. Test interruption before/after file publication and index
commit, duplicate IDs, a corrupt file during rebuild, and a missing file. Store
failure details separately from solved-spot entries. Bound and validate paths on
every index read/write operation; the `verify` command alone cannot protect normal
lookup or rebuild.

## Revised execution dependencies

| Work | What can unblock it |
|---|---|
| Types/schema, codecs, range charts and synthetic tests | Amend P5-1 through P5-5; use existing pinned dependencies where applicable |
| Capture adapter and small river/turn tests | Fix measurement binding contract; target the accepted API revision |
| SQLite index | Caleb's `rusqlite` approval and explicit lockfile/dependency review |
| CLI flow with a small test scenario | Capture, codec and index contracts; handle absent/stale progress measurements |
| Production gate library | Caleb's scenario, starting street, coverage/depth, host, coverage target, and accepted solves for that street |

`serde_json` already appears in the workspace dependency set; amend question 4
to distinguish using an existing pinned crate from adding `rusqlite` and its native
SQLite build. Keep the stated file ownership. A phase 5 executor must request an
API addition through Fable rather than editing phase 4 while its branch owns it.

Caleb's product answers remain open. They need not block formats and synthetic
tests once the amendments are reviewed. The full plan and production-library gate
are not accepted as currently written.
