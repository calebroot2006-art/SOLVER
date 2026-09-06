"""Tests for slopcheck.

Two things have to be true for this checker to be worth running: it must catch
the tells, and it must stay quiet on prose that is already in our voice. The
false-positive tests below matter more than the detection tests, because a
noisy linter gets ignored, and an ignored linter is worse than none.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import slopcheck  # noqa: E402

RULES = slopcheck.build_rules()


def check(text: str) -> list[slopcheck.Finding]:
    return slopcheck.check_text(text, RULES, "sample.md")


def names(text: str) -> set[str]:
    return {f.rule for f in check(text)}


# ---------------------------------------------------------------------------
# It stays quiet on our own voice.
# ---------------------------------------------------------------------------

HOUSE_VOICE = """---
client: acme-co
type: recommendation
status: proposed
date: 2026-08-25
effort: M
payoff: M
---

# Rec: missed-call text-back

Back to hub: [[acme-co]]

## The problem

Calls that come in while the crew is on site go to voicemail. Roughly a third
never call back. On the week of 4 August that was 11 calls and 4 lost jobs.

## The proposed fix

A missed call gets an automatic text within a minute, telling the caller when
someone will ring back. The text goes out from the same number they dialled.

## Estimated effort

`M`. Two days, once we have access to the phone system.

## Expected payoff

Every missed call gets a reply before the customer dials the next company.
"""


def test_house_voice_is_clean():
    assert check(HOUSE_VOICE) == []


def test_website_copy_is_clean():
    copy = (
        "Stop running the office from the cab of your truck. "
        "We find where the hours leak in a trades business, then build the "
        "automations that plug them. Nothing new to learn."
    )
    assert check(copy) == []


# ---------------------------------------------------------------------------
# It catches the vocabulary.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,rule",
    [
        ("We will delve into the intake process.", "delve"),
        ("We leverage the existing phone system.", "leverage"),
        ("The team can utilize the same tools.", "utilize"),
        ("A seamless handoff between the office and the crew.", "seamless"),
        ("This will streamline your quoting.", "streamline"),
        ("Unlock the potential of your scheduling data.", "unlock"),
        ("Our platform empowers owners.", "empower"),
        ("A myriad of small delays.", "myriad"),
        ("Cutting-edge tooling for dispatch.", "cutting-edge"),
        ("This is a game-changer for the office.", "game-changer"),
        ("A holistic view of the business.", "holistic"),
        ("Let us embark on the discovery phase.", "embark"),
        ("A meticulously prepared report.", "meticulous"),
        ("In today's fast-paced world, owners are busy.", "ever-evolving"),
        ("When it comes to invoicing, the delay is real.", "when-it-comes-to"),
        ("It's worth noting that the crew starts at seven.", "worth-noting"),
        ("Tailored solutions for trades businesses.", "solutions"),
        ("This will drive results for the shop.", "drive-results"),
        ("We map the customer journey end to end.", "journey"),
        ("The competitive landscape has shifted.", "landscape"),
        ("Studies show that follow-up matters.", "unsourced-authority"),
        ("Hassle-free onboarding for the crew.", "hassle-free"),
        ("This underscores the size of the leak.", "underscore"),
        ("As an AI language model, I should note this.", "as-an-ai"),
    ],
)
def test_banned_vocabulary(text: str, rule: str):
    found = check(text)
    assert rule in {f.rule for f in found}, [f.rule for f in found]
    assert all(f.tier == slopcheck.BANNED for f in found if f.rule == rule)


@pytest.mark.parametrize(
    "text,rule",
    [
        ("The script is robust.", "robust"),
        ("This significantly reduces the delay.", "vague-magnitude"),
        ("There are various ways to do this.", "vague-count"),
        ("In order to send the invoice, close the job.", "in-order-to"),
        ("We should circle back next week.", "business-speak"),
    ],
)
def test_review_vocabulary(text: str, rule: str):
    found = [f for f in check(text) if f.rule == rule]
    assert found, [f.rule for f in check(text)]
    assert found[0].tier == slopcheck.REVIEW


# ---------------------------------------------------------------------------
# It catches the constructions.
# ---------------------------------------------------------------------------


def test_em_dash():
    assert "em-dash" in names("The crew starts at seven — the office does not.")


def test_en_dash_and_entity():
    assert "em-dash" in names("Seven – eight in the morning.")
    assert "em-dash" in names("<p>Seven &mdash; eight.</p>")


def test_antithesis_snap():
    assert "antithesis-snap" in names("This is not just software, it's a way of working.")


def test_isnt_about():
    assert "isnt-about" in names("This isn't about software. It's about hours.")


def test_paragraph_connective():
    assert "paragraph-connective" in names("Moreover, the crew is already busy.")
    assert "paragraph-connective" in names("In conclusion, the process leaks.")


def test_connective_only_at_line_start():
    # "ultimately" mid-sentence is ordinary English and should not fire.
    assert "paragraph-connective" not in names("The invoice ultimately went out on Friday.")


def test_hook_question():
    assert "hook-question" in names("Ever wondered where the hours go?")


def test_hype_signoff():
    assert "hype-signoff" in names("You're well on your way to a tidier office.")


def test_reveal_tease():
    assert "reveal-tease" in names("Here's the thing: nobody chases the invoices.")


def test_heading_question_is_review():
    found = [f for f in check("## Why does this matter?\n") if f.rule == "heading-question"]
    assert found and found[0].tier == slopcheck.REVIEW


def test_heading_emoji():
    assert "heading-emoji" in names("## \U0001f680 Getting started\n")


def test_hedge_stack():
    text = "This might possibly be the sort of change that could typically help."
    assert "hedge-stack" in names(text)


def test_two_hedges_are_allowed():
    assert "hedge-stack" not in names("This might typically take a week.")


def test_long_sentence():
    sentence = "The " + " ".join(["invoice"] * 45) + " went out."
    assert "long-sentence" in names(sentence)


# ---------------------------------------------------------------------------
# It ignores everything that is not prose.
# ---------------------------------------------------------------------------


def test_fenced_code_is_ignored():
    text = "Run it.\n\n```python\ndef leverage_utilize():\n    return 'seamless'\n```\n"
    assert check(text) == []


def test_tilde_fence_is_ignored():
    text = "Run it.\n\n~~~\nleverage utilize seamless\n~~~\n"
    assert check(text) == []


def test_inline_code_is_ignored():
    assert check("Call `df.transform()` and `leverage()` from the shell.") == []


def test_frontmatter_is_ignored():
    text = "---\ntitle: leverage seamless synergy\n---\n\nThe crew starts at seven.\n"
    assert check(text) == []


def test_html_tags_and_attributes():
    # The tag itself is markup, but alt text is prose a visitor reads.
    assert check('<img src="a.png" class="leverage-grid">') == []
    assert "seamless" in names('<img src="a.png" alt="A seamless handoff">')


def test_script_body_is_ignored():
    assert check("<script>const leverage = 1; // seamless\n</script>") == []


def test_urls_and_link_targets_are_ignored():
    assert check("See [the note](https://example.com/delve-into-things).") == []
    assert check("Raw: https://example.com/seamless-synergy") == []


def test_wikilink_targets_are_ignored():
    assert check("Traced to [[2026-08-04-leverage-interview]].") == []


def test_html_comment_is_ignored():
    assert check("<!-- leverage this later -->\nThe crew starts at seven.") == []


# ---------------------------------------------------------------------------
# Positions, directives and exit codes.
# ---------------------------------------------------------------------------


def test_line_and_column_survive_stripping():
    text = "---\na: b\n---\n\n```\nleverage\n```\n\nWe leverage it.\n"
    found = [f for f in check(text) if f.rule == "leverage"]
    assert len(found) == 1
    assert (found[0].line, found[0].col) == (9, 4)
    assert text.splitlines()[8][3:11] == "leverage"


def test_off_directive_skips_the_file():
    assert check("<!-- slopcheck: off -->\nWe leverage a myriad of synergies.") == []


def test_allow_directive_skips_named_rules_only():
    text = "<!-- slopcheck: allow leverage -->\nWe leverage a myriad of things."
    found = names(text)
    assert "leverage" not in found
    assert "myriad" in found


def test_exit_code_zero_on_clean(tmp_path: Path, capsys):
    path = tmp_path / "clean.md"
    path.write_text(HOUSE_VOICE, encoding="utf-8")
    assert slopcheck.main([str(path)]) == 0


def test_exit_code_one_on_banned(tmp_path: Path, capsys):
    path = tmp_path / "slop.md"
    path.write_text("We leverage synergies.\n", encoding="utf-8")
    assert slopcheck.main([str(path)]) == 1
    assert "leverage" in capsys.readouterr().out


def test_review_only_fails_under_strict(tmp_path: Path, capsys):
    path = tmp_path / "review.md"
    path.write_text("The script is robust.\n", encoding="utf-8")
    assert slopcheck.main([str(path)]) == 0
    assert slopcheck.main([str(path), "--strict"]) == 1


def test_directory_walk_skips_non_text(tmp_path: Path, capsys):
    (tmp_path / "a.md").write_text("We leverage it.\n", encoding="utf-8")
    (tmp_path / "b.py").write_text("leverage = 1\n", encoding="utf-8")
    assert slopcheck.main([str(tmp_path)]) == 1
    out = capsys.readouterr().out
    assert "a.md" in out
    assert "b.py" not in out


def test_directory_walk_skips_cache_dirs(tmp_path: Path, capsys):
    cache = tmp_path / ".pytest_cache"
    cache.mkdir()
    (cache / "README.md").write_text("We leverage synergies.\n", encoding="utf-8")
    (tmp_path / "ours.md").write_text("The crew starts at seven.\n", encoding="utf-8")
    assert slopcheck.main([str(tmp_path)]) == 0
    assert "leverage" not in capsys.readouterr().out


def test_json_output_is_parseable(tmp_path: Path, capsys):
    import json

    path = tmp_path / "slop.md"
    path.write_text("We leverage synergies.\n", encoding="utf-8")
    slopcheck.main([str(path), "--json"])
    payload = json.loads(capsys.readouterr().out)
    assert payload["banned"] >= 2
    assert {f["rule"] for f in payload["findings"]} >= {"leverage", "synergy"}


def test_stats_counts_numbers(capsys):
    numbers = slopcheck.stats_for("Eleven calls. Four lost jobs in 1 week.")
    assert numbers["sentences"] == 2
    assert numbers["pct_sentences_with_a_number"] == 50.0


def test_list_flag(capsys):
    assert slopcheck.main(["--list"]) == 0
    out = capsys.readouterr().out
    assert "leverage" in out
    assert "em-dash" in out


def test_missing_path_is_reported_not_crashed(tmp_path: Path, capsys):
    assert slopcheck.main([str(tmp_path / "nope.md")]) == 0
    assert "no such file" in capsys.readouterr().err


# ---------------------------------------------------------------------------
# Internal notes drop the presentation rules and keep the thinking ones.
# ---------------------------------------------------------------------------


def test_internal_drops_em_dash():
    internal = slopcheck.build_rules(internal=True)
    text = "The crew starts at seven — the office does not."
    assert "em-dash" not in {f.rule for f in slopcheck.check_text(text, internal, "notes.md")}


def test_internal_keeps_the_vocabulary():
    internal = slopcheck.build_rules(internal=True)
    text = "We leverage a myriad of synergies."
    found = {f.rule for f in slopcheck.check_text(text, internal, "notes.md")}
    assert {"leverage", "myriad", "synergy"} <= found


def test_internal_drops_heading_rules():
    internal = slopcheck.build_rules(internal=True)
    found = {f.rule for f in slopcheck.check_text("## Why?\n", internal, "notes.md")}
    assert "heading-question" not in found


def test_internal_flag_changes_exit_code(tmp_path: Path, capsys):
    path = tmp_path / "notes.md"
    path.write_text("The crew starts at seven — the office does not.\n", encoding="utf-8")
    assert slopcheck.main([str(path)]) == 1
    assert slopcheck.main([str(path), "--internal"]) == 0


def test_claude_md_passes_as_an_internal_note():
    """Our own house docs use em dashes freely, and that is not slop."""
    repo_doc = Path(__file__).resolve().parents[4] / "CLAUDE.md"
    if not repo_doc.exists():  # the skill can be copied out of the repo
        pytest.skip("CLAUDE.md not next to the skill")
    internal = slopcheck.build_rules(internal=True)
    findings = slopcheck.check_text(repo_doc.read_text(encoding="utf-8"), internal, str(repo_doc))
    banned = [f for f in findings if f.tier == slopcheck.BANNED]
    assert banned == [], banned
