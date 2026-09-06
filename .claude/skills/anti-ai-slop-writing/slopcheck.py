#!/usr/bin/env python3
"""slopcheck: flag AI-slop tells in prose before a client ever reads it.

Reads Markdown and HTML, ignores code and markup, and reports two tiers:

    banned  words and constructions that do not belong in our writing
    review  words that are usually a symptom, but are occasionally right

Exit status is 1 when anything banned is found, or when --strict is passed and
anything at all is found. A file that passes is not automatically well written.
It has only stopped sounding like a machine produced it. The thinking is still
yours to do.

Usage:
    python slopcheck.py <path> [<path> ...] [--strict] [--stats] [--json]
    python slopcheck.py --changed
    python slopcheck.py CLAUDE.md --internal

Rules are written for prose a client or a visitor reads. Pass --internal for
notes only the two of us will ever see: it drops the rules that are about
presentation rather than thinking, chiefly the em dash ban.

A file containing the comment `<!-- slopcheck: off -->` anywhere is skipped
whole. A file containing `<!-- slopcheck: allow name-of-rule other-rule -->`
skips only those rules. Use the allow form and say why on a neighbouring line.
Use the off form only for catalogues of bad writing, such as this skill's own
reference notes.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

BANNED = "banned"
REVIEW = "review"

TEXT_SUFFIXES = frozenset({".md", ".markdown", ".html", ".htm", ".txt"})

# Nothing in these is ours to edit, and a tool README inside one is noise.
SKIP_DIRS = frozenset(
    {
        ".git",
        ".pytest_cache",
        ".ruff_cache",
        "__pycache__",
        "node_modules",
        ".venv",
        "venv",
        "dist",
        "build",
        ".obsidian",
    }
)


@dataclass(frozen=True)
class Rule:
    """One thing we look for, and what to do about it."""

    name: str
    pattern: re.Pattern[str]
    tier: str
    fix: str


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    col: int
    rule: str
    tier: str
    text: str
    fix: str


# ---------------------------------------------------------------------------
# The vocabulary. Each entry is (rule name, pattern body, what to do instead).
# Pattern bodies get word guards at build time, so write them bare.
# ---------------------------------------------------------------------------

BANNED_WORDS: list[tuple[str, str, str]] = [
    ("delve", r"delv(?:e|es|ed|ing)", "say what you did: read it, sat in on it, counted it"),
    ("tapestry", r"tapestry", "cut the sentence, it is decoration"),
    ("testament", r"(?:a )?testament to", "state what it is evidence of, and cite the source"),
    ("realm", r"(?:the )?realm of", "name the actual area of the business"),
    ("leverage", r"leverag(?:e|es|ed|ing)", "use"),
    ("utilize", r"utiliz(?:e|es|ed|ing|ation)", "use"),
    ("seamless", r"seamless(?:ly)?", "name what connects to what, and what happens when it fails"),
    ("streamline", r"streamlin(?:e|es|ed|ing)", "name the step that disappears"),
    (
        "unlock",
        r"unlock(?:s|ing)? (?:the )?(?:value|potential|growth|insights?|efficiency|savings)",
        "say what they get, and how much",
    ),
    ("elevate", r"elevat(?:e|es|ing) (?:your|their|the)", "say what improves and by how much"),
    (
        "empower",
        r"empower(?:s|ed|ing)?",
        "say what they can do afterwards that they could not do before",
    ),
    ("harness", r"harness(?:es|ed|ing)?", "use"),
    ("foster", r"foster(?:s|ed|ing)?", "build, cause, or cut the verb"),
    ("myriad", r"myriad", "say how many"),
    ("plethora", r"plethora", "say how many"),
    ("cutting-edge", r"cutting[- ]edge", "name the tool and its version"),
    ("state-of-the-art", r"state[- ]of[- ]the[- ]art", "name the tool and its version"),
    (
        "best-in-class",
        r"best[- ]in[- ]class|world[- ]class|industry[- ]leading|next[- ]level",
        "drop it, or show the comparison that proves it",
    ),
    ("game-changer", r"game[- ]chang(?:er|ing)", "say what changes"),
    ("revolutionize", r"revolutioniz(?:e|es|ed|ing)", "say what changes"),
    (
        "supercharge",
        r"supercharg(?:e|es|ed|ing)|turbocharg(?:e|es|ed|ing)",
        "say what gets faster, and by how much",
    ),
    ("holistic", r"holistic(?:ally)?", "list what is actually included"),
    ("synergy", r"synerg(?:y|ies|istic)", "name the two things, and what one does for the other"),
    ("embark", r"embark(?:s|ed|ing)?", "start"),
    (
        "deep-dive",
        r"deep[- ]div(?:e|es|ed|ing)|div(?:e|es|ing) (?:in|into|deeper)",
        "say what you looked at",
    ),
    ("curated", r"curat(?:e|es|ed|ing|ion)", "say how you chose"),
    ("bespoke", r"bespoke", "custom, or say what is specific to them"),
    ("meticulous", r"meticulous(?:ly)?", "show the care instead of claiming it"),
    (
        "ever-evolving",
        r"ever[- ]evolving|rapidly (?:evolving|changing)|fast[- ]paced world|in today's [a-z-]+ (?:world|landscape|market)",
        "cut it, it is throat clearing",
    ),
    ("paradigm", r"paradigm(?: shift)?", "say what changed"),
    (
        "transformative",
        r"transformative|transform(?:s|ing)? (?:your|their) (?:business|workflow|operations)",
        "say which job takes less time now",
    ),
    ("when-it-comes-to", r"when it comes to", "start the sentence at the subject"),
    (
        "worth-noting",
        r"it'?s worth noting|it is worth noting|it (?:is|should be) important to note|it should be noted",
        "if it is worth noting, note it, and drop the preamble",
    ),
    ("needless-to-say", r"needless to say", "then do not say it"),
    ("end-of-the-day", r"at the end of the day", "cut it"),
    ("rest-assured", r"rest assured", "cut it, reassurance without evidence reads as sales"),
    ("look-no-further", r"look no further", "cut it"),
    ("buckle-up", r"buckle up", "cut it"),
    (
        "solutions",
        r"(?:innovative|tailored|custom|cutting[- ]edge|end[- ]to[- ]end|comprehensive) solutions?",
        "name the thing you would build",
    ),
    (
        "actionable-insights",
        r"actionable insights?|key takeaways?",
        "give the action, the reader can see it is actionable",
    ),
    (
        "drive-results",
        r"driv(?:e|es|ing) (?:results|growth|value|efficiency|success|revenue)",
        "say what goes up, by how much, measured how",
    ),
    ("unparalleled", r"unparalleled|unmatched", "drop it, or show the comparison"),
    ("boasts", r"boasts", "has"),
    ("journey", r"(?:your|their|our|the customer'?s?|this) journey", "name the sequence of steps"),
    (
        "landscape",
        r"(?:business|digital|competitive|industry|marketing|technology|tech|current) landscape",
        "name the market, and the companies in it",
    ),
    (
        "navigate",
        r"navigat(?:e|es|ing) (?:the|this|your|their|complex)",
        "say what they have to do",
    ),
    ("unpack", r"unpack(?:s|ed|ing)? (?:the|this|what|why|how)", "explain it"),
    ("moving-forward", r"moving forward|going forward", "say when: a date, or the next milestone"),
    ("double-edged", r"double[- ]edged sword", "state both effects plainly"),
    (
        "hassle-free",
        r"hassle[- ]free|effortless(?:ly)?|frictionless|pain[- ]free",
        "say how many steps it takes now",
    ),
    ("simply-put", r"simply put|put simply|in essence", "then put it simply the first time"),
    ("arguably", r"arguably", "make the argument, or drop the claim"),
    ("vibrant", r"vibrant", "describe what it looks like"),
    ("beacon", r"beacon of", "cut it"),
    ("pivotal", r"pivotal|paramount", "say why it matters, concretely"),
    ("underscore", r"underscor(?:e|es|ed|ing)", "say what it shows"),
    ("resonate", r"resonat(?:e|es|ed|ing)", "say who agreed, and what they said"),
    (
        "align-with",
        r"align(?:s|ed|ing)? with (?:your|their|the) (?:goals|needs|vision|values|objectives|priorities)",
        "name the goal, and how this serves it",
    ),
    (
        "unsourced-authority",
        r"studies show|research suggests|experts (?:say|agree)|it'?s well known|it is well known",
        "cite the source, or cut the claim (traceability rule)",
    ),
    ("peace-of-mind", r"peace of mind", "say what stops going wrong"),
    (
        "youre-not-alone",
        r"you'?re not alone|we'?ve got you covered|we'?ve all been there",
        "cut it",
    ),
    (
        "chat-filler",
        r"great question|certainly!|absolutely!|i'?d be happy to|i hope this helps|let'?s break (?:this|it) down|in this (?:article|post|guide),? we'?ll",
        "cut it, start at the answer",
    ),
    (
        "as-an-ai",
        r"as an ai(?: language model)?|as a language model",
        "delete the sentence and the paragraph around it",
    ),
]

REVIEW_WORDS: list[tuple[str, str, str]] = [
    (
        "robust",
        r"robust(?:ness)?",
        "say what it survives: bad input, a dropped connection, a rate limit",
    ),
    ("comprehensive", r"comprehensive(?:ly)?", "say what is covered, or cut the adjective"),
    ("ensure", r"ensur(?:e|es|ed|ing)", "often filler, check the sentence still works without it"),
    ("crucial", r"crucial|vital|essential", "say what breaks without it"),
    (
        "vague-magnitude",
        r"significant(?:ly)?|dramatic(?:ally)?|substantial(?:ly)?|considerabl[ey]|greatly|vastly",
        "put a number on it, or drop the adverb",
    ),
    ("vague-count", r"various|numerous|a number of|several", "say how many"),
    (
        "can-help",
        r"can help (?:you )?(?:to )?[a-z]+|may be able to|has the potential to",
        "say what it does, or say you are not sure yet",
    ),
    ("in-order-to", r"in order to", "to"),
    ("wide-range", r"a (?:wide )?(?:range|variety) of", "list them"),
    ("low-hanging-fruit", r"low[- ]hanging fruit", "name the job, and the hours it costs"),
    (
        "business-speak",
        r"circle back|touch base|reach out|bandwidth|move the needle",
        "say the plain version: call, email, ask, capacity",
    ),
    (
        "that-said",
        r"that said|having said that",
        "usually deletable, check the sentence stands alone",
    ),
    (
        "closing-filler",
        r"let me know if you have any questions|feel free to reach out|don'?t hesitate to",
        "say the specific next step instead",
    ),
]

# (name, pattern body, tier, fix). These are used as written, not word-guarded.
STRUCTURE_RULES: list[tuple[str, str, str, str]] = [
    (
        "em-dash",
        r"[—–]|&mdash;|&ndash;|&#8212;|&#8211;",
        BANNED,
        "house style forbids em and en dashes, use a comma, a full stop, or a colon",
    ),
    (
        "antithesis-snap",
        r"(?:it'?s|this is|that'?s|we'?re|they'?re)\s+not\s+(?:just\s+)?(?:about\s+)?[^.!?\n]{1,70}[,.;]\s*(?:it'?s|it is|but)\b",
        BANNED,
        "the 'not just X, it's Y' snap, make the positive claim once and stop",
    ),
    (
        "isnt-about",
        r"(?:isn'?t|aren'?t|is not|are not)\s+(?:just\s+)?about\b[^.!?\n]{0,70}[.!?]\s*(?:it'?s|they'?re)\s+about\b",
        BANNED,
        "the same snap split across two sentences, keep the second one only",
    ),
    (
        "paragraph-connective",
        r"(?m)^[ ]{0,3}(?:>[ ]*)?(?:\*\*)?(?:moreover|furthermore|additionally|ultimately|essentially|notably|importantly|crucially|in conclusion|in summary|to sum up|all in all)\b",
        BANNED,
        "these open paragraphs with nothing new in them, delete the word or the paragraph",
    ),
    (
        "hook-question",
        r"what if i told you|ever wondered|have you ever (?:wondered|noticed)|sound familiar\?",
        BANNED,
        "open with the fact, not with a question aimed at the reader",
    ),
    (
        "hype-signoff",
        r"well on your way|you'?ve got this|happy (?:building|coding|automating|reading)!",
        BANNED,
        "end on the next action instead",
    ),
    (
        "reveal-tease",
        r"here'?s the (?:thing|kicker|catch)|the best part\?|and here'?s why|but here'?s the (?:thing|problem)",
        BANNED,
        "just say the thing",
    ),
    (
        "heading-question",
        r"(?m)^[ ]{0,3}#{1,6}[ ]+[^\n]*\?[ ]*$",
        REVIEW,
        "a heading should state the answer, so the reader can skim answers",
    ),
    (
        "heading-emoji",
        r"(?m)^[ ]{0,3}#{1,6}[ ]+[^\n]*[\U0001F300-\U0001FAFF☀-➿]",
        REVIEW,
        "no emoji in headings in client-facing work",
    ),
]

HEDGES = frozenset(
    {
        "may",
        "might",
        "could",
        "possibly",
        "potentially",
        "generally",
        "typically",
        "often",
        "usually",
        "somewhat",
        "relatively",
        "arguably",
        "perhaps",
        "likely",
        "seems",
        "appears",
        "tends",
    }
)

HEDGE_LIMIT = 3
LONG_SENTENCE_WORDS = 40

# Rules about how finished prose is presented rather than how it thinks. They
# hold for anything a client or a visitor reads, and are dropped for --internal.
PRESENTATION_RULES = frozenset({"em-dash", "heading-question", "heading-emoji"})

WORD_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9'’-]*")


def build_rules(internal: bool = False) -> list[Rule]:
    """Compile every rule once, with word guards on the vocabulary patterns."""
    rules: list[Rule] = []
    skip = PRESENTATION_RULES if internal else frozenset()
    for tier, table in ((BANNED, BANNED_WORDS), (REVIEW, REVIEW_WORDS)):
        for name, body, fix in table:
            if name in skip:
                continue
            pattern = re.compile(r"(?i)(?<![\w-])(?:" + body + r")(?![\w-])")
            rules.append(Rule(name=name, pattern=pattern, tier=tier, fix=fix))
    for name, body, tier, fix in STRUCTURE_RULES:
        if name in skip:
            continue
        rules.append(Rule(name=name, pattern=re.compile("(?i)" + body), tier=tier, fix=fix))
    return rules


# ---------------------------------------------------------------------------
# Stripping. Everything that is not prose is blanked in place, so line and
# column numbers still point at the real file.
# ---------------------------------------------------------------------------

FRONTMATTER_RE = re.compile(r"(?s)\A---\n.*?\n---(?:\n|\Z)")
HTML_COMMENT_RE = re.compile(r"(?s)<!--.*?-->")
SCRIPT_STYLE_RE = re.compile(r"(?is)<(script|style)\b[^>]*>.*?</\1\s*>")
FENCE_RE = re.compile(r"(?ms)^(?P<fence>```|~~~)[^\n]*\n.*?^(?P=fence)[^\n]*$")
INLINE_CODE_RE = re.compile(r"`[^`\n]+`")
WIKILINK_RE = re.compile(r"\[\[[^\]\n]+\]\]")
LINK_TARGET_RE = re.compile(r"\]\([^)\n]*\)")
BARE_URL_RE = re.compile(r"(?i)\b(?:https?://|www\.)\S+")
TAG_RE = re.compile(r"(?s)<[^>\n]*>")
ATTR_TEXT_RE = re.compile(r'(?i)\b(?:alt|title|content|placeholder|aria-label)\s*=\s*"([^"]*)"')

OFF_RE = re.compile(r"(?i)<!--\s*slopcheck:\s*off\s*-->")
ALLOW_RE = re.compile(r"(?i)<!--\s*slopcheck:\s*allow\s+([^>]*?)\s*-->")


def _blank(matched: str) -> str:
    return "".join(ch if ch == "\n" else " " for ch in matched)


def _blank_all(text: str, pattern: re.Pattern[str]) -> str:
    return pattern.sub(lambda m: _blank(m.group(0)), text)


def _blank_tag(match: re.Match[str]) -> str:
    """Blank an HTML tag, but keep the human-readable attribute values."""
    raw = match.group(0)
    chars = list(_blank(raw))
    for attr in ATTR_TEXT_RE.finditer(raw):
        start, end = attr.span(1)
        chars[start:end] = list(raw[start:end])
    return "".join(chars)


def strip_noncontent(text: str) -> str:
    """Blank frontmatter, code, markup and URLs, preserving every offset."""
    for pattern in (FRONTMATTER_RE, HTML_COMMENT_RE, SCRIPT_STYLE_RE, FENCE_RE, INLINE_CODE_RE):
        text = _blank_all(text, pattern)
    text = TAG_RE.sub(_blank_tag, text)
    for pattern in (WIKILINK_RE, LINK_TARGET_RE, BARE_URL_RE):
        text = _blank_all(text, pattern)
    return text


# ---------------------------------------------------------------------------
# Checks that need whole sentences rather than a regex.
# ---------------------------------------------------------------------------

PARAGRAPH_RE = re.compile(r"(?m)^[^\n]*\S[^\n]*(?:\n[^\n]*\S[^\n]*)*")
SENTENCE_RE = re.compile(r"[^.!?]*[.!?]+|[^.!?]+\Z")
SKIP_PARAGRAPH_RE = re.compile(r"^[ ]{0,3}(?:#{1,6}[ ]|\||!\[)")


def iter_sentences(text: str):
    """Yield (start offset, sentence) for prose paragraphs only."""
    for para in PARAGRAPH_RE.finditer(text):
        block = para.group(0)
        if not block.strip():
            continue
        first_line = block.split("\n", 1)[0]
        if SKIP_PARAGRAPH_RE.match(block) or "|" in first_line:
            continue
        for sent in SENTENCE_RE.finditer(block):
            body = sent.group(0)
            if body.strip():
                yield para.start() + sent.start(), body


def sentence_findings(text: str) -> list[tuple[int, str, str, str, str]]:
    """Return (offset, rule, tier, matched text, fix) for the computed checks."""
    out: list[tuple[int, str, str, str, str]] = []
    for offset, sentence in iter_sentences(text):
        words = WORD_RE.findall(sentence)
        lowered = {w.lower() for w in words}
        hedges = lowered & HEDGES
        if len(hedges) >= HEDGE_LIMIT:
            out.append(
                (
                    offset,
                    "hedge-stack",
                    REVIEW,
                    " ".join(sorted(hedges)),
                    "three hedges in one sentence means you have not decided, so decide, or say what you would need to know",
                )
            )
        if len(words) > LONG_SENTENCE_WORDS:
            out.append(
                (
                    offset,
                    "long-sentence",
                    REVIEW,
                    str(len(words)) + " words",
                    "split it, owners read these on a phone",
                )
            )
    return out


# ---------------------------------------------------------------------------
# Running over a file.
# ---------------------------------------------------------------------------


def line_starts(text: str) -> list[int]:
    starts = [0]
    for match in re.finditer(r"\n", text):
        starts.append(match.end())
    return starts


def locate(starts: list[int], offset: int) -> tuple[int, int]:
    """Turn a character offset into a 1-based (line, column)."""
    low, high = 0, len(starts) - 1
    while low < high:
        mid = (low + high + 1) // 2
        if starts[mid] <= offset:
            low = mid
        else:
            high = mid - 1
    return low + 1, offset - starts[low] + 1


def check_text(text: str, rules: list[Rule], path: str) -> list[Finding]:
    """Every finding in one document, sorted by position."""
    if OFF_RE.search(text):
        return []
    allowed: set[str] = set()
    for match in ALLOW_RE.finditer(text):
        allowed.update(match.group(1).split())

    prose = strip_noncontent(text)
    starts = line_starts(text)
    findings: list[Finding] = []

    for rule in rules:
        if rule.name in allowed:
            continue
        for match in rule.pattern.finditer(prose):
            snippet = text[match.start() : match.end()].strip()
            line, col = locate(starts, match.start())
            findings.append(Finding(path, line, col, rule.name, rule.tier, snippet, rule.fix))

    for offset, name, tier, snippet, fix in sentence_findings(prose):
        if name in allowed:
            continue
        line, col = locate(starts, offset)
        findings.append(Finding(path, line, col, name, tier, snippet, fix))

    findings.sort(key=lambda f: (f.line, f.col, f.rule))
    return findings


def stats_for(text: str) -> dict[str, float]:
    """Sentence length and how often a sentence carries a number."""
    prose = strip_noncontent(text)
    sentences = [s for _, s in iter_sentences(prose)]
    words = WORD_RE.findall(prose)
    with_number = sum(1 for s in sentences if re.search(r"\d", s))
    return {
        "words": float(len(words)),
        "sentences": float(len(sentences)),
        "mean_sentence_words": round(len(words) / len(sentences), 1) if sentences else 0.0,
        "pct_sentences_with_a_number": (
            round(100 * with_number / len(sentences), 1) if sentences else 0.0
        ),
    }


def collect_paths(raw: list[str]) -> list[Path]:
    paths: list[Path] = []
    for item in raw:
        path = Path(item)
        if path.is_dir():
            paths.extend(
                p
                for p in sorted(path.rglob("*"))
                if p.is_file()
                and p.suffix.lower() in TEXT_SUFFIXES
                and not SKIP_DIRS.intersection(p.parts)
            )
        elif path.is_file():
            paths.append(path)
        else:
            print("slopcheck: no such file: " + item, file=sys.stderr)
    return paths


def changed_paths() -> list[Path]:
    """Files changed against HEAD plus untracked ones. Empty if git is absent."""
    out: list[Path] = []
    for args in (
        ["git", "diff", "--name-only", "HEAD"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    ):
        try:
            result = subprocess.run(args, capture_output=True, text=True, check=True)
        except (OSError, subprocess.CalledProcessError) as exc:
            print("slopcheck: could not ask git for changed files: " + str(exc), file=sys.stderr)
            return []
        for name in result.stdout.splitlines():
            path = Path(name)
            if path.is_file() and path.suffix.lower() in TEXT_SUFFIXES:
                out.append(path)
    return sorted(set(out))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="slopcheck",
        description="Flag AI-slop tells in Markdown and HTML prose.",
    )
    parser.add_argument("paths", nargs="*", help="files or directories to check")
    parser.add_argument(
        "--changed",
        action="store_true",
        help="check files changed against HEAD, plus untracked ones",
    )
    parser.add_argument(
        "--internal",
        action="store_true",
        help="notes only we will read: drop the presentation rules, keep the thinking ones",
    )
    parser.add_argument("--strict", action="store_true", help="fail on review-tier findings too")
    parser.add_argument(
        "--stats", action="store_true", help="print sentence length and concreteness per file"
    )
    parser.add_argument(
        "--json", action="store_true", dest="as_json", help="machine-readable output"
    )
    parser.add_argument("--list", action="store_true", help="print every rule and exit")
    args = parser.parse_args(argv)

    # A matched em dash prints as a replacement box on a cp1252 console, which
    # makes the one rule we care most about look like a bug in the checker.
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            try:
                reconfigure(encoding="utf-8", errors="replace")
            except (OSError, ValueError):
                pass

    rules = build_rules(internal=args.internal)

    if args.list:
        for rule in sorted(rules, key=lambda r: (r.tier, r.name)):
            print(f"{rule.tier:6}  {rule.name:22}  {rule.fix}")
        return 0

    paths = changed_paths() if args.changed else collect_paths(args.paths)
    if not paths:
        if args.as_json:
            print(json.dumps({"files": 0, "banned": 0, "review": 0, "stats": {}, "findings": []}))
        else:
            print("slopcheck: nothing to check")
        return 0

    findings: list[Finding] = []
    stats: dict[str, dict[str, float]] = {}
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError) as exc:
            print(f"slopcheck: skipped {path}: {exc}", file=sys.stderr)
            continue
        findings.extend(check_text(text, rules, str(path)))
        if args.stats:
            stats[str(path)] = stats_for(text)

    banned = [f for f in findings if f.tier == BANNED]
    review = [f for f in findings if f.tier == REVIEW]

    if args.as_json:
        print(
            json.dumps(
                {
                    "files": len(paths),
                    "banned": len(banned),
                    "review": len(review),
                    "stats": stats,
                    "findings": [f.__dict__ for f in findings],
                },
                indent=2,
            )
        )
    else:
        current = ""
        for finding in findings:
            if finding.path != current:
                current = finding.path
                print("\n" + current)
            print(
                f"  {finding.line}:{finding.col}  {finding.tier:6} {finding.rule:22} {finding.text!r}\n      -> {finding.fix}"
            )
        if args.stats:
            print("")
            for path_name, numbers in stats.items():
                print(
                    "{}: {} words, {} words per sentence, {}% of sentences carry a number".format(
                        path_name,
                        int(numbers["words"]),
                        numbers["mean_sentence_words"],
                        numbers["pct_sentences_with_a_number"],
                    )
                )
        print(f"\n{len(paths)} file(s): {len(banned)} banned, {len(review)} review")

    if banned:
        return 1
    if args.strict and review:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
