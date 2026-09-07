"""Build and run a pinned external WASM reference on turn trees; never import our solver.

Same pinned revisions, toolchain and wrapper hashes as the river capture
(`tests/reference/river/capture.py`). What differs is the input contract (a four-card board
and per-street, per-player bet menus), the exported node schema (chance nodes, runout-scoped
export) and the output validation. Nothing here imports anything from `crates/`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import tomllib

WASM_REVISION = "97360db7644329b1c23a7adf06e9aa59406e4d4b"
ENGINE_REVISION = "9d1509fe5077d019825f833eed04b16d342dfda1"
TOOLCHAIN = "nightly-2023-10-01"
BINDGEN_VERSION = "0.2.87"
NODE_VERSION = "v24.19.0"
RAW_DISPLAY_ID = "wasm_wrapper_raw_display_v1"
# Hashes of the pinned external wrapper and its authorized presentation spans.
# No upstream implementation is stored in this repository.
WRAPPER_SHA256 = "b28410955c073a656381c76d8ebde9b231ef4f4800fd9f25733a0848fb0d8c08"
ROUND_SHA256 = "470a01918567a0c09e97145def2065fc015b92fc226a2367b34ac1eff7b74c0f"
TRUNC_SHA256 = "80e579d6f802f0bc95a9cb8bdf555845f21b2636433d63a6678fa736c639d555"
RAW_WRAPPER_SHA256 = "43be71b38f47ba7d187609f491f407dc6df5ed666647c2a0c4262baf99094c1a"
MAX_INPUT_BYTES = 64 * 1024
MAX_OUTPUT_BYTES = 64 * 1024 * 1024
MAX_LOG_BYTES = 16 * 1024 * 1024
MAX_NODES = 4000
RANKS = "23456789TJQKA"
SUITS = "cdhs"
CARD_PATTERN = re.compile(r"[2-9TJQKA][cdhs]")
BET_PATTERN = re.compile(r"(?:(?:\d{1,3}(?:\.\d{1,2})?%|a)(?:,(?!$))?){1,3}")
RAISE_PATTERN = re.compile(r"(?:(?:\d{1,3}(?:\.\d{1,2})?%|a)(?:,(?!$))?){1,2}")
CASE_KEYS = {
    "id",
    "street",
    "board",
    "ranges",
    "range_labels",
    "chips_per_bb",
    "starting_pot",
    "effective_stack",
    "donk_option",
    "menus",
    "export_runouts",
    "min_bet",
    "max_raises",
    "add_all_in_threshold",
    "force_all_in_threshold",
    "target_pct_of_pot",
    "max_iterations",
    "check_every",
}
STREET_MENU_KEYS = {
    "flop": {"oop_bet", "oop_raise", "ip_bet", "ip_raise"},
    "turn": {"oop_bet", "oop_raise", "oop_donk", "ip_bet", "ip_raise"},
    "river": {"oop_bet", "oop_raise", "oop_donk", "ip_bet", "ip_raise"},
}
ROOT = Path(__file__).resolve().parent


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def reject_constant(value: str) -> None:
    raise ValueError(f"Nonfinite JSON constant: {value}")


def finite_float(value: str) -> float:
    number = float(value)
    require(math.isfinite(number), "JSON number overflows floating point")
    return number


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        require(key not in result, f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path, limit: int) -> dict:
    require(
        path.is_file() and path.stat().st_size <= limit, f"Invalid file size: {path}"
    )
    with path.open("rb") as stream:
        content = stream.read(limit + 1)
    require(len(content) <= limit, "File grew beyond its size limit")
    value = json.loads(
        content,
        parse_constant=reject_constant,
        parse_float=finite_float,
        object_pairs_hook=unique_object,
    )
    require(type(value) is dict, "JSON root must be an object")
    return value


# --- range expansion -------------------------------------------------------------------
# Mirrors crates/cards/src/range.rs so the reference input and the project input mean the
# same thing. It is deliberately a second implementation: if the two disagree, the private
# card cross-check below fails loudly rather than quietly comparing different ranges.


def card_id(label: str) -> int:
    require(CARD_PATTERN.fullmatch(label) is not None, f"Invalid card: {label}")
    return 4 * RANKS.index(label[0]) + SUITS.index(label[1])


def card_label(identifier: int) -> str:
    require(0 <= identifier < 52, "Invalid card id")
    return RANKS[identifier // 4] + SUITS[identifier % 4]


def parse_class(text: str) -> tuple[int, int, str]:
    require(len(text) in (2, 3), f"Expected a two-rank class: {text}")
    require(text[0] in RANKS and text[1] in RANKS, f"Invalid rank in class: {text}")
    high, low = RANKS.index(text[0]), RANKS.index(text[1])
    require(high >= low, f"Class ranks must be in descending order: {text}")
    kind = text[2] if len(text) == 3 else ""
    require(kind in ("", "s", "o"), f"Class suffix must be s or o: {text}")
    require(high != low or kind == "", f"Pairs cannot have a suitedness suffix: {text}")
    return high, low, kind


def expand_expression(expression: str) -> list[tuple[int, int, str]] | tuple[int, int]:
    if "-" in expression:
        parts = expression.split("-")
        require(len(parts) == 2, f"Interval needs exactly one dash: {expression}")
        start, end = parse_class(parts[0]), parse_class(parts[1])
        require(start[2] == end[2], f"Interval suffixes must match: {expression}")
        start_pair, end_pair = start[0] == start[1], end[0] == end[1]
        require(
            start_pair == end_pair, f"Interval mixes a pair and a nonpair: {expression}"
        )
        if start_pair:
            low, high = sorted((start[0], end[0]))
            return [(rank, rank, "") for rank in range(low, high + 1)]
        low, high = sorted((start[1], end[1]))
        if start[0] == end[0]:
            return [(start[0], value, start[2]) for value in range(low, high + 1)]
        require(
            start[0] - start[1] == end[0] - end[1],
            f"Interval must keep its high rank or its rank gap constant: {expression}",
        )
        gap = start[0] - start[1]
        return [(value + gap, value, start[2]) for value in range(low, high + 1)]
    if expression.endswith("+"):
        high, low, kind = parse_class(expression[:-1])
        if high == low:
            return [(rank, rank, "") for rank in range(high, 13)]
        return [(high, value, kind) for value in range(low, high)]
    if len(expression) == 4:
        first, second = card_id(expression[:2]), card_id(expression[2:])
        require(first != second, f"Expected two distinct cards: {expression}")
        return (min(first, second), max(first, second))
    return [parse_class(expression)]


def combo_matches(item: tuple[int, int, str], low: int, high: int) -> bool:
    suited = low % 4 == high % 4
    kind = item[2]
    return (
        low // 4 == item[1]
        and high // 4 == item[0]
        and (kind == "" or (kind == "s") == suited)
    )


def expand_range(text: str) -> dict[tuple[int, int], float]:
    """Return positive inclusion weights keyed by (low card id, high card id)."""
    require(type(text) is str and 1 <= len(text) <= 1024, "Invalid range size")
    tokens: list[str] = []
    for part in text.split(","):
        require(part.strip() != "", "Empty comma item in range")
        tokens.extend(token for token in part.split() if token)
    require(1 <= len(tokens) <= 64, "Unsupported range token count")
    weights: dict[tuple[int, int], float] = {}
    for token in tokens:
        expression, _, suffix = token.partition(":")
        weight = 1.0
        if suffix:
            weight = float(suffix)
            require(
                math.isfinite(weight) and 0.0 <= weight <= 1.0,
                f"Weight must be finite and in [0,1]: {token}",
            )
        expanded = expand_expression(expression)
        if isinstance(expanded, tuple):
            targets = [expanded]
        else:
            targets = [
                (low, high)
                for low in range(52)
                for high in range(low + 1, 52)
                if any(combo_matches(item, low, high) for item in expanded)
            ]
        require(len(targets) > 0, f"Token matched no combo: {token}")
        for target in targets:
            previous = weights.get(target)
            require(
                previous is None or previous == weight,
                f"Conflicting assignment for {token}",
            )
            weights[target] = weight
    return {pair: weight for pair, weight in weights.items() if weight > 0.0}


def physical_hands(text: str, board: list[str]) -> set[tuple[int, int]]:
    dead = {card_id(label) for label in board}
    return {
        pair
        for pair in expand_range(text)
        if pair[0] not in dead and pair[1] not in dead
    }


# --- input contract --------------------------------------------------------------------


def validate_inputs(payload: dict) -> None:
    require(
        set(payload) == {"schema_version", "street", "ranges_provenance", "cases"},
        "Unknown input root fields",
    )
    require(
        type(payload["schema_version"]) is int and payload["schema_version"] == 1,
        "Unsupported schema",
    )
    require(payload["street"] == "turn", "This runner captures turn-start trees only")
    provenance = payload["ranges_provenance"]
    require(
        type(provenance) is str and 40 <= len(provenance) <= 2048,
        "Ranges must carry a provenance sentence",
    )
    cases = payload["cases"]
    require(type(cases) is list and 1 <= len(cases) <= 8, "Expected 1 to 8 cases")
    ids = set()
    for case in cases:
        require(type(case) is dict and set(case) == CASE_KEYS, "Invalid case fields")
        name = case["id"]
        require(
            type(name) is str
            and re.fullmatch(r"[a-z][a-z0-9_]{0,63}", name) is not None,
            "Invalid case id",
        )
        require(name not in ids, "Duplicate case id")
        ids.add(name)
        require(case["street"] == "turn", "Case street must be turn")
        board = case["board"]
        require(type(board) is list and len(board) == 4, "Expected four board cards")
        require(
            all(type(c) is str and CARD_PATTERN.fullmatch(c) for c in board),
            "Invalid board card",
        )
        require(len(set(board)) == 4, "Repeated board card")
        ranges = case["ranges"]
        require(type(ranges) is list and len(ranges) == 2, "Expected two ranges")
        for player, text in enumerate(ranges):
            hands = physical_hands(text, board)
            require(
                50 <= len(hands) <= 1326,
                f"Range {player} has {len(hands)} live combos; expected 50 to 1326",
            )
        labels = case["range_labels"]
        require(
            type(labels) is list
            and len(labels) == 2
            and all(type(x) is str and 1 <= len(x) <= 64 for x in labels),
            "Invalid range labels",
        )
        require(type(case["donk_option"]) is bool, "donk_option must be a boolean")
        menus = case["menus"]
        require(
            type(menus) is dict and set(menus) == set(STREET_MENU_KEYS),
            "Menus must name flop, turn and river",
        )
        for street, keys in STREET_MENU_KEYS.items():
            menu = menus[street]
            require(
                type(menu) is dict and set(menu) == keys,
                f"Invalid {street} menu fields",
            )
            for key, value in menu.items():
                require(
                    type(value) is str and len(value) <= 32, f"Invalid {street}.{key}"
                )
                if street == "flop":
                    require(value == "", "A turn tree must leave the flop menu empty")
                elif key.endswith("_donk"):
                    require(value == "", "Donk sizes are not supported by this runner")
                elif key.endswith("_bet"):
                    require(
                        BET_PATTERN.fullmatch(value) is not None, f"Unsupported {key}"
                    )
                else:
                    require(
                        RAISE_PATTERN.fullmatch(value) is not None, f"Unsupported {key}"
                    )
        require(
            not case["donk_option"],
            "donk_option must be false while donk sizes are empty",
        )
        runouts = case["export_runouts"]
        require(
            type(runouts) is list and 1 <= len(runouts) <= 4,
            "Expected 1 to 4 export runouts",
        )
        require(
            all(type(c) is str and CARD_PATTERN.fullmatch(c) for c in runouts),
            "Invalid export runout",
        )
        require(len(set(runouts)) == len(runouts), "Repeated export runout")
        require(
            not set(runouts).intersection(board),
            "Export runout is already on the board",
        )
        for field, value in (
            ("min_bet", 1),
            ("add_all_in_threshold", 0),
            ("force_all_in_threshold", 0),
        ):
            require(
                type(case[field]) is int and case[field] == value,
                f"Unsupported {field}",
            )
        # The binding has no raise cap of its own, so this is a bound the export
        # is checked against, one street at a time, not a setting sent upstream.
        require(
            type(case["max_raises"]) is int and 1 <= case["max_raises"] <= 32,
            "Unsupported max_raises",
        )
        require(
            type(case["chips_per_bb"]) is int and 1 <= case["chips_per_bb"] <= 100,
            "Unsupported chips per big blind",
        )
        require(
            type(case["starting_pot"]) is int and 2 <= case["starting_pot"] <= 400,
            "Unsupported starting pot",
        )
        require(
            type(case["effective_stack"]) is int
            and case["starting_pot"] < case["effective_stack"] <= 2000,
            "Unsupported effective stack",
        )
        require(
            type(case["max_iterations"]) is int
            and 1 <= case["max_iterations"] <= 20000,
            "Iteration limit must be 1 to 20000",
        )
        require(
            type(case["check_every"]) is int and 1 <= case["check_every"] <= 100,
            "Check interval must be 1 to 100",
        )
        target = case["target_pct_of_pot"]
        require(
            type(target) in {int, float}
            and math.isfinite(target)
            and 0.001 <= target <= 0.5,
            "Target must be 0.001 to 0.5 percent of pot",
        )


# --- output contract -------------------------------------------------------------------


def validate_presentation(output: dict, raw_display: bool) -> None:
    mode = "raw_f32" if raw_display else "upstream_display"
    require(
        output.get("presentation_mode", "upstream_display") == mode,
        "Incorrect presentation mode",
    )
    expected = {
        "reach_display_cutoff": 0 if raw_display else 0.0005,
        "values_rounded_by_upstream": not raw_display,
        "values_below_1_decimal_places": None if raw_display else 6,
        "zero_reach_evs": "null",
    }
    interface = output.get("interface", {})
    for key, value in expected.items():
        require(
            (key in interface or not raw_display)
            and interface.get(key, value) == value,
            f"Incorrect presentation metadata: {key}",
        )
    if raw_display:
        require(
            interface.get("arithmetic_precision") == "f32",
            "Raw display must retain f32 precision",
        )


def _validate_private_cards(result: dict, expected: dict) -> list[int]:
    private = result.get("private_cards")
    require(type(private) is list and len(private) == 2, "Missing private cards")
    counts = []
    for player, entries in enumerate(private):
        require(
            type(entries) is list and 1 <= len(entries) <= 1326,
            "Invalid private hand count",
        )
        seen = set()
        for entry in entries:
            ids = entry.get("ids")
            require(
                type(ids) is list
                and len(ids) == 2
                and all(type(i) is int for i in ids)
                and 0 <= ids[0] < ids[1] < 52,
                "Invalid private card IDs",
            )
            require(
                entry.get("cards") == [card_label(ids[0]), card_label(ids[1])],
                "Private card labels differ from IDs",
            )
            require(tuple(ids) not in seen, "Duplicate private hand")
            seen.add(tuple(ids))
        require(
            seen == physical_hands(expected["ranges"][player], expected["board"]),
            "Reference physical range differs from input",
        )
        counts.append(len(entries))
    return counts


def _validate_node_shapes(
    node: dict, counts: list[int], board: list[str], cap: int
) -> None:
    kind = node.get("kind")
    require(kind in {"decision", "chance", "terminal"}, "Unexpected node kind")
    labels = node.get("history_labels")
    history = node.get("history")
    require(
        type(history) is list and type(labels) is list and len(labels) == len(history),
        "Mismatched history labels",
    )
    require(len(history) <= 64, "Invalid history length")
    # max_raises counts raises after the opening bet on one street, so the tally
    # restarts at every deal. Counting the whole history would charge a river bet
    # against the turn's raises.
    per_street = [0]
    for label in labels:
        if label.startswith("chance:"):
            per_street.append(0)
        elif label.startswith(("bet:", "raise:", "allin:")):
            per_street[-1] += 1
    require(
        all(max(0, wagers - 1) <= cap for wagers in per_street),
        "Reference exceeded project raise cap",
    )
    require(node.get("street") in {"turn", "river"}, "Unexpected node street")
    contributions = node.get("contributions")
    reported = node.get("reported_contributions")
    require(
        type(contributions) is list
        and type(reported) is list
        and len(contributions) == 2
        and len(reported) == 2
        and all(type(v) is int and v >= 0 for v in contributions + reported),
        "Invalid chip contributions",
    )
    if kind == "terminal":
        require(
            node.get("actions") == [] and node.get("player") is None, "Invalid terminal"
        )
        require(node.get("terminal") in {"fold", "showdown"}, "Invalid terminal reason")
        if node["terminal"] == "fold":
            require(node.get("fold_winner") in {0, 1}, "Invalid fold winner")
            require(contributions[0] == contributions[1], "Fold refund was not applied")
        else:
            require(node.get("fold_winner") is None, "Showdown has a fold winner")
        # A turn-street showdown is an all-in called before the river; its value spans every
        # runout, so the capture keeps the reported per-hand values as review evidence.
        wants_values = node["street"] == "turn" and node["terminal"] == "showdown"
        for field in (
            "reach_weights",
            "normalized_weights",
            "expected_values",
            "ev_available",
        ):
            if not wants_values:
                require(field not in node, f"Unexpected {field} on a river terminal")
                continue
            rows = node.get(field)
            require(
                type(rows) is list
                and len(rows) == 2
                and all(
                    row is None or (type(row) is list and len(row) == count)
                    for row, count in zip(rows, counts, strict=True)
                ),
                f"Invalid terminal {field} dimensions",
            )
        return
    if kind == "chance":
        require(
            node.get("actions") == [] and node.get("player") is None,
            "Invalid chance node",
        )
        possible = node.get("possible_cards")
        require(
            type(possible) is list and 1 <= len(possible) <= 48,
            "Invalid possible-card list",
        )
        require(
            all(type(c) is str and CARD_PATTERN.fullmatch(c) for c in possible)
            and len(set(possible)) == len(possible)
            and not set(possible).intersection(board),
            "Invalid possible card",
        )
        representatives = node.get("representative_action_count")
        merged = node.get("isomorphic_merged_cards")
        require(
            type(representatives) is int and 1 <= representatives <= len(possible),
            "Invalid representative count",
        )
        require(
            merged == len(possible) - representatives, "Inconsistent merged-card count"
        )
        exported = node.get("exported_runouts")
        require(
            type(exported) is list and set(exported).issubset(set(possible)),
            "Exported runout is not dealable",
        )
        for field in (
            "reach_weights",
            "normalized_weights",
            "expected_values",
            "ev_available",
        ):
            rows = node.get(field)
            require(
                type(rows) is list
                and len(rows) == 2
                and all(
                    row is None or (type(row) is list and len(row) == count)
                    for row, count in zip(rows, counts, strict=True)
                ),
                f"Invalid chance {field} dimensions",
            )
        return
    player = node.get("player")
    require(player in {0, 1}, "Invalid acting player")
    actions = node.get("actions")
    require(type(actions) is list and 1 <= len(actions) <= 4, "Invalid action count")
    labels_seen = set()
    for action in actions:
        require(
            type(action) is dict and set(action) == {"kind", "label", "amount"},
            "Invalid action shape",
        )
        require(action["label"] not in labels_seen, "Repeated action label")
        labels_seen.add(action["label"])
    count = counts[player]
    require(
        type(node.get("reach_weights")) is list and len(node["reach_weights"]) == count,
        "Invalid reach dimensions",
    )
    require(
        type(node.get("ev_available")) is list
        and len(node["ev_available"]) == count
        and all(type(v) is bool for v in node["ev_available"]),
        "Invalid EV availability dimensions",
    )
    size = len(actions) * count
    for field in ("strategy", "action_expected_values"):
        require(
            type(node.get(field)) is list and len(node[field]) == size,
            f"Invalid {field} dimensions",
        )
    for hand in range(count):
        row = [node["strategy"][a * count + hand] for a in range(len(actions))]
        require(
            all(
                type(v) in {int, float} and math.isfinite(v) and 0 <= v <= 1
                for v in row
            )
            and abs(sum(row) - 1) <= 0.00002,
            "Invalid strategy row",
        )
        available = node["ev_available"][hand]
        require(
            all(
                (node["action_expected_values"][a * count + hand] is not None)
                == available
                for a in range(len(actions))
            ),
            "Unavailable action EV was filled",
        )


def _validate_topology(nodes: list[dict]) -> None:
    histories: dict[tuple, dict] = {}
    for node in nodes:
        key = tuple(node["history"])
        require(key not in histories, "Repeated history")
        histories[key] = node
    require(() in histories, "Missing root")
    children: dict[tuple, set[int]] = {key: set() for key in histories}
    for key, node in histories.items():
        if not key:
            require(node["street"] == "turn", "Root must be a turn node")
            continue
        parent = histories.get(key[:-1])
        require(parent is not None, "Orphan node")
        children[key[:-1]].add(key[-1])
        entry = key[-1]
        require(type(entry) is int and entry >= 0, "Invalid history entry")
        if parent["kind"] == "chance":
            require(0 <= entry < 52, "Chance history entry must be a card id")
            label = card_label(entry)
            require(
                label in parent["exported_runouts"],
                "Chance child is not an exported runout",
            )
            require(
                node["history_labels"]
                == parent["history_labels"] + [f"chance:{label}"],
                "Chance history label mismatch",
            )
            require(node["street"] == "river", "A dealt runout must be a river node")
            require(node["runout"] == label, "Runout label mismatch")
            if node["kind"] == "decision":
                require(node["player"] == 0, "OOP acts first after a chance node")
        else:
            require(
                parent["kind"] == "decision", "Only decision nodes take action children"
            )
            require(entry < len(parent["actions"]), "Action index out of range")
            require(
                node["history_labels"]
                == parent["history_labels"] + [parent["actions"][entry]["label"]],
                "History label mismatch",
            )
            require(node["street"] == parent["street"], "Betting keeps the street")
            require(node["runout"] == parent["runout"], "Betting keeps the runout")
            if node["kind"] == "decision":
                require(
                    node["player"] == 1 - parent["player"],
                    "Consecutive decisions must alternate players",
                )
    for key, node in histories.items():
        if node["kind"] == "decision":
            require(
                children[key] == set(range(len(node["actions"]))),
                "Reference omitted a decision branch",
            )
        elif node["kind"] == "chance":
            require(
                children[key] == {card_id(label) for label in node["exported_runouts"]},
                "Reference omitted an exported runout",
            )
        else:
            require(not children[key], "Terminal node has children")


def validate_output(
    output: dict, payload: dict, finish_budget: bool = False, raw_display: bool = False
) -> None:
    require(
        output.get("schema_version") == 1 and output.get("capture_version") == 1,
        "Unexpected reference schema",
    )
    require(output.get("street") == "turn", "Unexpected reference street")
    require(
        output.get("runtime", {}).get("node") == NODE_VERSION, "Unexpected Node runtime"
    )
    policy = "fixed_iteration_budget" if finish_budget else "target_or_cap"
    require(
        output.get("execution_stop_policy", "target_or_cap") == policy,
        "Incorrect execution stop policy",
    )
    validate_presentation(output, raw_display)
    cases = output.get("cases")
    require(
        type(cases) is list and len(cases) == len(payload["cases"]), "Missing cases"
    )
    for result, expected in zip(cases, payload["cases"], strict=True):
        require(result.get("input") == expected, "Reference changed its inputs")
        require(
            result.get("execution_stop_policy", "target_or_cap") == policy,
            "Incorrect case stop policy",
        )
        require(
            type(result.get("iterations")) is int
            and 0 <= result["iterations"] <= expected["max_iterations"],
            "Invalid measured iteration count",
        )
        residual = result.get("exploitability_chips")
        require(
            type(residual) in {float, int} and math.isfinite(residual),
            "Invalid residual",
        )
        require(
            abs(
                result["exploitability_pct_of_pot"]
                - 100 * residual / expected["starting_pot"]
            )
            < 1e-12,
            "Residual units differ",
        )
        target = expected["target_pct_of_pot"] * expected["starting_pot"] / 100
        reason = (
            "fixed_iteration_budget"
            if finish_budget
            else "target" if residual <= target else "iteration_cap"
        )
        require(result.get("stop_reason") == reason, "Incorrect stop reason")
        require(
            reason not in {"iteration_cap", "fixed_iteration_budget"}
            or result["iterations"] == expected["max_iterations"],
            "Incomplete iteration budget",
        )
        nodes = result.get("nodes")
        require(
            type(nodes) is list and 1 <= len(nodes) <= MAX_NODES, "Invalid node count"
        )
        counts = _validate_private_cards(result, expected)
        for node in nodes:
            _validate_node_shapes(
                node, counts, expected["board"], expected["max_raises"]
            )
        _validate_topology(nodes)
        require(
            sum(node["kind"] == "chance" for node in nodes) >= 1,
            "A turn tree must contain at least one chance node",
        )


# --- orchestration ---------------------------------------------------------------------


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_command(
    args: list[str], cwd: Path, env: dict, timeout: int, commands: list
) -> str:
    """Bound subprocess time/log output and kill the entire process group on failure."""
    log = cwd / f"command-{len(commands):03d}.log"
    started = time.monotonic()
    print(f"Reference step {len(commands) + 1}: {args[0]} {args[1:3]!r}", flush=True)
    with log.open("wb") as output:
        process = subprocess.Popen(
            args,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            while process.poll() is None:
                if (
                    time.monotonic() - started > timeout
                    or log.stat().st_size > MAX_LOG_BYTES
                ):
                    raise TimeoutError(
                        f"Reference command exceeded time/log budget: {args[0]}"
                    )
                time.sleep(0.1)
            require(log.stat().st_size <= MAX_LOG_BYTES, "Reference log exceeds budget")
            if process.returncode:
                tail = log.read_bytes()[-6000:].decode("utf-8", errors="replace")
                raise RuntimeError(
                    f"Command failed ({process.returncode}): {args!r}\n{tail}"
                )
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    commands.append({"argv": args, "seconds": time.monotonic() - started})
    return log.read_text(encoding="utf-8", errors="replace").strip()


def replace_once(path: Path, before: str, after: str) -> None:
    text = path.read_text(encoding="utf-8")
    require(text.count(before) == 1, f"Unexpected upstream manifest: {path.name}")
    path.write_text(text.replace(before, after), encoding="utf-8")


def instrument_wrapper(source: bytes) -> bytes:
    """Replace only three hash-verified presentation spans in external source."""
    round_pattern = rb"(?ms)^fn round\(value: f64\) -> f64 \{\n.*?^\}"
    trunc_pattern = rb"(?m)^( *)let trunc = .*;$"
    rounds = list(re.finditer(round_pattern, source))
    truncs = list(re.finditer(trunc_pattern, source))
    require(len(rounds) == 1, "Expected exactly one wrapper round function")
    require(len(truncs) == 2, "Expected exactly two wrapper trunc closures")
    require(
        hashlib.sha256(rounds[0][0]).hexdigest() == ROUND_SHA256,
        "Unexpected wrapper round source shape",
    )
    require(
        all(hashlib.sha256(m[0].strip()).hexdigest() == TRUNC_SHA256 for m in truncs),
        "Unexpected wrapper trunc source shape",
    )
    require(
        hashlib.sha256(source).hexdigest() == WRAPPER_SHA256,
        "Unexpected pinned wrapper hash",
    )
    result = re.sub(
        round_pattern, b"fn round(value: f64) -> f64 {\n    value\n}", source
    )
    result = re.sub(
        trunc_pattern, lambda m: m[1] + b"let trunc = |&w: &f32| w;", result
    )
    require(
        hashlib.sha256(result).hexdigest() == RAW_WRAPPER_SHA256,
        "Unexpected instrumented wrapper hash",
    )
    return result


def prepare_wrapper(path: Path, raw_display: bool) -> dict:
    source = path.read_bytes()
    original = hashlib.sha256(source).hexdigest()
    require(original == WRAPPER_SHA256, "Unexpected pinned wrapper hash")
    instrumented = instrument_wrapper(source) if raw_display else source
    # Default mode does not write the wrapper, even with byte-identical contents.
    if raw_display:
        path.write_bytes(instrumented)
    return {
        "wrapper_source_path": "rust/solver-src/lib.rs",
        "original_wrapper_sha256": original,
        "instrumented_wrapper_sha256": hashlib.sha256(instrumented).hexdigest(),
        "instrumentation_id": RAW_DISPLAY_ID if raw_display else None,
        "presentation_replacements": {
            "round": int(raw_display),
            "trunc": 2 * int(raw_display),
        },
    }


def orchestrate(
    inputs: Path,
    destination: Path,
    temp_root: Path,
    finish_budget: bool = False,
    raw_display: bool = False,
) -> None:
    require(sys.platform == "linux", "Reference compilation runs only on Linux CI")
    payload = read_json(inputs, MAX_INPUT_BYTES)
    validate_inputs(payload)
    require(not destination.exists(), "Refusing to overwrite an existing capture")
    workspace = Path(os.environ.get("GITHUB_WORKSPACE", ROOT.parents[2])).resolve()
    temp_root = temp_root.resolve(strict=True)
    require(
        not temp_root.is_relative_to(workspace),
        "External temp root is inside the repository",
    )
    commands: list[dict] = []
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="turn-wasm-", dir=temp_root) as directory:
        external = Path(directory)
        env = dict(os.environ)
        env.update(
            {
                "CARGO_HOME": str(external / "cargo"),
                "RUSTUP_HOME": str(external / "rustup"),
                "CARGO_TARGET_DIR": str(external / "target"),
                "CARGO_BUILD_JOBS": "2",
                "GIT_TERMINAL_PROMPT": "0",
                "CARGO_TERM_COLOR": "never",
            }
        )

        def command(args, cwd=external, timeout=600):
            return run_command(args, cwd, env, timeout, commands)

        node = command(["node", "--version"])
        require(node == NODE_VERSION, f"Expected Node {NODE_VERSION}, got {node}")
        for name, revision in (
            ("wasm-postflop", WASM_REVISION),
            ("postflop-solver", ENGINE_REVISION),
        ):
            checkout = external / name
            checkout.mkdir()
            command(["git", "init", "--quiet"], checkout)
            command(
                [
                    "git",
                    "remote",
                    "add",
                    "origin",
                    f"https://github.com/b-inary/{name}.git",
                ],
                checkout,
            )
            command(
                ["git", "fetch", "--quiet", "--depth", "1", "origin", revision],
                checkout,
            )
            command(["git", "checkout", "--quiet", "--detach", "FETCH_HEAD"], checkout)
            require(
                command(["git", "rev-parse", "HEAD"], checkout) == revision,
                "Wrong reference revision",
            )
            require((checkout / "LICENSE").is_file(), "Upstream license missing")
        reference = external / "wasm-postflop" / "rust" / "solver-st"
        wrapper = reference.parent / "solver-src" / "lib.rs"
        presentation = prepare_wrapper(wrapper, raw_display)
        engine_manifest = external / "postflop-solver" / "Cargo.toml"
        manifest = reference / "Cargo.toml"
        before_hashes = {
            "wasm_manifest": sha256(manifest),
            "engine_manifest": sha256(engine_manifest),
        }
        replace_once(
            manifest,
            'postflop-solver = { git = "https://github.com/b-inary/postflop-solver",',
            'postflop-solver = { path = "../../../postflop-solver",',
        )
        replace_once(manifest, 'wasm-bindgen = "0.2.87"', 'wasm-bindgen = "=0.2.87"')
        replace_once(engine_manifest, 'once_cell = "1.18.0"', 'once_cell = "=1.18.0"')
        replace_once(engine_manifest, 'regex = "1.9.6"', 'regex = "=1.9.6"')
        command(
            ["git", "diff", "--exit-code", "--", "src"], external / "postflop-solver"
        )
        command(
            [
                "rustup",
                "toolchain",
                "install",
                TOOLCHAIN,
                "--profile",
                "minimal",
                "--component",
                "rust-src",
                "--target",
                "wasm32-unknown-unknown",
            ],
            timeout=900,
        )
        rustc = command(["rustup", "run", TOOLCHAIN, "rustc", "-Vv"])
        cargo = ["rustup", "run", TOOLCHAIN, "cargo"]
        command(
            cargo
            + [
                "install",
                "wasm-bindgen-cli",
                "--version",
                BINDGEN_VERSION,
                "--locked",
                "--root",
                str(external / "tools"),
            ],
            timeout=1800,
        )
        command(cargo + ["generate-lockfile"], reference)
        command(
            cargo
            + ["build", "--locked", "--release", "--target", "wasm32-unknown-unknown"],
            reference,
            timeout=900,
        )
        bindgen = external / "tools" / "bin" / "wasm-bindgen"
        version = command([str(bindgen), "--version"])
        require(
            version == f"wasm-bindgen {BINDGEN_VERSION}",
            "Unexpected wasm-bindgen version",
        )
        package = external / "package"
        command(
            [
                str(bindgen),
                str(
                    external
                    / "target"
                    / "wasm32-unknown-unknown"
                    / "release"
                    / "solver.wasm"
                ),
                "--target",
                "nodejs",
                "--out-dir",
                str(package),
                "--out-name",
                "solver",
            ]
        )
        validated_inputs = external / "inputs.json"
        validated_inputs.write_text(
            json.dumps(payload, allow_nan=False), encoding="utf-8"
        )
        driver = external / "capture.mjs"
        shutil.copyfile(ROOT / "capture.mjs", driver)
        raw_output = external / "capture.json"
        command(
            [
                "node",
                "--max-old-space-size=4096",
                str(driver),
                str(package / "solver.js"),
                str(validated_inputs),
                str(raw_output),
            ]
            + (["--finish-budget"] if finish_budget else [])
            + (["--raw-display"] if raw_display else []),
            timeout=3600,
        )
        output = read_json(raw_output, MAX_OUTPUT_BYTES)
        validate_output(output, payload, finish_budget, raw_display)
        require(
            sha256(wrapper) == presentation["instrumented_wrapper_sha256"],
            "Wrapper changed during capture",
        )
        lock = tomllib.loads((reference / "Cargo.lock").read_text(encoding="utf-8"))
        output["provenance"] = {
            **presentation,
            "wasm_postflop_revision": WASM_REVISION,
            "engine_revision": ENGINE_REVISION,
            "toolchain": TOOLCHAIN,
            "rustc_verbose": rustc,
            "wasm_bindgen_cli": version,
            "python": sys.version,
            "capture_python_sha256": sha256(Path(__file__)),
            "capture_javascript_sha256": sha256(driver),
            "input_file_sha256": sha256(inputs),
            "resolved_reference_lock_sha256": sha256(reference / "Cargo.lock"),
            "resolved_dependencies": [
                {
                    key: package[key]
                    for key in ("name", "version", "source", "checksum")
                    if key in package
                }
                for package in lock["package"]
            ],
            "wasm_sha256": sha256(package / "solver_bg.wasm"),
            "wasm_bindings_sha256": sha256(package / "solver.js"),
            "upstream_manifest_hashes": before_hashes,
            "adjusted_manifest_hashes": {
                "wasm_manifest": sha256(manifest),
                "engine_manifest": sha256(engine_manifest),
            },
            "build_adjustments": [
                "Engine dependency replaced with verified local pinned checkout",
                "wasm-bindgen =0.2.87, once_cell =1.18.0, regex =1.9.6",
                "wasm-bindgen nodejs bindings; no wasm-opt; unmodified engine source",
            ]
            + (["Wrapper presentation only: " + RAW_DISPLAY_ID] if raw_display else []),
            "execution": "single-thread upstream WASM in separate Node process",
            "hosted_website_build_reproduction": False,
            "elapsed_seconds": time.monotonic() - started,
            "commands": commands,
        }
        serialized = json.dumps(output, allow_nan=False, separators=(",", ":")) + "\n"
        require(
            len(serialized.encode("utf-8")) <= MAX_OUTPUT_BYTES,
            "Final capture exceeds 64 MiB",
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        staging = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=destination.parent,
                prefix=".turn-capture-",
                delete=False,
            ) as stream:
                staging = Path(stream.name)
                stream.write(serialized)
                stream.flush()
                os.fsync(stream.fileno())
            # Same-directory link publishes atomically and refuses an existing destination.
            os.link(staging, destination)
        finally:
            if staging is not None:
                staging.unlink(missing_ok=True)
        for case in output["cases"]:
            print(
                f"{case['input']['id']}: {case['iterations']} iterations, "
                f"{case['exploitability_pct_of_pot']:.4f}% of pot ({case['stop_reason']}), "
                f"{len(case['nodes'])} exported nodes",
                flush=True,
            )
        print(
            f"Saved {len(output['cases'])} measured WASM cases to {destination}",
            flush=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, default=ROOT / "cases.json")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--temp-root", type=Path, default=os.environ.get("RUNNER_TEMP"))
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument(
        "--finish-budget",
        action="store_true",
        help="Run every input iteration even after reaching the target residual",
    )
    parser.add_argument(
        "--raw-display",
        action="store_true",
        help="Remove only wrapper display rounding and reach cutoff; retain f32 arithmetic",
    )
    args = parser.parse_args()
    if args.validate_only:
        validate_inputs(read_json(args.inputs, MAX_INPUT_BYTES))
        print("Input contract passed")
        return
    require(
        args.output is not None and args.temp_root is not None,
        "Capture requires --output and --temp-root (or RUNNER_TEMP)",
    )
    orchestrate(
        args.inputs.resolve(),
        args.output.resolve(),
        args.temp_root,
        args.finish_budget,
        args.raw_display,
    )


if __name__ == "__main__":
    main()
