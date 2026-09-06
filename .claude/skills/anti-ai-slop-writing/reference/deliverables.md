<!-- slopcheck: allow streamline solutions deep-dive closing-filler -->
<!-- Those four are quoted below as examples of what not to write. -->

# Applying the standard to what we actually write

One section per artifact. Each gives the reader, the failure mode that artifact
invites, and the test that catches it.

---

## Findings and recommendation documents

Template: [templates/recommendation.md](../../../../templates/recommendation.md).
Reader: a business owner, usually reading on a phone, between jobs.

**The failure mode.** A findings document is the one place where sounding
thorough substitutes for being thorough. Padding survives here because nobody
argues with a paragraph that agrees with them.

**Section by section.**

*The problem.* Open with the cost, in their units. Hours per week, jobs lost,
days of float. "Calls that come in while the crew is on site go to voicemail.
Roughly a third never call back." The number comes before the explanation,
because the number is what makes them keep reading.

*The proposed fix.* One paragraph, written as a sequence of events, in the
order they happen. Name the trigger, the action, and who sees the result. Do not
name the library. Do not use "integrate", "sync", or "streamline" as the verb
that carries the paragraph.

*Estimated effort.* `S`/`M`/`L` plus what it depends on. Dependencies are where
honesty shows: access, data quality, approvals. A fix with no stated dependency
reads as a fix nobody has thought about.

*Expected payoff.* Their numbers where we have them, and an explicit "we do not
have a number for this yet" where we do not. The second is not a weakness. It
is the sentence that makes the first one believable.

*Evidence.* Every fact above traces to a `[[wikilink]]`. This is the CLAUDE.md
traceability rule, and it is also the strongest anti-slop device we have: a
sentence that cannot name its source is usually a sentence we invented.

**The test.** Cover the client's name and hand the document to the other
partner. If they cannot tell which client it is about, it is slop, however
polished.

---

## Client emails

Reader: someone who opened this between two other things.

- First sentence says what this email is for. No "I hope this finds you well".
- One ask per email, in bold or on its own line, with a date attached to it.
- No closing filler. "Let me know if you have any questions" adds nothing.
  End with the next action and who owns it: "I'll send the draft Thursday.
  Nothing needed from you before then."
- Length is the tell. Past about 150 words, an owner scrolls and skims. If it
  needs to be longer, it needs to be a document with the email as a pointer.

---

## Proposals and anything selling the paid call

Reader: a stranger deciding whether we are worth $499.99 and ninety minutes.

This is where hype pressure is highest and where it does the most damage,
because a trades owner has already been called by four vendors this month who
all said "tailored solutions".

- Claims are things we can actually do. No testimonials until we have real ones.
- Never describe the deliverable in adjectives. Describe what physically arrives:
  a written findings and recommendations document, in their hands, theirs to
  keep whether or not they hire us for phase two.
- Specificity is the whole sales argument. "Ninety minutes on a video call with
  both of us" outsells "a deep dive into your operations" because the first one
  is checkable and the second one is a mood.
- The site sells the call through a request form, never a calendar. Copy that
  implies instant booking is wrong on the facts, not on style.

---

## Website copy

Rules live in [website/README.md](../../../../website/README.md) and are
enforced in part by `website/build.py`, which fails the build on an em dash or
on visible TODO text.

- Plain language, no hype verbs, no em dashes.
- Name the moment. The site works because it describes a specific Tuesday:
  last night's quote typed in twice, the reschedule text that lives in your
  head, the review you meant to ask for after the Hendricks job.
- No all-caps headlines and no eyebrows above headings.
- Run `slopcheck.py website/index.html` before shipping copy. It should come back
  with zero banned findings, which is where it stands today.

---

## Client-facing READMEs and runbooks

Template: [templates/automation-runbook.md](../../../../templates/automation-runbook.md).
Reader: the other partner, cold, in six months, at the moment something broke.

Slop here is different. It is not hype, it is the confident description of
behaviour nobody verified.

- Say what it does, how to run it, and what breaks if it stops running. That
  third one is the section people skip and the reason the document exists.
- "Handles errors gracefully" is slop. "Retries three times on a timeout, then
  writes to `logs/run.log` and exits non-zero" is a runbook.
- Every claim about behaviour is a claim you tested. If it was not run, say so
  in the document. Rule 1 in written form.

---

## Commit messages

Reader: the other partner, reading `git log` to work out why a line exists.

- Subject line says the change. Body says why, which is the part that is not
  recoverable from the diff.
- No "improved", "enhanced", "refactored for clarity" on their own. Those
  describe how you felt about the change rather than what changed.
- Past tense, plain, one idea per commit.

---

## Internal notes

Reader: us, later.

The presentation rules relax. Em dashes are fine, headings can be questions,
nobody cares about tone. Run the checker with `--internal` and it drops those
and keeps the rest.

The thinking rules do not relax at all, and one gets stricter: a note that
records a fact must record where it came from on the same line. An interview
note without a source attribution becomes an unsourced claim in the findings
document three weeks later, and by then nobody remembers whether the owner said
it or we assumed it.
