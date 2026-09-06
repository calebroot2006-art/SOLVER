---
name: anti-ai-slop-writing
description: Write and edit prose that does not read as machine-generated. Use when drafting, rewriting, tightening, or reviewing anything a person will read: findings and recommendation documents, client emails and proposals, website and marketing copy, READMEs, runbooks, commit messages. Also use when asked to make writing sound human, less generic, less corporate, or less like AI wrote it. Ships slopcheck.py, a linter for the tells. Not for code.
---

# Anti AI slop writing

## The one idea

Slop is writing that could have been produced without knowing anything.

Every rule here is a way of forcing prose to prove it knows something. That is
the same standard as the traceability rule in CLAUDE.md, applied one sentence at
a time. A finding that traces to a source cannot be slop. A claim that traces to
nothing usually is, however well it reads.

The test, applied to any sentence:

> Could a competitor publish this exact sentence, under their own name, without
> changing a word?

If yes, the sentence carries no information. Cut it, or replace it with the fact
underneath it.

## When this fires

Anything a client or a visitor reads: consulting deliverables, recommendations,
emails, proposals, the website, client-facing READMEs and runbooks. Also commit
bodies, because the other partner reads those cold months later.

Not code. Not config. For notes only the two of us will ever read, the standard
still holds for the thinking, so run the checker with `--internal`.

## The four passes

**1. Substance.** Before drafting, answer three things in one line each:

- Who reads this, and what do they know already?
- What one thing should they do or believe afterwards?
- What fact makes that true, and where did the fact come from?

If the third has no answer, stop. Writing is not the missing step. Go and get
the fact, or write down plainly that we do not have it yet.

**2. Draft.** Write only what you would defend if the client pushed back on it
in the room. Lead with the finding, not with context. Owners read on a phone
between jobs, so put the answer in the first sentence and the heading.

**3. Strip.** Run the checker:

```
python .claude/skills/anti-ai-slop-writing/slopcheck.py <paths> [--strict] [--stats]
python .claude/skills/anti-ai-slop-writing/slopcheck.py --changed
```

Every `banned` hit gets fixed. Every `review` hit gets a decision, not a
reflex. The checker prints what to do instead of each one. Exit code is 1 on
any banned hit, so it drops straight into a commit check.

The checker catches the vocabulary and the shapes. It cannot catch a confident
paragraph that says nothing, so passes 1 and 4 stay yours.

**4. Specificity.** Walk every claim. Each one gets a number, a name, a date, or
a `[[wikilink]]` to the note it came from. A claim that gets none of the four is
either cut or rewritten as an open question. "Roughly a third never call back"
is a claim. "Improves customer retention" is a wish.

Then read it aloud. Anything you would not say out loud to a roofer across a
table gets rewritten in the words you would have used.

## The house voice

Ours is already written down, on the site and in [website/README.md](../../../website/README.md):
plain language, no hype verbs, no em dashes. What that sounds like in practice:

<!-- slopcheck: allow streamline leverage vague-magnitude -->
<!-- The left column below is deliberate slop. The rest of this file passes --strict. -->

| Slop | Ours |
|---|---|
| Our solutions streamline operations for trades businesses. | Two calls missed while you're on a roof. One of them dials the next company on the list. |
| This significantly improves invoicing efficiency. | Invoices at the kitchen table. Last week's still say "sent". |
| Leverage automation to unlock new levels of productivity. | The evening re-typing stops. Quotes go out the same day they're written. |

What the right column has in common: a specific moment, a concrete noun, a verb
that names a real action, and nothing a competitor could copy without lying.

Four habits carry most of it:

1. **Name the moment, not the category.** "The review you meant to ask for after
   the Hendricks job" beats "customer review generation".
2. **Numbers where numbers exist, silence where they do not.** Never dress up a
   guess as a measurement, and never hide behind an adverb instead of counting.
3. **Second person, present tense, short sentences.** The reader is the owner
   and the subject is their week.
4. **No em dashes.** House style, and `website/build.py` already fails the build
   on one in the site copy. Use a comma, a full stop, or a colon.

## Reference

- [reference/tells.md](reference/tells.md) is the catalogue: every word and shape
  the checker flags, why it is a tell, and what to write instead. Read it when a
  finding is unclear or when you want to add a rule.
- [reference/deliverables.md](reference/deliverables.md) applies the standard to
  each thing we actually write: findings documents, recommendations, client
  emails, proposals, READMEs, commit messages.

## The bar

The deliverable is the product, and the client is reading over your shoulder.
Prose that survives the checker is not automatically good. It has only stopped
sounding like a machine wrote it. Whether it says anything is still on you.

## Maintaining the checker

Rules live in `slopcheck.py` as three tables: `BANNED_WORDS`, `REVIEW_WORDS`,
`STRUCTURE_RULES`. Adding one means adding a row and a test.

```
cd .claude/skills/anti-ai-slop-writing
uv run --no-project --with pytest -- python -m pytest tests/ -q
uv run --no-project --with black --with ruff -- black . && ruff check .
```

Two properties are worth protecting. The checker must stay quiet on prose that
is already in our voice, because a noisy linter gets switched off, and the test
suite pins that with our real site copy. It must also point at the real line
after code, frontmatter and markup are ignored, which `test_line_and_column_survive_stripping`
pins.
