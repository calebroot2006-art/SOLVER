"""Fetch only eleven pinned PHH data files; record metadata, never execute content."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import tomllib
from urllib.request import urlopen

REVISION = "e47fbd5816372360bade4de5d712346fe1bb70f6"
NAMES = "00-02-07 00-08-38 00-15-36 00-18-39 02-51-10 02-53-09 02-54-12 02-56-12 02-57-27 03-00-32 03-02-41".split()


def inspect(name):
    url = f"https://raw.githubusercontent.com/uoftcprg/phh-dataset/{REVISION}/data/wsop/2023/43/5/{name}.phh"
    with urlopen(url, timeout=25) as response:
        raw = response.read(65537)
    assert len(raw) <= 65536
    hand = tomllib.loads(raw.decode("utf-8"))
    assert hand["variant"] == "NT"
    assert len(hand["starting_stacks"]) == len(hand["finishing_stacks"]) == 5
    assert all(type(n) is int and n >= 0 for key in ("starting_stacks", "finishing_stacks", "antes", "blinds_or_straddles") for n in hand[key])
    assert hand["ante_trimming_status"] is False
    assert not any("?" in action for action in hand["actions"])
    return {"file": name + ".phh", "url": url, "sha256": hashlib.sha256(raw).hexdigest(), "variant": hand["variant"], "players": len(hand["starting_stacks"]), "ante_trimming_status": hand["ante_trimming_status"], "antes": hand["antes"], "blinds_or_straddles": hand["blinds_or_straddles"], "actions": len(hand["actions"]), "show_actions": [action for action in hand["actions"] if " sm" in action]}


with ThreadPoolExecutor(max_workers=3) as executor:
    inventory = list(executor.map(inspect, NAMES))
result = {"revision": REVISION, "count": len(inventory), "files": inventory}
Path("docs/reviews/2026-09-10-step6-review/phh-inventory.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"verified_NT_fixtures": len(inventory), "all_five_handed": True, "integer_chips_and_known_cards": True, "show_actions": sum(len(item["show_actions"]) for item in inventory)}))
