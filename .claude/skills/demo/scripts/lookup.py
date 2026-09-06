"""
Pull everything a demo site needs out of a business's Google Business Profile.

    python lookup.py "Pro Roofing Services Katy"
    python lookup.py "Alpha HVAC" --near "Cypress, TX"
    python lookup.py "Alpha HVAC" --place-id ChIJ...      # skip the search
    python lookup.py "Alpha HVAC" --pick 2                # choose from the search list

Writes an evidence bundle to the client's consulting folder:

    clients/<slug>/consulting/google-place-record-<date>.json
    clients/<slug>/consulting/photos/NN.jpg
    clients/<slug>/consulting/photos/00-contact-sheet.jpg

and prints the triage that decides what the website is allowed to say. Read the
triage. Do not skip to the scaffold: the photo and trading-name findings are the
whole reason this step is separate.

The key is GOOGLE_PLACES_API_KEY in tools/leadgen/.env. Billable. See SECRETS.md.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
ENV = REPO / "tools" / "leadgen" / ".env"
BASE = "https://places.googleapis.com/v1"

DETAIL_FIELDS = ",".join(
    [
        "id", "displayName", "formattedAddress", "shortFormattedAddress", "addressComponents",
        "location", "nationalPhoneNumber", "internationalPhoneNumber", "websiteUri",
        "rating", "userRatingCount", "googleMapsUri", "businessStatus",
        "primaryTypeDisplayName", "types", "regularOpeningHours", "photos", "reviews",
        "editorialSummary",
    ]
)

SEARCH_FIELDS = ",".join(
    [
        "places.id", "places.displayName", "places.formattedAddress",
        "places.nationalPhoneNumber", "places.websiteUri", "places.rating",
        "places.userRatingCount", "places.businessStatus", "places.primaryTypeDisplayName",
    ]
)

# Google's category -> schema.org type, for the JSON-LD block.
SCHEMA_TYPES = {
    "roofing_contractor": "RoofingContractor",
    "plumber": "Plumber",
    "electrician": "Electrician",
    "hvac_contractor": "HVACBusiness",
    "general_contractor": "GeneralContractor",
    "painter": "HousePainter",
    "locksmith": "Locksmith",
    "moving_company": "MovingCompany",
}


def api_key() -> str:
    if not ENV.exists():
        sys.exit(f"No {ENV}. Copy tools/leadgen/.env.example to .env and add the key.")
    for line in ENV.read_text(encoding="utf-8").splitlines():
        if line.strip().startswith("GOOGLE_PLACES_API_KEY="):
            key = line.split("=", 1)[1].strip()
            if key:
                return key
    sys.exit(f"GOOGLE_PLACES_API_KEY is empty in {ENV}. See SECRETS.md.")


def post(url: str, body: dict, key: str, mask: str) -> dict:
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "X-Goog-Api-Key": key, "X-Goog-FieldMask": mask},
    )
    with urllib.request.urlopen(req, timeout=45) as r:
        return json.load(r)


def get(url: str, key: str, mask: str) -> dict:
    req = urllib.request.Request(url, headers={"X-Goog-Api-Key": key, "X-Goog-FieldMask": mask})
    with urllib.request.urlopen(req, timeout=45) as r:
        return json.load(r)


def slugify(name: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    return re.sub(r"-{2,}", "-", s)


def search(name: str, near: str | None, key: str) -> list[dict]:
    body: dict = {"textQuery": f"{name} {near}" if near else name, "maxResultCount": 10}
    return post(f"{BASE}/places:searchText", body, key, SEARCH_FIELDS).get("places", [])


def fetch_photos(place: dict, out: Path, key: str) -> list[dict]:
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for i, ph in enumerate(place.get("photos", []), 1):
        author = (ph.get("authorAttributions") or [{}])[0].get("displayName", "unknown")
        width = min(ph.get("widthPx", 1600), 4000)
        url = f"{BASE}/{ph['name']}/media?maxWidthPx={width}&skipHttpRedirect=true&key={key}"
        try:
            uri = json.load(urllib.request.urlopen(url, timeout=45))["photoUri"]
            path = out / f"{i:02d}.jpg"
            urllib.request.urlretrieve(uri, path)
            rows.append({
                "n": i, "file": path.name, "author": author,
                "w": ph.get("widthPx"), "h": ph.get("heightPx"),
                "by_business": author.strip().lower() == place["displayName"]["text"].strip().lower(),
                "bytes": path.stat().st_size,
            })
        except Exception as exc:  # noqa: BLE001
            rows.append({"n": i, "author": author, "error": str(exc)[:80]})
    return rows


def contact_sheet(photos: list[dict], folder: Path) -> Path | None:
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        print("  (Pillow not installed, skipping the contact sheet)")
        return None
    good = [p for p in photos if "error" not in p]
    if not good:
        return None
    cw, ch, cols = 520, 390, 4
    rows = (len(good) + cols - 1) // cols
    sheet = Image.new("RGB", (cw * cols, ch * rows), (20, 20, 20))
    draw = ImageDraw.Draw(sheet)
    for i, meta in enumerate(good):
        im = Image.open(folder / meta["file"])
        im.thumbnail((cw - 8, ch - 30))
        x, y = (i % cols) * cw + 4, (i // cols) * ch + 4
        sheet.paste(im, (x, y))
        tag = "BUSINESS" if meta["by_business"] else "customer"
        draw.text((x + 4, y + ch - 26), f"{meta['n']}. {tag}: {meta['author']}  {meta['w']}x{meta['h']}",
                  fill=(255, 220, 160))
    path = folder / "00-contact-sheet.jpg"
    sheet.save(path, quality=88)
    return path


def triage(place: dict, photos: list[dict]) -> None:
    """Print the findings that decide what the site may claim."""
    name = place["displayName"]["text"]
    print("\n" + "=" * 68)
    print("TRIAGE. Read this before scaffolding.")
    print("=" * 68)

    by_biz = [p for p in photos if p.get("by_business")]
    by_cust = [p for p in photos if "error" not in p and not p.get("by_business")]
    print(f"\nPHOTOS: {len(photos)} total, {len(by_biz)} uploaded by the business, {len(by_cust)} by customers.")
    print("  Open 00-contact-sheet.jpg and judge every business upload for stock.")
    print("  Stock tells: a model facing the lens, a thumbs up, lens flare, a building")
    print("  or landscape that is not local, crews in clothing nobody wears in this state.")
    print("  Customer photos are that customer's copyright, not the client's.")

    reviews = place.get("reviews", [])
    print(f"\nREVIEWS: Google returned {len(reviews)} (its cap is 5) of {place.get('userRatingCount', '?')} total.")
    today = dt.date.today()
    names_seen: set[str] = set()
    for r in reviews:
        when = (r.get("publishTime") or "")[:10]
        age = ""
        if when:
            try:
                age = f"{(today - dt.date.fromisoformat(when)).days // 365}y old"
            except ValueError:
                pass
        text = (r.get("text") or {}).get("text", "")
        flag = "  <-- LOW" if r.get("rating", 5) <= 3 else ""
        print(f"  {r.get('rating')}/5  {when}  {age:>8s}  {r['authorAttribution']['displayName']}{flag}")
        # Other trading names hiding inside the review text.
        for m in re.finditer(r"\b([A-Z][\w&'-]*(?:\s+[A-Z][\w&'-]*){0,3}\s+(?:Roofing|Services|Construction|Contracting|HVAC|Plumbing|Electric|Air|Lawn))\b", text):
            cand = m.group(1).strip()
            if cand.lower() not in name.lower():
                names_seen.add(cand)
    if reviews:
        newest = max((r.get("publishTime", "")[:10] for r in reviews), default="")
        print(f"  Newest of these: {newest}. Old reviews are a finding, and the pitch for the review automation.")
    if names_seen:
        print("\n  OTHER TRADING NAMES found inside review text:")
        for c in sorted(names_seen):
            print(f"    - {c}")
        print("  Ask the client whether these were them before quoting those reviews.")

    print(f"\nWEBSITE ON FILE: {place.get('websiteUri') or 'none. This is why they need one.'}")
    addr = place.get("formattedAddress", "")
    print(f"ADDRESS: {addr}")
    print("  If that is a house on a residential street, treat it as a service-area")
    print("  business and leave showAddress false until the client says otherwise.")
    print("=" * 68)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("name", help="business name as the client says it")
    ap.add_argument("--near", help="city or state to bias the search")
    ap.add_argument("--place-id", help="skip the search and use this place id")
    ap.add_argument("--pick", type=int, help="1-based choice from the search results")
    ap.add_argument("--slug", help="client folder name (default: slugified business name)")
    args = ap.parse_args()

    key = api_key()

    if args.place_id:
        place_id = args.place_id
    else:
        hits = search(args.name, args.near, key)
        if not hits:
            sys.exit(f'No Google listing found for "{args.name}". Try --near "City, ST".')
        if len(hits) > 1 and not args.pick:
            print(f'{len(hits)} matches for "{args.name}":\n')
            for i, h in enumerate(hits, 1):
                print(f"  {i}. {h.get('displayName', {}).get('text')}")
                print(f"     {h.get('formattedAddress')}")
                print(f"     {h.get('nationalPhoneNumber') or 'no phone'} | "
                      f"{h.get('rating', '?')} from {h.get('userRatingCount', '?')} | "
                      f"{h.get('websiteUri') or 'NO WEBSITE'}")
            print("\nRe-run with --pick N to choose.")
            return 2
        place_id = hits[(args.pick or 1) - 1]["id"]

    place = get(f"{BASE}/places/{place_id}?languageCode=en", key, DETAIL_FIELDS)
    if "error" in place:
        sys.exit(f"Places error {place['error'].get('status')}: {place['error'].get('message', '')[:200]}")

    name = place["displayName"]["text"]
    slug = args.slug or slugify(name)
    folder = REPO / "clients" / slug / "consulting"
    folder.mkdir(parents=True, exist_ok=True)
    stamp = dt.date.today().isoformat()

    record = folder / f"google-place-record-{stamp}.json"
    record.write_text(json.dumps(place, indent=2, ensure_ascii=False), encoding="utf-8")

    photos = fetch_photos(place, folder / "photos", key)
    sheet = contact_sheet(photos, folder / "photos")

    types = place.get("types", [])
    schema = next((SCHEMA_TYPES[t] for t in types if t in SCHEMA_TYPES), "LocalBusiness")

    print(f"\n{name}")
    print(f"  slug     : {slug}")
    print(f"  place id : {place['id']}")
    print(f"  address  : {place.get('formattedAddress')}")
    print(f"  phone    : {place.get('nationalPhoneNumber') or 'none'}")
    print(f"  rating   : {place.get('rating')} from {place.get('userRatingCount')}")
    print(f"  category : {place.get('primaryTypeDisplayName', {}).get('text')}  -> schema.org/{schema}")
    print(f"  hours    : {'; '.join(place.get('regularOpeningHours', {}).get('weekdayDescriptions', [])[:1]) or 'not published'}")
    print(f"\n  record   : {record.relative_to(REPO)}")
    print(f"  photos   : {len([p for p in photos if 'error' not in p])} saved to {(folder / 'photos').relative_to(REPO)}")
    if sheet:
        print(f"  sheet    : {sheet.relative_to(REPO)}")

    triage(place, photos)

    print(f"\nNEXT: read the contact sheet, then\n"
          f"  python .claude/skills/demo/scripts/scaffold.py {slug}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
