---
name: leadgen
description: Build, refresh, and explain cold-call lead lists for local trades verticals (HVAC, roofing, lawn care, car detailing, and any vertical in verticals.toml). Use when the user asks to build a call list, find leads, find businesses with a bad or missing website, refresh leads, export a call sheet, put leads in a Google Sheet, or asks why a business is ranked where it is. Runs the leadgen CLI in tools/leadgen.
---

# leadgen — call-list runner

The pipeline lives in `tools/leadgen` (Python, uv-managed). This skill translates requests
like "build me a call list for roofers in Greenville" or "find detailers with no website"
into pipeline runs and presents the results conversationally.

## Invocation

All commands run from `tools/leadgen`:

```
uv run leadgen <command> ...
```

If `uv` is not on PATH, use `$env:LOCALAPPDATA\Microsoft\WinGet\Packages\astral-sh.uv_Microsoft.Winget.Source_8wekyb3d8bbwe\uv.exe`.

If the command fails with `An Application Control policy has blocked this file (os error
4551)`, Windows is refusing the unsigned `leadgen.exe` shim. Run the same thing as
`uv run python -m leadgen <command> ...` (this is the case on Caleb's machine).

## "Build me a call list for <vertical> in <geo>"

1. Map the vertical to a key in `config/verticals.toml` (`roofers` -> `roofing`,
   `HVAC companies` -> `hvac`, `lawn guys` -> `lawn_care`, `detailers` / `car detailing` /
   `ceramic coating shops` -> `car_detailing`). If no key fits, ask whether to add a new
   vertical block — a config edit, not code.
2. Settle the geo before spending. If the user says "the area" or "around here" and no
   geo is on record (config `[geo].home_label` is still `SET-ME`), ask. A metro is several
   `enumerate` runs, one per suburb, into the same database.
3. Run in order, surfacing any spend-confirmation prompt to the user verbatim:
   - `uv run leadgen enumerate <vertical> "<geo>"` (once per geo label)
   - `uv run leadgen webaudit` (free; skip for `hooks` verticals if in a hurry)
   - `uv run leadgen enrich --vertical <vertical> --geo "<geo>"`
   - `uv run leadgen score`
   - `uv run leadgen export`
4. Read `exports/callsheet-latest.csv` and summarize the top 20 conversationally:
   rank, name, phone, owner (if sourced), the hook sentence, hook date, tier. For a
   `web_presence` vertical the hook is the website status and the reason.
5. Mention counts for: the unverified-size bucket (worth eyeballing), and how many were
   disqualified (viewable via `leadgen export --disqualified`).

## "Find <vertical> with a bad website or none at all"

That is the `web_presence` ranking, which `car_detailing` uses by default. For another
vertical, set `lead_signal = "web_presence"` in its block (or ask before changing a
vertical that has been run as `hooks`). Then the flow above. The website-status
histogram `webaudit` prints is the first thing to report back: how many have no site,
a Facebook page only, a dead site, a weak one.

## Owner names and employee counts without a Perplexity key

`enrich` skips the Perplexity lookups when `PERPLEXITY_API_KEY` is empty (it says so).
Caleb's Perplexity account is connected through Composio, so run the pass from the
session instead:

1. Read `exports/callsheet-latest.csv` (and the unverified-size sheet) for the active
   records: name, address, website, phone, place_id.
2. For each, call `PERPLEXITYAI_CREATE_CHAT_COMPLETION` (model `sonar`) with the prompt in
   `src/leadgen/perplexity.py` (`PROMPT`) and `response_format` json_schema matching its
   JSON shape. Batch up to 50 per `COMPOSIO_MULTI_EXECUTE_TOOL` call.
3. Write the answers to a JSON list of `{"place_id", "kind", "claim", "source_url"}` and
   run `uv run python -m leadgen import-claims <file>`. Only keep claims that carry a
   source URL from `data.citations`; drop anything the model asserts without one, or
   import it with `source_url: null` so it shows as unsourced.
4. Re-run `export`.

## "Put it in a Google Sheet"

`leadgen export` writes `exports/sheet-latest.json`: `{title, tabs: [{name, headers,
rows}]}` with rectangular rows of plain strings and numbers. Push it through the Composio
Google Sheets connector (Caleb's Google account is connected; check with
`COMPOSIO_SEARCH_TOOLS` first):

1. `GOOGLESHEETS_SEARCH_SPREADSHEETS` by the workbook title's prefix (e.g. "Car detailing
   leads"). If one exists for the same vertical and area, update it in place so the
   partners' link and their "Called on" notes survive: read the current "Called on" and
   "Outcome / notes" columns back with `GOOGLESHEETS_BATCH_GET`, then rewrite the data
   rows and re-apply the notes by matching on the Google Maps link (stable per listing).
   Otherwise `GOOGLESHEETS_CREATE_GOOGLE_SHEET1` with the title.
2. `GOOGLESHEETS_GET_SHEET_NAMES` — rename the first tab to the first tab name and add the
   others with `GOOGLESHEETS_UPDATE_SHEET_PROPERTIES` / `GOOGLESHEETS_ADD_SHEET`.
3. Write each tab with `GOOGLESHEETS_VALUES_UPDATE` (`USER_ENTERED`, range
   `'<tab>'!A1`, headers first). Chunk at ~200 rows per call; the connector rate limit
   is 60 writes a minute.
4. Format: header row bold with `GOOGLESHEETS_FORMAT_CELL`, freeze the header row via
   `GOOGLESHEETS_UPDATE_SHEET_PROPERTIES` (`gridProperties.frozenRowCount = 1`; include
   the existing `rowCount`/`columnCount`), auto-size columns with
   `GOOGLESHEETS_AUTO_RESIZE_DIMENSIONS`.
5. Verify by reading the first rows back (`GOOGLESHEETS_BATCH_GET`) and comparing with
   the JSON before handing over the link.

Report the sheet URL, the tab names, the row count, and the website-status counts.

## "Why is <business> ranked high/low?"

Run `uv run leadgen why "<name or place_id>"` and translate the breakdown: which tier
fired and why, the website findings (each with its points), the evidence behind headcount
and AI-adoption, and the flagged review text verbatim when there is one.

## "Refresh my leads"

Run `uv run leadgen refresh`, then report: how many re-enriched, how many disqualified
records got re-queued (reactivated), whether any new tier-1 hooks appeared, and for a
`web_presence` vertical whether any "broken" sites came back up.

## Hard rules

- Never present a record whose hook lacks a source URL — if one appears, flag it as a bug.
- Never mark model-asserted facts (owner names, employee counts, news) as verified; say
  "per <source URL>". An employee count is an estimate with named evidence, never a fact.
- The phone number is the Google listing's number. Say so; never call it the owner's cell
  as a fact.
- Surface spend-confirmation prompts to the user; never auto-confirm past the budget gate.
- Missing API keys: point to `.env.example` in `tools/leadgen` (keys never live in code).
- `ai_status: unclear` records carry "verify by phone" — remind the user to check those
  manually before dialing properly.
