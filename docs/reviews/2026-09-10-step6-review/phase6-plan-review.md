# Phase 6 plan review: needs changes; fixture scope is now concrete

Astra reviewed `docs/phase-6/PLAN.md` at `2f81339` on 2026-09-10. The helper
checked the plans and inventoried upstream fixtures read-only. Astra read the
plan, relevant research and PHH specification, then independently fetched and
validated the eleven supported data files. No upstream Python code was executed
and no engine replay was claimed; the engine remains a skeleton.

**P6-1, high: the proposed 83-hand gate cannot run on this engine.** Plan lines
93–98 promise all 83 PPC hands, but only eleven are no-limit Hold'em. The helper's
static inventory at PokerKit `54571ddda38a7da9b8c527d54c16c788ac14dac7` counted
11 NLHE, 13 stud, 14 Omaha eight-or-better, 10 razz, and seven each of PLO, limit
Hold'em, no-limit deuce-to-seven single draw, stud eight-or-better, and limit
deuce-to-seven triple draw. [Pinned PPC test source](https://github.com/uoftcprg/pokerkit/blob/54571ddda38a7da9b8c527d54c16c788ac14dac7/pokerkit/tests/test_wsop/test_2023_43_5.py).

PokerKit stores those fixtures as Python tests. The PHH files are in the separate
`uoftcprg/phh-dataset` repository. Pin its revision
`e47fbd5816372360bade4de5d712346fe1bb70f6` and the eleven files listed below; record
the data repository's license and attribution before copying fixtures. Preserve
the engine's NLHE scope and reject unsupported variants by name. [Pinned data directory](https://github.com/uoftcprg/phh-dataset/tree/e47fbd5816372360bade4de5d712346fe1bb70f6/data/wsop/2023/43/5).

```
00-02-07.phh  00-08-38.phh  00-15-36.phh  00-18-39.phh
02-51-10.phh  02-53-09.phh  02-54-12.phh  02-56-12.phh
02-57-27.phh  03-00-32.phh  03-02-41.phh
```

Astra's `check_phh_inventory.py` verified these as `NT`, five players, known cards,
integer chip amounts, complete starting/finishing stacks, and
`ante_trimming_status=false`. Their ante is 1.5 big blinds, so import the recorded
amount. File hashes and relevant fields are saved in `phh-inventory.json`. These
hands correspond to PPC hands 1–4 and 62–68. Eleven five-handed histories need
separate tests for heads-up and 6–9 seats, short stacks, straddles, and dead blinds;
they do not establish those rules by themselves.

**P6-2, high: short-stack posting and ante eligibility lack a contract.** Plan line
60 says `TableConfig::validate` rejects an ante over the stack, but `TableConfig`
has no stacks. Line 65 specifies ante-first posting; the existing tournament
research describes blind-first priority for a short big blind. Define the selected
rule set explicitly and separate configuration validation from stack-dependent
hand construction. Legal short stacks need partial forced postings; they should
not be rejected merely for being unable to cover a full ante.

PHH's `ante_trimming_status` changes eligibility for ante money when stacks are
short, yet the planned history struct omits it. Preserve the setting or refuse
unsupported semantics. Include ante money explicitly in pot construction and
eligibility. Check stacks below the ante, below the blind, and between the blind
and blind-plus-ante; assert per-seat awards as well as conservation. This is an
unresolved rule requirement, not an instruction to assume one house rule across
all game modes. [PHH ante semantics](https://phh.readthedocs.io/en/stable/optional.html#ante-trimming-status).

**P6-3, high: PHH serialization does not establish action or visibility semantics.**
Plan lines 84–89 promise a typed round trip without defining the adapter. Map PHH's
one-based positional player indices to engine seats, including heads-up reversal
of forced-bet assignment. `cbr` is a raise-to amount; `cc` means check or call
according to legal state. Test those mappings with expected legal actions and
stacks after each transition, not merely `decode(encode(x)) == x`.
[PHH positions](https://phh.readthedocs.io/en/stable/spec.html#position),
[PHH forced bets and actions](https://phh.readthedocs.io/en/stable/required.html).

Private dealing and public reveal are different events even though the imported
file knows all hole cards. Two supported histories include four explicit `sm`
actions; the last fixture reveals both players before the board runs out. Keep
unrevealed and mucked cards out of every seat/public view, serialized event stream,
and bot/coach input. Define an internal complete history separately from the
observer's history. Test views and events before and after each reveal, including
all-in runouts, so replay cannot expose future cards. [Pinned all-in fixture](https://github.com/uoftcprg/phh-dataset/blob/e47fbd5816372360bade4de5d712346fe1bb70f6/data/wsop/2023/43/5/03-02-41.phh).

The general PHH format permits partial histories, unknown cards, and non-integer
or unknown stacks. State the accepted phase 6 subset and refuse unsupported cases
without inventing cards, rounding money, or treating an incomplete hand as final.
The eleven chosen fixtures need none of those extensions. Bound file bytes,
action counts, string lengths and allocations before parsing imported histories.
[PHH required fields](https://phh.readthedocs.io/en/stable/required.html).

**P6-4, medium: decision dependencies are understated.** The Progress section says
only question 1 blocks execution. Straddle location/re-straddles affect steps 1–2;
dead-blind policy affects steps 1–3; rake affects pots and conservation; clock
semantics affect step 7. Each step must name unresolved paths excluded from its
acceptance. A rule divergence from `rs_poker` needs an explicit expected result and
reason, not just a comment beside a failing comparison. If its oracle step is
deferred, the complete phase gate remains incomplete.

## Astra's design decision and amended sequence

Return the uncalled portion when the betting round closes and it is established
that no opponent can match it. Emit a distinct return event before pot awards;
the display can animate that return before showdown. Pot amounts shown as
contestable exclude the returned chips. This resolves question 5's display choice
and keeps animations tied to recorded state changes.

After amendment, state/view/error types, a bounded PHH subset, fixture inventory,
and the ICM stub can proceed independently of phase 4 and of unresolved product
rules. Betting and pot work may proceed for explicitly selected rules; it must
not silently choose Caleb's straddle, dead-blind, rake, or clock answers.

Retain the three-all-in/fold known-answer case, then add cumulative short raises
and reopening rights, an uncalled excess, tied side pots with odd chips, and
simultaneous busts with equal starting stacks. Specify the expected rule for each.
Use checked chip arithmetic throughout; a sum wrapping in both sides of a
conservation assertion would not prove conservation. Keep all seat counts and
the seed/case count explicit in randomized tests.

Caleb still owns the `rs_poker` arena dependency approval, straddle scope,
dead-blind enforcement, rake, and tournament clock choices. The amended plan
must map them to dependent steps. The full phase 6 plan is not accepted as written.
