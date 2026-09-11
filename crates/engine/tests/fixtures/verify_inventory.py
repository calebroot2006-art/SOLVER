"""Check the pinned local PHH corpus as data; this is not an engine replay."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

REVISION = "e47fbd5816372360bade4de5d712346fe1bb70f6"
LICENSE_SHA256 = "fd8a94019f50ff8cd84e674acd790427882a99b32435c85e43b8e1d5b37626d1"
INVENTORY_SHA256 = "ff16afed7d58f962a2c193cfeb4b0e3affab40df44b0404d5cde404144f48a8c"
NAMES = (
    "00-02-07", "00-08-38", "00-15-36", "00-18-39", "02-51-10",
    "02-53-09", "02-54-12", "02-56-12", "02-57-27", "03-00-32", "03-02-41",
)
HAND_NUMBERS = (1, 2, 3, 4, 62, 63, 64, 65, 66, 67, 68)
DATA_PATH = Path("wsop/2023/43/5")
MAX_DATA_BYTES = 65_536


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def bounded_read(root, relative, limit):
    path = (root / relative).resolve()
    require(path.is_relative_to(root), f"path leaves fixture directory: {relative}")
    with path.open("rb") as stream:
        raw = stream.read(limit + 1)
    require(len(raw) <= limit, f"{relative}: exceeds {limit} bytes")
    return raw


def card_ids(text):
    require(bool(re.fullmatch(r"(?:[2-9TJQKA][cdhs])+", text)), "unknown/invalid cards")
    result = [text[index:index + 2] for index in range(0, len(text), 2)]
    require(len(result) == len(set(result)), "duplicate cards in action")
    return result


def player_id(text):
    require(bool(re.fullmatch(r"p[1-5]", text)), "invalid positional player")
    return int(text[1:])


def inspect_hand(raw, entry, expected_hand):
    hand = tomllib.loads(raw.decode("utf-8"))
    require(hand["variant"] == entry["variant"] == "NT", "variant differs")
    require(hand["ante_trimming_status"] is False, "ante trimming differs")
    require(entry["ante_trimming_status"] is False, "inventory ante trimming differs")
    require(hand["hand"] == expected_hand, "published hand number differs")
    require(entry["players"] == len(hand["players"]) == 5, "seat count differs")
    for field in ("starting_stacks", "finishing_stacks", "antes", "blinds_or_straddles"):
        values = hand[field]
        require(type(values) is list and len(values) == 5, f"{field}: requires five values")
        require(all(type(value) is int and value >= 0 for value in values),
                f"{field}: requires complete nonnegative integers")
    require(all(value > 0 for value in hand["starting_stacks"]), "nonpositive starting stack")
    require(sum(hand["starting_stacks"]) == sum(hand["finishing_stacks"]),
            "recorded chip totals differ")
    for field in ("antes", "blinds_or_straddles"):
        require(hand[field] == entry[field], f"{field}: differs from inventory")
    bb = hand["blinds_or_straddles"][1]
    require(type(hand["min_bet"]) is int and hand["min_bet"] == bb > 0, "minimum bet differs")
    require(hand["antes"][1] * 2 == bb * 3, "big-blind ante is not 1.5 big blinds")
    actions = hand["actions"]
    require(type(actions) is list and len(actions) == entry["actions"] <= 256,
            "action count differs")
    require(all(type(action) is str and len(action.encode("utf-8")) <= 256
                for action in actions), "invalid/oversized action string")
    holes, dealt, board, shows = {}, set(), [], []
    early_shows = 0
    for index, action in enumerate(actions):
        tokens = action.split()
        require(bool(tokens), f"action {index}: empty")
        if tokens[:2] == ["d", "dh"]:
            require(len(tokens) == 4, f"action {index}: malformed private deal")
            seat = player_id(tokens[2])
            cards = card_ids(tokens[3])
            require(len(cards) == 2 and seat not in holes, f"action {index}: duplicate/bad hole deal")
            require(not dealt.intersection(cards), f"action {index}: cards already dealt")
            holes[seat] = set(cards)
            dealt.update(cards)
        elif tokens[:2] == ["d", "db"]:
            require(len(tokens) == 3, f"action {index}: malformed board deal")
            cards = card_ids(tokens[2])
            require(len(cards) == (3 if not board else 1) and len(board) + len(cards) <= 5,
                    f"action {index}: board length differs")
            require(not dealt.intersection(cards), f"action {index}: cards already dealt")
            board.extend(cards)
            dealt.update(cards)
        else:
            require(len(tokens) >= 2, f"action {index}: malformed player action")
            seat = player_id(tokens[0])
            kind = tokens[1]
            if kind in ("f", "cc"):
                require(len(tokens) == 2, f"action {index}: malformed fold/check/call")
            elif kind == "cbr":
                require(len(tokens) == 3 and bool(re.fullmatch(r"[1-9][0-9]*", tokens[2])),
                        f"action {index}: raise-to amount is not a positive integer")
            elif kind == "sm":
                require(len(tokens) in (2, 3), f"action {index}: malformed show/muck")
                if len(tokens) == 3:
                    require(set(card_ids(tokens[2])) == holes.get(seat),
                            f"action {index}: revealed cards differ from private deal")
                    early_shows += int(len(board) < 5)
                shows.append(action)
            else:
                raise ValueError(f"action {index}: unsupported inventory token {kind!r}")
    require(set(holes) == set(range(1, 6)), "private deals do not cover five seats")
    require(shows == entry["show_actions"], "show actions differ from inventory")
    return {
        "file": entry["file"], "hand": hand["hand"], "bytes": len(raw),
        "actions": len(actions), "board_cards": len(board),
        "show_actions": shows, "shows_before_board_complete": early_shows,
    }


def verify(root):
    root = root.resolve()
    license_raw = bounded_read(root, Path("LICENSE"), 16_384)
    require(hashlib.sha256(license_raw).hexdigest() == LICENSE_SHA256, "license checksum mismatch")
    raw = bounded_read(root, Path("inventory.json"), MAX_DATA_BYTES)
    require(hashlib.sha256(raw).hexdigest() == INVENTORY_SHA256, "inventory checksum mismatch")
    inventory = json.loads(raw)
    require(inventory["revision"] == REVISION and inventory["count"] == 11
            and len(inventory["files"]) == 11, "inventory revision/count differs")
    expected = {DATA_PATH / (name + ".phh") for name in NAMES}
    found = set()
    for path in root.rglob("*.phh"):
        require(len(found) < 11, "unexpected PHH file count")
        require(path.resolve().is_relative_to(root), "PHH path leaves fixture directory")
        found.add(path.relative_to(root))
    require(found == expected, "PHH file names or count differ")
    prefix = f"https://raw.githubusercontent.com/uoftcprg/phh-dataset/{REVISION}/data/wsop/2023/43/5/"
    rows = []
    for index, entry in enumerate(inventory["files"]):
        require(entry["file"] == NAMES[index] + ".phh", "inventory order differs")
        require(entry["url"] == prefix + entry["file"], "unpinned source URL")
        raw = bounded_read(root, DATA_PATH / entry["file"], MAX_DATA_BYTES)
        require(hashlib.sha256(raw).hexdigest() == entry["sha256"],
                f"{entry['file']}: fixture checksum mismatch")
        try:
            rows.append(inspect_hand(raw, entry, HAND_NUMBERS[index]))
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError(f"{entry['file']}: {error}") from error
    return {
        "revision": REVISION, "fixtures": len(rows),
        "fixture_bytes": sum(row["bytes"] for row in rows),
        "actions": sum(row["actions"] for row in rows),
        "show_actions": sum(len(row["show_actions"]) for row in rows),
        "shows_before_board_complete": sum(row["shows_before_board_complete"] for row in rows),
        "all_five_handed_nt_known_cards_integer_stacks": True,
        "engine_replay_performed": False, "files": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent,
                        help="fixture directory (default: this script's directory)")
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.root), indent=2))
    except (OSError, KeyError, TypeError, ValueError) as error:
        print(f"fixture inventory failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
