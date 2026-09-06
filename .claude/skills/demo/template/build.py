"""
Builds the deployable site into dist/.

Same two jobs as website-v2/build.py, and the second one still matters more:

  1. Copy the files that belong on a web server and nothing else. This folder
     holds README.md, tools/ and build.py, none of which any visitor should be
     able to read. The copy is an allow-list, not an exclude-list, so a new note
     added to this folder is not published by default.

  2. Refuse to build while something that must not ship is still in place.

This client's site adds two checks that v2 does not have, and both exist because
of what the listing audit found:

  * THE CONFIRMED GATE. Every fact on this page came off a Google listing rather
    than out of the client's mouth. js/config.js carries a `confirmed` block and
    this refuses to build the live site while any flag in it is false. A
    homeowner is going to ring that number and expect that warranty.

  * THE HONEST REVIEWS CHECK. js/reviews.js hides one of the five reviews Google
    returns. A page that shows a filtered selection has to say so and has to
    link to the unfiltered listing, so this fails the build if any review is
    hidden and either the Google listing link or the count element is missing
    from index.html.

    python build.py --check            # run the pre-flight only, write nothing
    python build.py                    # pre-flight, then write dist/
    python build.py --allow-warnings   # build a preview with placeholders left

Never use --allow-warnings for the live site.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DIST = ROOT / "dist"

# Allow-list. Anything not named here does not reach the web server.
PAGES = ["index.html", "privacy.html", "404.html", "_headers"]
TREES = ["css", "js", "assets"]

# Files inside an allowed tree that still must not ship.
TREE_EXCLUDE = {".md"}

# Placeholders that block a launch, as (file glob, pattern, why).
#
# Two conventions, both launch-blocking. A raw TODO is a note nobody should ever
# read on a live page. A `chip` is copy that is deliberately standing in for
# something only the client can give us, written so the page can be demonstrated
# on a call without a TODO in the middle of it. Both have to be gone before the
# site is real, and the chip is the one worth watching: it looks finished, which
# is exactly what makes it easy to forget.
BLOCKERS: list[tuple[str, str, str]] = [
    ("*.html", r"TODO", "a visible TODO is still in the page copy"),
    ("*.html", r'class="chip"', "placeholder copy is still on the page, marked with a chip"),
]

# Copy rules the site is held to. Em-dashes are rejected outright: our copy uses
# full stops and commas, and an em-dash is the single clearest tell that a
# paragraph was pasted in from somewhere else rather than written.
COPY_RULES: list[tuple[str, str]] = [
    ("—", "em-dash in the copy. Use a full stop or a comma."),
    ("–", "en-dash in the copy. Use a full stop or a comma."),
    ("Lorem ipsum", "placeholder text left in the page."),
]


def html_text(path: Path) -> str:
    """The visible copy, with comments, scripts and styles taken out.

    A TODO inside an HTML comment is a note to us and is allowed. A TODO the
    visitor can read is not, and that is the whole distinction this makes.
    """
    text = path.read_text(encoding="utf-8")
    text = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    text = re.sub(r"<script\b.*?</script>", "", text, flags=re.DOTALL | re.IGNORECASE)
    text = re.sub(r"<style\b.*?</style>", "", text, flags=re.DOTALL | re.IGNORECASE)
    return text


def preflight() -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []

    # 1. Tokens must match the generator.
    result = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "gen_tokens.py"), "--check"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        errors.append("design tokens have drifted: " + (result.stderr.strip() or "run gen_tokens.py --write"))

    # 2. Copy rules, on the visible text only.
    for page in ROOT.glob("*.html"):
        visible = html_text(page)
        for needle, why in COPY_RULES:
            if needle in visible:
                errors.append(f"{page.name}: {why}")

    # 3. Launch-blocking placeholders. Warnings, because a preview build of an
    #    unfinished page is a legitimate thing to want.
    for glob, pattern, why in BLOCKERS:
        for page in ROOT.glob(glob):
            hits = len(re.findall(pattern, html_text(page)))
            if hits:
                warnings.append(f"{page.name}: {why} ({hits})")

    # 3b. Inline style attributes. The pages ship style-src 'self', so the
    #     browser drops these and the layout they carried silently does not
    #     happen. This shipped once on privacy.html and was only caught by
    #     opening the page, which is exactly the kind of check a build should be
    #     doing instead of a person.
    for page in ROOT.glob("*.html"):
        raw = page.read_text(encoding="utf-8")
        count = len(re.findall(r"<[^>]+\sstyle=", raw))
        if count:
            errors.append(
                f"{page.name}: {count} inline style attribute(s). "
                "The CSP is style-src 'self', so these are dropped. Put them in css/styles.css."
            )

    config = (ROOT / "js" / "config.js").read_text(encoding="utf-8")

    # 4. Config values that are still empty.
    for key in ("email", "formEndpoint"):
        if re.search(rf'{key}:\s*""', config):
            warnings.append(f"js/config.js: {key} is still empty")

    # 5. The confirmed gate. Every false flag is a fact a homeowner will act on
    #    that no human has checked.
    block = re.search(r"confirmed:\s*\{(.*?)\}", config, re.DOTALL)
    if block is None:
        errors.append("js/config.js: the confirmed block is gone. Put it back.")
    else:
        unchecked = re.findall(r"(\w+):\s*false", block.group(1))
        for key in unchecked:
            warnings.append(f"js/config.js: {key} has not been confirmed with the client")

    # 6. The honest reviews check.
    reviews = (ROOT / "js" / "reviews.js").read_text(encoding="utf-8")
    shown = len(re.findall(r"show:\s*true", reviews))
    hidden = len(re.findall(r"show:\s*false", reviews))
    index = (ROOT / "index.html").read_text(encoding="utf-8")

    if shown == 0:
        warnings.append("js/reviews.js: no review is set to show, so the reviews section will be empty")
    if hidden and "googleUrl" not in index:
        errors.append(
            f"index.html: {hidden} review(s) are hidden but the page no longer links to the Google listing. "
            "A filtered selection has to link to the unfiltered source."
        )
    if hidden and 'id="reviews-note"' not in index:
        errors.append(
            "index.html: the reviews-note element is gone, so the page no longer says how many "
            "of the reviews it is showing."
        )

    # 7. Every asset the pages reference must exist.
    #    The lookbehind matters: without it, `href="` also matches inside
    #    `data-config-href="googleUrl"` and the check goes looking for a file
    #    called googleUrl. The fragment has to come off too, or `index.html#book`
    #    is reported missing when index.html is sitting right there.
    for page in ROOT.glob("*.html"):
        raw = page.read_text(encoding="utf-8")
        for ref in re.findall(r'(?<![-\w])(?:src|srcset|href)="([^"]+)"', raw):
            for candidate in re.split(r",\s*", ref):
                rel = candidate.strip().split(" ")[0].split("#")[0]
                if rel == "" or rel.startswith(("http", "data:", "mailto:", "tel:")):
                    continue
                if not (ROOT / rel).exists():
                    errors.append(f"{page.name}: missing asset {rel}")

    return errors, warnings


def build() -> None:
    if DIST.exists():
        shutil.rmtree(DIST)
    DIST.mkdir(parents=True)

    for name in PAGES:
        src = ROOT / name
        if src.exists():
            shutil.copy2(src, DIST / name)

    for tree in TREES:
        src = ROOT / tree
        if not src.exists():
            continue
        for path in src.rglob("*"):
            if not path.is_file() or path.suffix.lower() in TREE_EXCLUDE:
                continue
            target = DIST / path.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)

    total = sum(f.stat().st_size for f in DIST.rglob("*") if f.is_file())
    count = sum(1 for f in DIST.rglob("*") if f.is_file())
    print(f"Wrote {DIST} ({count} files, {total / 1024:.0f}KB).")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="run the pre-flight only")
    parser.add_argument("--allow-warnings", action="store_true", help="build a preview with placeholders left")
    args = parser.parse_args()

    errors, warnings = preflight()

    for w in warnings:
        print(f"  warning: {w}")
    for e in errors:
        print(f"  ERROR:   {e}", file=sys.stderr)

    if errors:
        print("\nBuild refused. Fix the errors above.", file=sys.stderr)
        return 1

    if warnings and not args.allow_warnings and not args.check:
        print(
            "\nBuild refused: launch-blocking placeholders are still in place.\n"
            "Use --allow-warnings to build a preview anyway. Never for the live site.",
            file=sys.stderr,
        )
        return 1

    if args.check:
        print(f"\nPre-flight complete, {len(warnings)} warning(s)." if warnings else "\nPre-flight clean.")
        return 0

    build()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
