<!-- slopcheck: off -->
<!-- This file is a catalogue of bad writing. It quotes every banned word on purpose. -->

# The catalogue

Every rule `slopcheck.py` enforces, why it is a tell, and what to write instead.
Read this when a finding looks unfair, or before adding a rule of your own.

Two tiers. **banned** words carry no information in our writing and fail the
check. **review** words are sometimes the right word, so they are reported and
you decide. Both come with the same instruction underneath: say the thing you
were gesturing at.

---

## 1. Words that stand in for a fact

The largest family. Each one names an outcome without committing to what the
outcome is, which is exactly why a model reaches for it and why a client cannot
argue with it.

| Word | Why it is a tell | Write instead |
|---|---|---|
| leverage, utilize, harness | Longer synonyms for "use", chosen for weight | use |
| streamline | Names an improvement without naming a step | "the evening re-typing stops" |
| seamless, frictionless, effortless, hassle-free | Claims nothing goes wrong, promises nothing specific | what connects to what, and what happens when it fails |
| empower, elevate, unlock, transform | Outcome words with the outcome removed | what they can do afterwards that they could not do before |
| drive results, drive growth, move the needle | Motion without a direction or a number | what goes up, by how much, measured how |
| optimize, enhance, improve efficiency | The verb is doing the work the number should do | "quotes go out the same day instead of the next" |
| actionable insights, key takeaways | Announces value rather than delivering it | the action itself |
| tailored, bespoke, custom solutions | Every vendor says this | what is specific to them, named |
| holistic, end-to-end, comprehensive | Claims coverage without listing it | the list |
| robust, scalable | Fine in a runbook if you say what it survives | "retries three times on a timeout, then logs and stops" |
| peace of mind | Sells a feeling in place of a mechanism | what stops going wrong |

**The pattern:** each of these is a lid on a box. Take the lid off and write
what is inside. If the box is empty, that is the real finding, and the honest
move is to say we do not know yet.

---

## 2. Inflation

Words that raise the temperature of a sentence without adding to it. A trades
owner has heard all of them from the last three vendors who called.

`cutting-edge` · `state-of-the-art` · `best-in-class` · `world-class` ·
`industry-leading` · `next-level` · `game-changer` · `revolutionize` ·
`supercharge` · `turbocharge` · `unparalleled` · `unmatched` · `transformative` ·
`vibrant` · `pivotal` · `paramount` · `boasts`

Fix: delete the word and read the sentence again. If it now says nothing, the
word was carrying the sentence, and the sentence needs a fact instead. If a
comparison is genuinely true, show the comparison rather than asserting the rank.

---

## 3. Machine vocabulary

Words that are rare in ordinary business speech and common in generated text.
Their presence is close to a signature.

`delve` · `tapestry` · `testament to` · `realm of` · `myriad` · `plethora` ·
`meticulous` · `curated` · `foster` · `underscore` · `resonate` · `embark` ·
`paradigm` · `beacon of` · `navigate the` · `unpack the` · `deep dive`

Fix: use the word you would use out loud. "We went through six months of
invoices" beats "we delved into the invoicing data".

---

## 4. Throat clearing

Openers that delay the sentence they introduce. All are deletable with no loss.

`when it comes to` · `it is worth noting` · `it should be noted` ·
`needless to say` · `at the end of the day` · `in today's fast-paced world` ·
`in essence` · `simply put` · `rest assured` · `look no further` · `buckle up` ·
`moving forward` · `in order to`

Fix: start the sentence at its subject. "When it comes to invoicing, the delay
is real" becomes "Invoices go out nine days late on average".

Paragraph-opening connectives get their own rule, because they are the tell that
a paragraph has nothing new in it: `Moreover` · `Furthermore` · `Additionally` ·
`Ultimately` · `Essentially` · `Notably` · `Importantly` · `Crucially` ·
`In conclusion` · `In summary` · `To sum up` · `All in all`. Mid-sentence these
words are ordinary English and the checker leaves them alone. At the start of a
line they usually mean the paragraph should be deleted.

---

## 5. Shapes

Structure gives a model away faster than vocabulary does.

**The antithesis snap.** "It's not just software, it's a way of working."
"This isn't about tools. It's about hours." The shape manufactures profundity by
denying something nobody claimed. Make the positive claim once and stop.

**The reveal tease.** "Here's the thing:" · "The best part?" · "And here's why."
Withholding a fact for one clause is a blog trick. Say the thing.

**The hook question.** "Ever wondered where the hours go?" · "Sound familiar?"
Opening by asking the reader a question you are about to answer yourself puts
them in the position of an audience. Open with the fact.

**The hype signoff.** "You're well on your way." · "You've got this." ·
"Happy building!" End on the next action instead, with an owner and a date.

**Chat residue.** "Great question" · "Certainly!" · "I'd be happy to" ·
"Let's break this down" · "In this guide, we'll explore" · "I hope this helps".
These come from a conversation that the reader was not part of. Start at the
answer.

**The em dash.** The strongest single tell in current machine prose, and already
house style: `website/build.py` fails the build on one in the site copy. A comma,
a full stop, or a colon does the job every time. The checker also catches en
dashes and the `&mdash;` entity.

**Headings as questions.** "Why does this matter?" makes the reader open the
section to learn the answer. "The office loses six hours a week here" lets them
skim the answers and stop at the one they care about. Reported as review, since
an FAQ page is a real exception.

**Emoji in headings.** Reported as review. Never in client-facing work.

---

## 6. Hedging and vagueness

Where slop hides from being checkable.

| Pattern | Why | Fix |
|---|---|---|
| significantly, dramatically, substantially, greatly, vastly | An adverb where a number belongs | count it, or drop the adverb |
| various, numerous, several, a number of | Avoids committing to a count you could have made | say how many |
| can help, may be able to, has the potential to | Promises without promising | say what it does, or say we are not sure yet |
| a wide range of | Avoids listing | list them |
| studies show, research suggests, experts agree | Authority with no source, which breaks the traceability rule outright | cite it or cut it |
| three or more hedges in one sentence | You have not decided | decide, or write down what you would need to know to decide |

The hedge stack is worth dwelling on. "This might potentially help reduce some
of the delays that typically occur" contains no claim at all. Either the delay
is measurable and you have the measurement, or you do not and should say so.
Both are respectable. The sentence above is neither.

---

## 7. Business speak

Reported as review, because we do sometimes talk this way and it is not a
machine tell so much as a distance tell.

`circle back` · `touch base` · `reach out` · `bandwidth` · `move the needle` ·
`low-hanging fruit` · `going forward`

Fix: call, email, ask, capacity, and the name of the job.

---

## 8. What the checker cannot see

Three failures survive a clean run, and all three matter more than any word on
this page.

**The confident empty paragraph.** Grammatical, specific-sounding, and true of
every business in the trade. Test it: swap the client's name for a competitor's.
If it is still true, it says nothing about this client.

**The unfalsifiable recommendation.** "Improve communication between the office
and the field" cannot be wrong, so it cannot be right. A recommendation should
be checkable in ninety days.

**The restated question.** Answering "how long does quoting take?" with "quoting
takes a variable amount of time depending on the job" is a mirror, not an answer.

---

## Adding a rule

A rule earns its place when it has fired on real writing of ours more than once.
Add a row to the matching table in `slopcheck.py`, write the fix as an
instruction rather than a scold, and add a test in both directions: one string
that must fire it, and one piece of ordinary prose that must not. The false
positive test is the one that keeps the checker worth running.
