"""Match measured turn policies against the pinned reference, or validate the reference alone.

Two modes:

* reference-only (`--reference` without `--project`): validate the capture against the output
  contract in `capture.py` and print its exploitability, iteration count, stop reason and
  runout bookkeeping. This is what CI runs until the project's turn capture exists.
* full (`--reference` and `--project`): every exported public history must match, and every
  policy row differing by more than two percentage points is listed. With `--review`, each of
  those rows must be covered by a committed `per-combo-review.json` entry carrying a non-empty
  `review_reasoning`; an uncovered row fails the run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

import tomllib
from capture import (
    ENGINE_REVISION,
    MAX_OUTPUT_BYTES,
    WASM_REVISION,
    card_id,
    card_label,
    expand_range,
    read_json,
    require,
    validate_output,
)

FREQUENCY_THRESHOLD = 0.02


def indexed(items, key):
    result = {}
    for item in items:
        name = key(item)
        require(name not in result, f"Repeated comparison key: {name}")
        result[name] = item
    return result


def hand_key(cards):
    return tuple(sorted(cards))


def finite_tree(value):
    if isinstance(value, float):
        require(math.isfinite(value), "Nonfinite project value")
    elif isinstance(value, dict):
        for item in value.values():
            finite_tree(item)
    elif isinstance(value, list):
        for item in value:
            finite_tree(item)


def compatible_mass(case_input):
    """Weighted count of ordered-by-player card-disjoint pairs, board cards removed."""
    board = set(case_input["board"])
    live = []
    for text in case_input["ranges"]:
        hands = {}
        for (low, high), weight in expand_range(text).items():
            cards = {
                "23456789TJQKA"[low // 4] + "cdhs"[low % 4],
                "23456789TJQKA"[high // 4] + "cdhs"[high % 4],
            }
            if not cards.intersection(board):
                hands[frozenset(cards)] = weight
        live.append(hands)
    total = 0.0
    for oop, oop_weight in live[0].items():
        for ip, ip_weight in live[1].items():
            if not oop.intersection(ip):
                total += oop_weight * ip_weight
    return total


def reference_payload(reference):
    """The shape `validate_output` needs: only `cases` is read from it."""
    return {"cases": [case["input"] for case in reference["cases"]]}


def validate_reference(reference):
    require(reference.get("street") == "turn", "Reference is not a turn capture")
    provenance = reference.get("provenance", {})
    require(
        provenance.get("engine_revision") == ENGINE_REVISION, "Wrong reference engine"
    )
    require(
        provenance.get("wasm_postflop_revision") == WASM_REVISION,
        "Wrong reference interface",
    )
    interface = reference.get("interface", {})
    require(
        interface.get("strategy_layout") == "action_major", "Wrong reference row layout"
    )
    require(
        interface.get("rake_rate") == 0 and interface.get("rake_cap") == 0,
        "Reference rake differs",
    )
    validate_output(
        reference,
        reference_payload(reference),
        reference.get("execution_stop_policy", "target_or_cap")
        == "fixed_iteration_budget",
        reference.get("presentation_mode", "upstream_display") == "raw_f32",
    )


def swap_suits(label, first, second):
    if label[1] == first:
        return label[0] + second
    if label[1] == second:
        return label[0] + first
    return label


def ranges_are_suit_symmetric(case_input, first, second):
    """Whether swapping two suits leaves both input ranges unchanged, weights included."""
    for text in case_input["ranges"]:
        weights = expand_range(text)
        swapped = {}
        for (low, high), weight in weights.items():
            pair = tuple(
                sorted(
                    card_id(swap_suits(card_label(card), first, second))
                    for card in (low, high)
                )
            )
            swapped[pair] = weight
        if swapped != weights:
            return False
    return True


def isomorphic_runout_pairs(case):
    """Exported runouts the reference should have merged, and how far their rows agree.

    The engine deals a merged card by replaying its representative and swapping the suits
    back, so two exported runouts of the same rank whose suits are interchangeable on this
    board must produce policies that map onto each other exactly under that swap. It is the
    one property of the reference's isomorphism that our own runout handling has to respect,
    so it is checked here rather than taken on trust.
    """
    board = case["input"]["board"]
    exported = case["input"]["export_runouts"]
    private = [
        [tuple(sorted(entry["cards"])) for entry in case["private_cards"][player]]
        for player in (0, 1)
    ]
    index = [
        {hand: position for position, hand in enumerate(private[player])}
        for player in (0, 1)
    ]
    nodes = {tuple(node["history_labels"]): node for node in case["nodes"]}
    reports = []
    for first in exported:
        for second in exported:
            if first >= second or first[0] != second[0]:
                continue
            suits = (first[1], second[1])
            if sorted(swap_suits(card, *suits) for card in board) != sorted(board):
                continue
            if not ranges_are_suit_symmetric(case["input"], *suits):
                continue
            compared = 0
            worst = 0.0
            for history, node in nodes.items():
                if node["kind"] != "decision" or node["runout"] != first:
                    continue
                twin = nodes.get(
                    tuple(
                        f"chance:{second}" if step == f"chance:{first}" else step
                        for step in history
                    )
                )
                require(twin is not None, f"{first} has no {second} twin history")
                require(
                    [a["label"] for a in node["actions"]]
                    == [a["label"] for a in twin["actions"]],
                    f"Isomorphic runouts {first} and {second} offer different actions",
                )
                player = node["player"]
                count = len(private[player])
                for hand in private[player]:
                    if first in hand or second in hand:
                        continue
                    mine = index[player][hand]
                    theirs = index[player][
                        tuple(sorted(swap_suits(card, *suits) for card in hand))
                    ]
                    for action in range(len(node["actions"])):
                        worst = max(
                            worst,
                            abs(
                                node["strategy"][action * count + mine]
                                - twin["strategy"][action * count + theirs]
                            ),
                        )
                        compared += 1
            require(
                compared > 0,
                f"Isomorphic runouts {first} and {second} share no comparable rows",
            )
            require(
                worst == 0.0,
                f"Isomorphic runouts {first} and {second} differ by {worst} after the "
                "suit swap; the reference did not merge them as expected",
            )
            reports.append(
                {
                    "runouts": [first, second],
                    "swapped_suits": list(suits),
                    "compared_cells": compared,
                    "max_absolute_difference": worst,
                }
            )
    return reports


def summarize_reference(reference):
    """Reference-only report: what the capture measured, without any project comparison."""
    validate_reference(reference)
    cases = []
    for case in reference["cases"]:
        chance = [node for node in case["nodes"] if node["kind"] == "chance"]
        require(chance, "A turn capture must expose at least one chance node")
        possible = {len(node["possible_cards"]) for node in chance}
        merged = {node["isomorphic_merged_cards"] for node in chance}
        cases.append(
            {
                "id": case["input"]["id"],
                "board": case["input"]["board"],
                "iterations": case["iterations"],
                "stop_reason": case["stop_reason"],
                "target_pct_of_pot": case["input"]["target_pct_of_pot"],
                "exploitability_chips": case["exploitability_chips"],
                "exploitability_pct_of_pot": case["exploitability_pct_of_pot"],
                "reached_target": case["exploitability_pct_of_pot"]
                <= case["input"]["target_pct_of_pot"],
                "root_centered_expected_values": case["root_centered_expected_values"],
                "exported_nodes": len(case["nodes"]),
                "chance_nodes": len(chance),
                "dealable_runouts": sorted(possible),
                "isomorphic_merged_runouts": sorted(merged),
                "exported_runouts": case["input"]["export_runouts"],
                "isomorphic_runout_pairs": isomorphic_runout_pairs(case),
                "private_hand_counts": [
                    len(entries) for entries in case["private_cards"]
                ],
                "reference_memory_estimate_bytes": case[
                    "reference_memory_estimate_bytes"
                ],
                "elapsed_seconds": case["elapsed_seconds"],
            }
        )
    return {
        "schema_version": 1,
        "mode": "reference_only",
        "project_capture": "absent",
        "structural_and_convergence_checks": "passed",
        "note": (
            "No project capture exists yet, so no frequency comparison was made. "
            "The reference solved every runout; only the runouts named in export_runouts "
            "were exported."
        ),
        "cases": cases,
    }


def compare(project, reference):
    finite_tree(project)
    require(project.get("schema_version") == 1, "Unknown project schema")
    require(project.get("street") == "turn", "Project capture is not a turn capture")
    validate_reference(reference)
    ours = indexed(project["cases"], lambda c: c["input"]["id"])
    theirs = indexed(reference["cases"], lambda c: c["input"]["id"])
    require(ours.keys() == theirs.keys(), "Case set differs")
    reports = []
    for name, own in ours.items():
        ref = theirs[name]
        require(own["input"] == ref["input"], f"{name}: input differs")
        pot = own["input"]["starting_pot"]
        br = own["best_response_values"]
        require(
            len(br) == 2
            and abs(50 * max(0, sum(br)) / pot - own["exploitability_pct_of_pot"])
            <= 1e-10,
            "Project best-response metric units differ",
        )
        policy = project.get("execution_stop_policy", "target_or_cap")
        if policy == "fixed_iteration_budget":
            require(
                own["stop_reason"] == policy
                and own["iterations"] == own["input"]["max_iterations"],
                "Project fixed budget is incomplete",
            )
        else:
            require(policy == "target_or_cap", "Unknown project stop policy")
            reason = (
                "TargetReached"
                if own["exploitability_pct_of_pot"] <= own["input"]["target_pct_of_pot"]
                else "IterationCap"
            )
            require(own["stop_reason"] == reason, "Project target/cap reason differs")
            require(
                reason != "IterationCap"
                or own["iterations"] == own["input"]["max_iterations"],
                "Project cap is incomplete",
            )
        expected_mass = compatible_mass(own["input"])
        require(
            abs(own["compatible_weight"] - expected_mass)
            <= 1e-6 * max(1.0, expected_mass),
            "Root compatible mass differs",
        )
        for result in (own, ref):
            require(
                -0.001 <= result["exploitability_pct_of_pot"] < 0.5,
                f"{name}: convergence gate failed",
            )
        pn = indexed(own["nodes"], lambda n: tuple(n["history_labels"]))
        rn = indexed(ref["nodes"], lambda n: tuple(n["history_labels"]))
        require(pn.keys() == rn.keys(), f"{name}: public history set differs")
        differences = []
        checked_rows = 0
        max_frequency_difference = 0.0
        max_available_action_ev_difference = 0.0
        committed_fold_origin_cells = 0
        runout_reports = []
        for history, node in pn.items():
            other = rn[history]
            require(
                node["kind"] == other["kind"], f"{name} {history}: node kind differs"
            )
            require(
                node["street"] == other["street"]
                and (node.get("runout") or None) == (other.get("runout") or None),
                f"{name} {history}: street or runout differs",
            )
            require(
                node["contributions"] == other["contributions"],
                f"{name} {history}: chip contributions differ",
            )
            if node["kind"] == "chance":
                require(
                    sorted(node["possible_cards"]) == sorted(other["possible_cards"]),
                    f"{name} {history}: dealable runout set differs",
                )
                runout_reports.append(
                    {
                        "history": list(history),
                        "dealable_runouts": len(other["possible_cards"]),
                        "reference_representative_actions": other[
                            "representative_action_count"
                        ],
                        "reference_isomorphic_merged": other["isomorphic_merged_cards"],
                        "project_isomorphic_merged": node.get(
                            "isomorphic_merged_cards", 0
                        ),
                    }
                )
                continue
            require(
                node["actions"] == [a["label"] for a in other["actions"]],
                f"{name} {history}: actions differ",
            )
            if node["kind"] == "terminal":
                require(
                    node["terminal"] == other["terminal"], "Terminal reason differs"
                )
                if node["terminal"] == "fold":
                    require(
                        node["fold_winner"] == other["fold_winner"],
                        "Fold winner differs",
                    )
                continue
            player = node["player"]
            require(player == other["player"], "Acting player differs")
            private = ref["private_cards"][player]
            count = len(private)
            indices = indexed(
                enumerate(private), lambda pair: hand_key(pair[1]["cards"])
            )
            hands = indexed(node["hands"], lambda h: hand_key(h["cards"]))
            runout = {node["runout"]} if node.get("runout") else set()
            dead = set(own["input"]["board"]) | runout
            live = {key for key in indices if not set(key).intersection(dead)}
            require(
                hands.keys() == live, f"{name} {history}: physical combo set differs"
            )
            for key, hand in hands.items():
                checked_rows += 1
                index = indices[key][0]
                own_policy = hand["strategy"]
                n = len(node["actions"])
                require(
                    len(own_policy) == n
                    and all(0 <= p <= 1 for p in own_policy)
                    and abs(sum(own_policy) - 1) < 1e-10,
                    "Invalid project policy row",
                )
                ref_policy = [other["strategy"][a * count + index] for a in range(n)]
                delta = [
                    abs(a - b) for a, b in zip(own_policy, ref_policy, strict=True)
                ]
                max_frequency_difference = max(max_frequency_difference, *delta)
                ref_available = other["ev_available"][index]
                available = hand["ev_available"] and ref_available
                ev = hand["action_expected_values"]
                require(
                    len(ev) == (n if hand["ev_available"] else 0),
                    "Project EV availability differs",
                )
                ref_ev = []
                if ref_available:
                    origin = pot / 2 + other["reported_contributions"][player]
                    ref_ev = [
                        other["action_expected_values"][a * count + index] - origin
                        for a in range(n)
                    ]
                    if "fold" in node["actions"]:
                        fold = node["actions"].index("fold")
                        expected = -pot / 2 - node["contributions"][player]
                        if hand["ev_available"]:
                            require(
                                abs(ev[fold] - expected) <= 1e-9,
                                "Project fold EV origin differs",
                            )
                        require(
                            abs(ref_ev[fold] - expected) <= 1e-6,
                            "Reference fold EV origin differs",
                        )
                        if hand["ev_available"] and node["contributions"][player] > 0:
                            committed_fold_origin_cells += 1
                if available:
                    max_available_action_ev_difference = max(
                        max_available_action_ev_difference,
                        *(abs(a - b) for a, b in zip(ev, ref_ev, strict=True)),
                    )
                changed = [a for a, d in enumerate(delta) if d > FREQUENCY_THRESHOLD]
                if changed:
                    differences.append(
                        {
                            "history": list(history),
                            "street": node["street"],
                            "runout": node.get("runout") or None,
                            "player": player,
                            "cards": list(key),
                            "actions": node["actions"],
                            "changed_action_indices": changed,
                            "project_strategy": own_policy,
                            "reference_strategy": ref_policy,
                            "absolute_frequency_difference": delta,
                            "project_action_ev": ev if hand["ev_available"] else None,
                            "reference_action_ev": ref_ev if ref_available else None,
                            "project_action_gap": (
                                [max(ev) - v for v in ev] if ev else None
                            ),
                            "reference_action_gap": (
                                [max(ref_ev) - v for v in ref_ev] if ref_ev else None
                            ),
                            "project_own_reach": hand["own_reach"],
                            "project_opponent_mass": hand["opponent_mass"],
                            "reference_display_own_reach": other["reach_weights"][
                                index
                            ],
                            "both_evs_available": available,
                            "reference_ev_available": ref_available,
                            "project_ev_available": hand["ev_available"],
                            "review_status": "requires_per_combo_review",
                        }
                    )
        root_delta = [
            abs(a - b)
            for a, b in zip(
                own["root_centered_expected_values"],
                ref["root_centered_expected_values"],
                strict=True,
            )
        ]
        reports.append(
            {
                "id": name,
                "matched_nodes": len(pn),
                "matched_policy_rows": checked_rows,
                "committed_fold_origin_cells": committed_fold_origin_cells,
                "project_pct_of_pot": own["exploitability_pct_of_pot"],
                "reference_pct_of_pot": ref["exploitability_pct_of_pot"],
                "project_iterations": own["iterations"],
                "reference_iterations": ref["iterations"],
                "root_ev_absolute_difference": root_delta,
                "max_frequency_difference": max_frequency_difference,
                "max_available_action_ev_difference": max_available_action_ev_difference,
                "frequency_rows_requiring_review": len(differences),
                "chance_nodes": runout_reports,
                "differences": differences,
            }
        )
    require(
        sum(case["committed_fold_origin_cells"] for case in reports) > 0,
        "No raised-history fold established the contribution EV offset",
    )
    return {
        "schema_version": 1,
        "mode": "project_versus_reference",
        "structural_and_convergence_checks": "passed",
        "frequency_review": (
            "required"
            if any(r["differences"] for r in reports)
            else "within_two_percentage_points"
        ),
        "ev_scope": (
            "Action EVs follow each respective policy; root residuals are not per-hand error "
            "bounds. Only the runouts named in export_runouts are compared; both sides solved "
            "every runout."
        ),
        "cases": reports,
    }


def review_index(review):
    """Map (case id, history, cards) to the committed reasoning for that row."""
    require(
        type(review) is dict and type(review.get("cases")) is list,
        "Invalid review file",
    )
    index = {}
    for case in review["cases"]:
        for row in case.get("rows", []):
            key = (case["id"], tuple(row["history"]), tuple(sorted(row["cards"])))
            reasoning = row.get("review_reasoning")
            if type(reasoning) is str and reasoning.strip():
                index[key] = reasoning.strip()
    return index


def uncovered_rows(report, review):
    index = review_index(review) if review is not None else {}
    missing = []
    for case in report["cases"]:
        for row in case["differences"]:
            key = (case["id"], tuple(row["history"]), tuple(sorted(row["cards"])))
            if key not in index:
                missing.append(
                    {"id": case["id"], "history": row["history"], "cards": row["cards"]}
                )
            else:
                row["review_status"] = "reviewed"
                row["review_reasoning"] = index[key]
    return missing


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--project", type=Path)
    parser.add_argument(
        "--review",
        type=Path,
        help="Committed per-combo-review.json; every row over two points must appear in it",
    )
    args = parser.parse_args()
    reference = read_json(args.reference, MAX_OUTPUT_BYTES)
    hashes = {"reference": hashlib.sha256(args.reference.read_bytes()).hexdigest()}
    if args.project is None:
        require(
            args.review is None,
            "A review file is only meaningful with a project capture",
        )
        result = summarize_reference(reference)
        missing = []
    else:
        require(
            args.project.stat().st_size <= MAX_OUTPUT_BYTES,
            "Project file exceeds 64 MiB",
        )
        project = tomllib.loads(args.project.read_text(encoding="utf-8"))
        hashes["project"] = hashlib.sha256(args.project.read_bytes()).hexdigest()
        result = compare(project, reference)
        review = None
        if args.review is not None:
            review = read_json(args.review, MAX_OUTPUT_BYTES)
            hashes["review"] = hashlib.sha256(args.review.read_bytes()).hexdigest()
        missing = uncovered_rows(result, review)
        result["rows_missing_review_reasoning"] = missing
    result["input_sha256"] = hashes
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, allow_nan=False, indent=2)
        stream.write("\n")
    print(
        json.dumps(
            {
                key: value
                for key, value in result.items()
                if key not in {"cases", "input_sha256"}
            }
            | {
                "cases": [
                    {k: v for k, v in case.items() if k != "differences"}
                    for case in result["cases"]
                ]
            },
            indent=2,
        )
    )
    if missing:
        print(
            f"{len(missing)} policy rows exceed {100 * FREQUENCY_THRESHOLD:.0f} percentage "
            "points with no committed review_reasoning",
            file=sys.stderr,
        )
        raise SystemExit(1)


if __name__ == "__main__":
    main()
