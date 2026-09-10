# Pending contract and plan review

Saved at Caleb's 2026-09-10 stop request. These are review leads and helper
findings, not final plan acceptance. Main documents were read at `2f81339`.

## 5d contract: Astra's unfinished assessment

Astra read `docs/phase-4/job-contract.md` and the incoming implementation.
The three disclosed changes are reasonable directions to assess: optional
measurements with their iteration, one charged browsing snapshot, and a pair
type for job identity. The draft has further gaps that need explicit resolution:

- The draft says `log_every_secs` can trigger measurement; implementation now
  deliberately measures on the iteration schedule only.
- A cached estimate cannot safely key only on the draft's `GameId`, because it
  excludes worker count while the estimate depends on resolved worker count.
- `GameId`, `SnapshotId`, event sequences, request acknowledgements, timing
  fields, snapshot replacement, and release events are not implemented by the
  pair type alone. Define which layer and milestone owns each obligation.
- Cancellation keeps the resumable solver and its lease alive until drop. Each
  game has its own budget, so the promised replacement-on-any-game ordering
  needs a driver-level policy. Existing tests admit multiple solvers when space
  permits; they do not establish a strict release-before-replacement rule.
- The one-snapshot table assumption is not an API limit of one live snapshot.
  The draft's invalidating replacement registry and concurrent-request checks
  remain to be implemented or explicitly deferred.
- In-iteration cancellation is not automatically a numerical no-op: abandoning
  partially updated accumulators needs rollback, disposal, or a last completed
  snapshot. Existing poisoning prevents reads after an error; it is not rollback.

Confirmed cancellation/identity defects are R5 and R6 in `findings.md`. Astra
has not yet decided pushed versus polled progress or compare-view snapshot
requirements. Do not present those as Caleb decisions or implemented contracts.

## Phase 5/6 helper findings, awaiting Astra's final assessment

The read-only helper inspected the plans, relevant research, product decisions,
and incoming report fields. It made no edits. Its findings follow.

1. **High, phase 5 measurement provenance.** `docs/phase-5/PLAN.md:69` requires
   scalar exploitability/BR/EV fields, while `:187` requires cancelled exports.
   Step 6 allows absent or stale measurements. The format needs optional
   measurement, measurement iteration, stop reason, and checked correspondence
   between policy, report, and inputs. Test cancellation before measurement,
   cancellation after an unmeasured iteration, and mismatched reports. Accuracy
   measured before quantization must not certify the decoded payload by itself.
2. **High, phase 5 allocation limits.** Binary counts/file caps at `:102` and
   JSON `deny_unknown_fields` at `:110` do not establish aggregate decoded-memory
   limits. Capture's `Vec<NodeRecord>` is separate from solver reservations
   (`:183`). Define bounded binary/JSON decoding, collection and nesting limits,
   checked total allocation accounting, and bounded capture/output. Test hostile
   JSON and interrupted writes/index rebuilds. This is a planning gap, not a
   demonstrated vulnerability in an implemented importer.
3. **Medium, missing accepted coverage gate.** Phase 5 step 8 (`:151`) omits the
   representative-session coverage measurement accepted in
   `docs/ROADMAP.md:383`. Add exact/approximate/ungraded coverage by cause, waits,
   and fallback counts. The coverage target remains Caleb's; the plan still
   calls the accepted measurement a proposal at `:181`.
4. **High, unsupported phase 6 fixture gate.** `docs/phase-6/PLAN.md:93` promises
   all 83 WSOP PPC hands, but its cited research
   `docs/research/bots-and-game-engine.md:107` says they span nine variants.
   Inventory and pin supported no-limit Hold'em fixtures, report the actual
   count, and add independent Hold'em edge cases. Explicitly reject unsupported
   variants rather than expanding this engine's scope to nine games.
5. **High, short-stack mandatory postings.** Phase 6 `:60` rejects antes larger
   than a stack; `:65` posts antes first. Existing tournament research at
   `docs/research/tournaments-and-icm.md:90` describes blind-first priority for a
   short big blind. Separate config from stack-dependent state validation;
   establish the chosen partial-posting rules and test short stacks below ante,
   below blind, and between blind and blind-plus-ante. Check pot eligibility and
   awards as well as conservation. The proposed `TableConfig` has no stacks to
   validate the asserted stack-dependent condition.
6. **Medium, unresolved phase 6 dependencies.** `:23` says only the oracle
   question blocks execution; questions at `:162` also affect straddles, dead
   blinds, rake, and clocks in steps 1–3 and 7. Map each answer to dependent work
   and permit independent steps without claiming unresolved rules are accepted.

After these amendments, phase 5 schema/codecs/chart types and synthetic tests
can proceed independently of library scope and the flop gate. Phase 6 views,
errors, supported-fixture inventory, and the ICM stub are independent of phase 4.
Other steps must name the unresolved rule paths excluded from their acceptance.

Caleb's outstanding phase 5 decisions: library scenario, starting street,
coverage/depth, generation host, `rusqlite` approval, and coverage target.
Phase 6: oracle feature/dependency approval, straddles, dead blinds, rake, and
clock semantics. Uncalled-bet display timing is Astra's design decision.
`serde_json` is already pinned, so phase 5's new-crate wording needs reconciling.

## 5b acceptance: not finished

Astra read the merge handoff and began inspecting the rule/comparison code.
A follow-up helper was interrupted before delivering a completed code review.
The known excluded called-turn-all-in chance nodes still need numerical closure.
The downloaded turn artifacts have not been unpacked or compared by Astra.
Fable's reported acceptance and proposed Decision 14 thresholds remain evidence
to verify, not an independent Astra verdict.
