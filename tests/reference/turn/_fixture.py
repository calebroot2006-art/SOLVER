"""Synthetic turn captures for the unit tests. Not used by the capture or the comparison.

The tree is the smallest one that still has every shape the turn schema adds: a turn betting
round, an all-in called before the river, a chance node with one exported runout, a river
betting round with a raise, two fold terminals and two showdowns.
"""

from __future__ import annotations

import copy

BOARD = ["Ac", "Kd", "7s", "2h"]
OOP_RANGE = "AA,KK"
IP_RANGE = "QQ,JJ"
RUNOUT = "4c"
POT = 10
STACK = 40

OOP_HANDS = [
    ["Ad", "Ah"],
    ["Ad", "As"],
    ["Ah", "As"],
    ["Kc", "Kh"],
    ["Kc", "Ks"],
    ["Kh", "Ks"],
]
IP_HANDS = [
    ["Qc", "Qd"],
    ["Qc", "Qh"],
    ["Qc", "Qs"],
    ["Qd", "Qh"],
    ["Qd", "Qs"],
    ["Qh", "Qs"],
    ["Jc", "Jd"],
    ["Jc", "Jh"],
    ["Jc", "Js"],
    ["Jd", "Jh"],
    ["Jd", "Js"],
    ["Jh", "Js"],
]
HANDS = [OOP_HANDS, IP_HANDS]


def case_input(runout=RUNOUT, **overrides):
    value = {
        "id": "turn_fixture",
        "street": "turn",
        "board": list(BOARD),
        "ranges": [OOP_RANGE, IP_RANGE],
        "range_labels": ["OOP fixture", "IP fixture"],
        "chips_per_bb": 2,
        "starting_pot": POT,
        "effective_stack": STACK,
        "donk_option": False,
        "menus": {
            "flop": {"oop_bet": "", "oop_raise": "", "ip_bet": "", "ip_raise": ""},
            "turn": {
                "oop_bet": "33%,a",
                "oop_raise": "100%",
                "oop_donk": "",
                "ip_bet": "33%,a",
                "ip_raise": "100%",
            },
            "river": {
                "oop_bet": "33%,75%",
                "oop_raise": "100%",
                "oop_donk": "",
                "ip_bet": "33%,75%",
                "ip_raise": "100%",
            },
        },
        "export_runouts": [runout],
        "min_bet": 1,
        "max_raises": 32,
        "add_all_in_threshold": 0,
        "force_all_in_threshold": 0,
        "target_pct_of_pot": 0.25,
        "max_iterations": 1500,
        "check_every": 50,
    }
    value.update(overrides)
    return value


def card_id(label):
    return 4 * "23456789TJQKA".index(label[0]) + "cdhs".index(label[1])


RUNOUT_ID = card_id(RUNOUT)


def _action(label):
    kind = label.split(":")[0]
    amount = None if ":" not in label else int(label.split(":")[1])
    return {"kind": kind, "label": label, "amount": amount}


def live_hands(player, runout):
    return [hand for hand in HANDS[player] if not runout or runout not in hand]


def _decision(history, labels, street, runout, player, actions, weights, contributions):
    count = len(HANDS[player])
    origin = POT / 2 + contributions[player]
    centered = [
        -POT / 2 - contributions[player] if label == "fold" else float(index)
        for index, label in enumerate(actions)
    ]
    return {
        "history": history,
        "history_labels": labels,
        "street": street,
        "runout": runout,
        "contributions": contributions,
        "reported_contributions": contributions,
        "wasm_empty_range_flag": 0,
        "kind": "decision",
        "player": player,
        "actions": [_action(label) for label in actions],
        "terminal": None,
        "fold_winner": None,
        "reach_weights": [1.0] * count,
        "ev_available": [True] * count,
        "strategy": [
            weights[index] if not runout or runout not in hand else 1.0 / len(actions)
            for index in range(len(actions))
            for hand in HANDS[player]
        ],
        "action_expected_values": [
            centered[index] + origin
            for index in range(len(actions))
            for _ in range(count)
        ],
    }


def _terminal(
    history, labels, street, runout, kind, winner, contributions, values=False
):
    node = {
        "history": history,
        "history_labels": labels,
        "street": street,
        "runout": runout,
        "contributions": contributions,
        "reported_contributions": contributions,
        "wasm_empty_range_flag": 0,
        "kind": "terminal",
        "player": None,
        "actions": [],
        "terminal": kind,
        "fold_winner": winner,
    }
    if values:
        node.update(_reported_rows(contributions))
    return node


def _reported_rows(contributions):
    counts = [len(OOP_HANDS), len(IP_HANDS)]
    return {
        "reach_weights": [[1.0] * counts[0], [1.0] * counts[1]],
        "normalized_weights": [[1.0] * counts[0], [1.0] * counts[1]],
        # Display EVs: a centered value of zero plus the wrapper's origin.
        "expected_values": [
            [POT / 2 + contributions[0]] * counts[0],
            [POT / 2 + contributions[1]] * counts[1],
        ],
        "ev_available": [[True] * counts[0], [True] * counts[1]],
    }


def reference_capture(oop_check_frequency=0.75, stop_reason="target", runout=RUNOUT):
    """A complete reference capture that satisfies capture.validate_output."""
    counts = [len(OOP_HANDS), len(IP_HANDS)]
    check = oop_check_frequency
    river = [0, 0, card_id(runout)]
    river_labels = ["check", "check", f"chance:{runout}"]
    nodes = [
        _decision(
            [],
            [],
            "turn",
            None,
            0,
            ["check", f"allin:{STACK}"],
            [check, 1 - check],
            [0, 0],
        ),
        _decision(
            [0],
            ["check"],
            "turn",
            None,
            1,
            ["check", f"allin:{STACK}"],
            [0.5, 0.5],
            [0, 0],
        ),
        {
            "history": [0, 0],
            "history_labels": ["check", "check"],
            "street": "turn",
            "runout": None,
            "contributions": [0, 0],
            "reported_contributions": [0, 0],
            "wasm_empty_range_flag": 0,
            "kind": "chance",
            "player": None,
            "actions": [],
            "terminal": None,
            "fold_winner": None,
            "possible_cards": [runout, "5c", "6c"],
            "representative_action_count": 2,
            "isomorphic_merged_cards": 1,
            "exported_runouts": [runout],
            **_reported_rows([0, 0]),
        },
        _decision(
            river,
            river_labels,
            "river",
            runout,
            0,
            ["check", "bet:3"],
            [0.6, 0.4],
            [0, 0],
        ),
        _decision(
            river + [0],
            river_labels + ["check"],
            "river",
            runout,
            1,
            ["check"],
            [1.0],
            [0, 0],
        ),
        _terminal(
            river + [0, 0],
            river_labels + ["check", "check"],
            "river",
            runout,
            "showdown",
            None,
            [0, 0],
        ),
        _decision(
            river + [1],
            river_labels + ["bet:3"],
            "river",
            runout,
            1,
            ["fold", "call", "raise:10"],
            [0.2, 0.5, 0.3],
            [3, 0],
        ),
        _terminal(
            river + [1, 0],
            river_labels + ["bet:3", "fold"],
            "river",
            runout,
            "fold",
            0,
            [0, 0],
        ),
        _terminal(
            river + [1, 1],
            river_labels + ["bet:3", "call"],
            "river",
            runout,
            "showdown",
            None,
            [3, 3],
        ),
        _decision(
            river + [1, 2],
            river_labels + ["bet:3", "raise:10"],
            "river",
            runout,
            0,
            ["fold", "call"],
            [0.3, 0.7],
            [3, 10],
        ),
        _terminal(
            river + [1, 2, 0],
            river_labels + ["bet:3", "raise:10", "fold"],
            "river",
            runout,
            "fold",
            1,
            [3, 3],
        ),
        _terminal(
            river + [1, 2, 1],
            river_labels + ["bet:3", "raise:10", "call"],
            "river",
            runout,
            "showdown",
            None,
            [10, 10],
        ),
        _decision(
            [0, 1],
            ["check", f"allin:{STACK}"],
            "turn",
            None,
            0,
            ["fold", "call"],
            [0.4, 0.6],
            [0, STACK],
        ),
        _terminal(
            [0, 1, 0],
            ["check", f"allin:{STACK}", "fold"],
            "turn",
            None,
            "fold",
            1,
            [0, 0],
        ),
        _terminal(
            [0, 1, 1],
            ["check", f"allin:{STACK}", "call"],
            "turn",
            None,
            "showdown",
            None,
            [STACK, STACK],
            values=True,
        ),
        _decision(
            [1],
            [f"allin:{STACK}"],
            "turn",
            None,
            1,
            ["fold", "call"],
            [0.5, 0.5],
            [STACK, 0],
        ),
        _terminal([1, 0], [f"allin:{STACK}", "fold"], "turn", None, "fold", 0, [0, 0]),
        _terminal(
            [1, 1],
            [f"allin:{STACK}", "call"],
            "turn",
            None,
            "showdown",
            None,
            [STACK, STACK],
            values=True,
        ),
    ]
    return {
        "schema_version": 1,
        "capture_version": 1,
        "street": "turn",
        "execution_stop_policy": "target_or_cap",
        "presentation_mode": "upstream_display",
        "runtime": {"node": "v24.19.0"},
        "interface": {
            "strategy_layout": "action_major",
            "reach_display_cutoff": 0.0005,
            "values_rounded_by_upstream": True,
            "values_below_1_decimal_places": 6,
            "arithmetic_precision": "f32",
            "zero_reach_evs": "null",
            "rake_rate": 0,
            "rake_cap": 0,
        },
        "provenance": {
            "engine_revision": "9d1509fe5077d019825f833eed04b16d342dfda1",
            "wasm_postflop_revision": "97360db7644329b1c23a7adf06e9aa59406e4d4b",
        },
        "cases": [
            {
                "input": case_input(runout=runout),
                # The fixture keeps `max_raises: 32`, which this pot and stack cannot reach,
                # so `raise_cap.derive_removed_lines` returns nothing and the reference was
                # asked to prune nothing. A test that lowers the cap must supply the lines.
                "removed_lines": [],
                "private_cards": [
                    [
                        {"cards": hand, "ids": [card_id(hand[0]), card_id(hand[1])]}
                        for hand in HANDS[player]
                    ]
                    for player in (0, 1)
                ],
                "nodes": nodes,
                "iterations": 100 if stop_reason == "target" else 1500,
                "execution_stop_policy": "target_or_cap",
                "stop_reason": stop_reason,
                "exploitability_chips": 0.01 if stop_reason == "target" else 0.04,
                "exploitability_pct_of_pot": 0.1 if stop_reason == "target" else 0.4,
                "root_expected_values": [POT / 2 + 1 - check, POT / 2 - 1 + check],
                "root_centered_expected_values": [1 - check, check - 1],
                "root_normalized_weights": [[1.0] * counts[0], [1.0] * counts[1]],
                "checkpoints": [],
                "reference_memory_estimate_bytes": 4096,
                "wasm_memory_bytes": None,
                "elapsed_seconds": 1.0,
            }
        ],
    }


def project_capture(reference, root_check_frequency=None):
    """A project capture in the shape step 5b's `turn_capture.rs` must emit."""
    case = reference["cases"][0]
    nodes = []
    for node in case["nodes"]:
        runout = node["runout"] or ""
        entry = {
            "history_labels": list(node["history_labels"]),
            "kind": node["kind"],
            "street": node["street"],
            "runout": runout,
            "contributions": list(node["contributions"]),
            "terminal": node["terminal"] or "",
            "fold_winner": -1 if node["fold_winner"] is None else node["fold_winner"],
            "player": -1 if node["player"] is None else node["player"],
            "actions": [action["label"] for action in node["actions"]],
        }
        if node["kind"] == "decision":
            player = node["player"]
            count = len(HANDS[player])
            actions = len(node["actions"])
            origin = case["input"]["starting_pot"] / 2 + node["contributions"][player]
            hands = []
            for hand in live_hands(player, runout):
                index = HANDS[player].index(hand)
                strategy = [node["strategy"][a * count + index] for a in range(actions)]
                if not node["history_labels"] and root_check_frequency is not None:
                    strategy = [root_check_frequency, 1 - root_check_frequency]
                hands.append(
                    {
                        "cards": list(hand),
                        "strategy": strategy,
                        "action_expected_values": [
                            node["action_expected_values"][a * count + index] - origin
                            for a in range(actions)
                        ],
                        "ev_available": True,
                        "own_reach": 1.0,
                        "opponent_mass": 12.0,
                    }
                )
            entry["hands"] = hands
        elif node["kind"] == "chance":
            entry["possible_cards"] = list(node["possible_cards"])
            entry["isomorphic_merged_cards"] = 0
            entry["exported_runouts"] = list(node["exported_runouts"])
            entry["hands"] = _project_reported_hands(case, node)
        elif "expected_values" in node:
            entry["hands"] = _project_reported_hands(case, node)
        nodes.append(entry)
    capture = {
        "schema_version": 1,
        "street": "turn",
        "project_revision": "0" * 40,
        "execution_stop_policy": "target_or_cap",
        "os": "linux",
        "architecture": "x86_64",
        "cases": [
            {
                "input": copy.deepcopy(case["input"]),
                "iterations": 100,
                "stop_reason": "TargetReached",
                "exploitability_pct_of_pot": 0.1,
                "root_centered_expected_values": [0.0, 0.0],
                "best_response_values": [0.01, 0.01],
                "compatible_weight": 72.0,
                "nodes": nodes,
            }
        ],
    }
    # Policy-classification fixtures retain the one-chip action gap, but their
    # root mixture and BR summaries must describe the same root value.
    own_case = capture["cases"][0]
    root = own_case["nodes"][0]
    root_value = case["root_centered_expected_values"][0]
    for hand in root["hands"]:
        offset = root_value - sum(
            p * v for p, v in zip(hand["strategy"], hand["action_expected_values"])
        )
        hand["action_expected_values"] = [
            v + offset for v in hand["action_expected_values"]
        ]
    own_case["root_centered_expected_values"] = [root_value, -root_value]
    own_case["best_response_values"] = [root_value + 0.01, -root_value + 0.01]
    refresh_project_reach(own_case)
    return capture


def refresh_project_reach(case):
    """Populate producer reach semantics after a test deliberately changes a policy."""
    from oracle import Oracle, key

    oracle = Oracle(case)
    for history, node in oracle.nodes.items():
        if not node.get("hands"):
            continue
        players = (node["player"],) if node["kind"] == "decision" else (0, 1)
        for player in players:
            evidence = oracle.reach_evidence(history, player)
            for row in node["hands"]:
                if node["kind"] != "decision" and row["player"] != player:
                    continue
                own, mass = evidence[key(row["cards"])]
                row["own_reach"], row["opponent_mass"] = own, mass
                available = mass > 0 and (own > 0 or node["kind"] != "decision")
                row["ev_available"] = available
                if not available:
                    if node["kind"] == "decision":
                        row["action_expected_values"] = []
                    else:
                        row.pop("expected_value", None)


def _project_reported_hands(case, node):
    """Centered per-hand values; our solver never uses the wrapper's display origin."""
    hands = []
    runout = node["runout"] or ""
    for player in (0, 1):
        origin = case["input"]["starting_pot"] / 2 + node["contributions"][player]
        for hand in live_hands(player, runout):
            index = HANDS[player].index(hand)
            hands.append(
                {
                    "player": player,
                    "cards": list(hand),
                    "expected_value": node["expected_values"][player][index] - origin,
                    "ev_available": node["ev_available"][player][index],
                    "own_reach": 1.0,
                    "opponent_mass": 12.0,
                }
            )
    return hands
