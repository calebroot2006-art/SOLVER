# Step 5b acceptance review: captured results reproduced; gate needs changes

Astra reviewed the merged tooling at `2f81339` on 2026-09-10 and reran it against
the step 6 captures at `e338d7a`. Step 5b was already merged at `114b815`; this
report does not undo that merge. It withholds independent acceptance of the gate
until the three demonstrated defects below and the stated coverage gap are closed.
The helper found the three defects; Astra personally ran every reproduction and
inspected the affected rules and comparison code.

## Confirmed acceptance defects

**B1, high: root EV agreement is never enforced.**
`tests/reference/turn/compare.py:717–737` computes the root differences, but
`joint_report:1143–1149` does not put them into gate failures. Starting from an
accepting shipped fixture, changing root values to `[1000, -1000]` still returns
`accepted=true`, zero failures and zero stale rows. The handoff explicitly claims
root agreement is a gate.

Validate both root vectors' dimensions, finite values, zero-sum consistency, and
correspondence to the captured strategy/measurements. Enforce the reviewed root
agreement rule. Approximate equilibria need a residual-aware comparison; identical
games do not imply bit-identical root values from different converged strategies.
Use a documented numerical bound and rejection tests, rather than borrowing a
per-combo frequency threshold or silently requiring 1e-9 equality between solvers.

**B2, high: recorded oracle evidence survives changes to its inputs.**
`compare.py:1029–1063` compares stored oracle values with the captured parent EV;
it never recomputes the oracle. The stale check at `:847–898` binds recorded C rows,
not all policies and continuation values the oracle read downstream.

The reproduction first constructs an accepting C row using the real oracle and
shipped tolerances, with values `[-1, 2.5]`. Changing a downstream policy from
`[0.5, 0.5]` to `[0.51, 0.49]` leaves acceptance and staleness unchanged. Rerunning
the oracle gives `[-0.98, 2.5]`. Changing the captured chance continuation instead
gives `[49, 2.5]` and is also accepted. Both exceed the 1e-9 oracle-agreement rule.

Recompute C-row evidence from the current capture or bind it to a complete,
validated fingerprint of its dependencies, including canonical inputs, downstream
policies, continuation values, oracle version, and conventions. A record generated
from Linux may still validate Windows, but that requires equality of the relevant
numerical content rather than ignoring changed dependencies or OS-specific metadata.
Test both mutations and a downstream change that does not itself become a C row.

**B3, high: contradictory reach/availability evidence can excuse a discrepancy.**
`compare.py:651–657,706–713` trusts exported reach and EV availability;
`review_rule.py:199–246` classifies missing EV or low reach as B. Root mixes of
0.40 versus 0.75 pass if all six project root EVs are cleared while their positive
reach/mass remains unchanged. They also pass if root reach is falsely set to zero
while ranges and policies remain unchanged. Both mutations produce six B rows.

This is separate from Caleb's choice of B thresholds. Validate root reach from
ranges and board, and reconstruct path reach/compatible mass where those fields
excuse a difference. Check that unavailable EVs follow the producer's stated
semantics. Contradictory evidence must fail before A/B/C classification. Test
missing values at a reached root, false zero reach, blockers, genuine zero-action
reach, and numerical underflow without treating missing as zero.

Run the saved reproduction from the review checkout root:

```
python -B docs/reviews/2026-09-10-step6-review/check_acceptance_mutations.py
```

`acceptance-mutation-results.json` records two accepting controls and five bad
mutations that are also accepted. The script exits zero when it reproduces these
defects; that is not a passing acceptance test for corrected tooling.

## Evidence reproduced on the actual captures

`check_captures.py` compared step 5b run `34510476253` with step 6 run `34524553796`
on both operating systems. Each before/after turn capture contains 2,501,439 scalar
fields. No solved field changed; all differences are confined to timings (including
renamed timing fields and checkpoint elapsed time), revision, progress interval,
and the two memory-accounting fields. Inputs, policies, EVs, reach, measurements,
and stopping iterations agree exactly. Raw and refined river captures agree with
the accepted `2930550` records on all solved fields too. The river memory fields
are 136 bytes higher. `capture-check-results.json` saves hashes, differences by
field, timing-schema changes and every non-timing difference. The full timing
diff is retained under `target/review-evidence/astra-full-capture-check-results.json`.

The existing `joint_report` function was rerun on both step 6 captures with the
committed review, shipped rules, and the exact expected revision. Both return
accepted, zero missing rows and zero stale rows. This describes the current gate's
behavior; B1–B3 limit what that acceptance establishes.

| Case | A / B / C rows | Project / reference residual, % pot | Largest root EV difference, chips | Reported all-row loss, fraction of pot |
|---|---|---|---:|---:|
| Dry rainbow | 10,998 / 32,193 / 351 | 0.208210 / 0.158694 | 0.003370871 | 0.001082549 |
| Paired | 17,029 / 35,561 / 189 | 0.166112 / 0.178302 | 0.000912890 | 0.000495645 |
| Flush possible | 9,913 / 32,227 / 277 | 0.207552 / 0.185103 | 0.000667413 | 0.000836184 |

Astra also generated fresh oracle evidence from the Linux capture with:

```
python -B tests/reference/turn/review_combos.py target/review-evidence/step6-turn-linux/cases.toml target/review-evidence/step6-turn-reference/cases.json target/review-evidence/astra-fresh-per-combo-review.json
```

All three fresh case records are exactly equal to the committed case records.
All 817 C rows agree; maximum absolute difference is `6.856737400084967e-13`
chips. There are 700 independent showdown walks and 117 rows using reported
chance continuation values. `fresh-oracle-results.json` saves the summary and
input hashes. The Windows numerical inputs are unchanged in the capture checks;
the expensive fresh generation was run on Linux data only.

The existing turn Python suite passed all 144 tests. The first two sandboxed runs
had five temporary-directory permission errors; an approved run outside the
sandbox, with temporary files inside this checkout, passed. No tests or thresholds
were weakened. The new mutations explain why the existing green suite is not
sufficient evidence of gate correctness.

## Remaining coverage and Decision 14

The known called-turn-all-in gap remains at `compare.py:537–575`: it recognizes
matching terminal contributions and excludes exported river descendants without
a numerical comparison. The three cases each have two such histories; only
3/4/3 descendants are exported per history, from 48 board-legal river outcomes.
Add a numerical check of the parent continuation on compatible private deals and
all legal rivers, with explicit payoff origin and chance normalization. Test
wrong payoff, wrong runout probability, and omitted/duplicated outcomes. Passing
the small Rust all-in enumeration tests supports the kernel but does not close
this omission in the real-capture gate.

Do not call the 117 continuation-dependent oracle rows fully independent: their
leaf values come from the solver being checked. The fresh run verifies internal
agreement at those leaves, not an independent turn best response or complete
turn solve. Extending that scope requires more exported continuation data or an
independent traversal/evaluator over the omitted runouts.

Decision 14 remains Caleb's: A indifference `0.01` pot, B reach floor `1e-6`, and
C aggregate budget `0.005` pot are still candidates. The reported all-row totals
cover differing rows over exported runouts. They are not a certificate for replacing
the complete strategy, nor an error bound for a single coaching decision. Fixing
B1–B3 does not require choosing those thresholds. Preserve the measured record,
correct the gate, and rerun both operating systems before independent acceptance.
