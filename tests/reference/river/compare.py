"""Match measured physical river policies and record every material difference."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import tomllib

from capture import (
    ENGINE_REVISION,
    WASM_REVISION,
    MAX_OUTPUT_BYTES,
    read_json,
    require,
    validate_output,
)


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


def compare(project, reference):
    finite_tree(project)
    require(project.get("schema_version") == 1, "Unknown project schema")
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
    ours = indexed(project["cases"], lambda c: c["input"]["id"])
    theirs = indexed(reference["cases"], lambda c: c["input"]["id"])
    require(ours.keys() == theirs.keys(), "Case set differs")
    validate_output(
        reference,
        {"schema_version": 1, "cases": [c["input"] for c in reference["cases"]]},
        reference.get("execution_stop_policy", "target_or_cap")
        == "fixed_iteration_budget",
    )
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
        pairs = sum(
            not set(a["cards"]).intersection(b["cards"])
            for a in ref["private_cards"][0]
            for b in ref["private_cards"][1]
        )
        require(own["compatible_weight"] == pairs, "Root compatible mass differs")
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
        for history, node in pn.items():
            other = rn[history]
            require(
                node["kind"] == other["kind"], f"{name} {history}: node kind differs"
            )
            require(
                node["contributions"] == other["contributions"],
                f"{name} {history}: chip contributions differ",
            )
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
            live = {
                key
                for key in indices
                if not set(key).intersection(own["input"]["board"])
            }
            require(
                hands.keys() == live, f"{name} {history}: physical combo set differs"
            )
            for key, hand in hands.items():
                checked_rows += 1
                index = indices[key][0]
                policy = hand["strategy"]
                n = len(node["actions"])
                require(
                    len(policy) == n
                    and all(0 <= p <= 1 for p in policy)
                    and abs(sum(policy) - 1) < 1e-10,
                    "Invalid project policy row",
                )
                ref_policy = [other["strategy"][a * count + index] for a in range(n)]
                delta = [abs(a - b) for a, b in zip(policy, ref_policy, strict=True)]
                max_frequency_difference = max(max_frequency_difference, *delta)
                available = (
                    hand["ev_available"] and other["ev_available"][player][index]
                )
                ev = hand["action_expected_values"]
                require(
                    len(ev) == (n if hand["ev_available"] else 0),
                    "Project EV availability differs",
                )
                ref_ev = []
                if available:
                    origin = pot / 2 + other["reported_contributions"][player]
                    ref_ev = [
                        other["action_expected_values"][a * count + index] - origin
                        for a in range(n)
                    ]
                    if "fold" in node["actions"]:
                        fold = node["actions"].index("fold")
                        expected = -pot / 2 - node["contributions"][player]
                        require(
                            abs(ev[fold] - expected) <= 1e-9,
                            "Project fold EV origin differs",
                        )
                        require(
                            abs(ref_ev[fold] - expected) <= 1e-6,
                            "Reference fold EV origin differs",
                        )
                        if node["contributions"][player] > 0:
                            committed_fold_origin_cells += 1
                    max_available_action_ev_difference = max(
                        max_available_action_ev_difference,
                        *(abs(a - b) for a, b in zip(ev, ref_ev, strict=True)),
                    )
                changed = [a for a, difference in enumerate(delta) if difference > 0.02]
                if changed:
                    differences.append(
                        {
                            "history": list(history),
                            "player": player,
                            "cards": list(key),
                            "actions": node["actions"],
                            "changed_action_indices": changed,
                            "project_strategy": policy,
                            "reference_strategy": ref_policy,
                            "absolute_frequency_difference": delta,
                            "project_action_ev": ev if hand["ev_available"] else None,
                            "reference_action_ev": ref_ev if available else None,
                            "project_action_gap": (
                                [max(ev) - v for v in ev] if ev else None
                            ),
                            "reference_action_gap": (
                                [max(ref_ev) - v for v in ref_ev] if ref_ev else None
                            ),
                            "project_own_reach": hand["own_reach"],
                            "project_opponent_mass": hand["opponent_mass"],
                            "reference_display_own_reach": other["reach_weights"][
                                player
                            ][index],
                            "both_evs_available": available,
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
        if name.startswith("diagnostic_"):
            # These AA-versus-QQ boards are deterministic; they establish the EV
            # origin independently of convergence or equilibrium nonuniqueness.
            require(
                all(
                    abs(a - b) <= 0.001
                    for a, b in zip(
                        own["root_centered_expected_values"], [5, -5], strict=True
                    )
                ),
                "Project diagnostic payoff is wrong",
            )
            require(
                all(v <= 0.001 for v in root_delta),
                "Reference diagnostic EV origin differs",
            )
            require(
                max_available_action_ev_difference <= 0.001,
                "Diagnostic action EV origin differs",
            )
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
                "differences": differences,
            }
        )
    if any(not case["id"].startswith("diagnostic_") for case in reports):
        require(
            sum(case["committed_fold_origin_cells"] for case in reports) > 0,
            "No raised-history fold established the contribution EV offset",
        )
    return {
        "schema_version": 1,
        "structural_and_convergence_checks": "passed",
        "frequency_review": (
            "required"
            if any(r["differences"] for r in reports)
            else "within_two_percentage_points"
        ),
        "ev_scope": "Action EVs follow each respective policy; root residuals are not per-hand error bounds.",
        "cases": reports,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project", type=Path)
    parser.add_argument("reference", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    require(
        args.project.stat().st_size <= MAX_OUTPUT_BYTES, "Project file exceeds 64 MiB"
    )
    project = tomllib.loads(args.project.read_text(encoding="utf-8"))
    reference = read_json(args.reference, MAX_OUTPUT_BYTES)
    result = compare(project, reference)
    result["input_sha256"] = {
        "project": hashlib.sha256(args.project.read_bytes()).hexdigest(),
        "reference": hashlib.sha256(args.reference.read_bytes()).hexdigest(),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, allow_nan=False, indent=2)
        stream.write("\n")
    print(
        json.dumps(
            {
                "frequency_review": result["frequency_review"],
                "cases": [
                    {k: v for k, v in r.items() if k != "differences"}
                    for r in result["cases"]
                ],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
