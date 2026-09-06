"""Verify captured project policies with independent scalar deal enumeration."""

import argparse
import json
from pathlib import Path
import tomllib

from capture import require
from oracle import Oracle


def verify(payload):
    reports = []
    for case in payload["cases"]:
        oracle = Oracle(case)
        metrics = oracle.metrics()
        for field, expected in (
            ("expected_values", case["root_centered_expected_values"]),
            ("best_response_values", case["best_response_values"]),
        ):
            require(
                all(
                    abs(a - b) <= 1e-9
                    for a, b in zip(metrics[field], expected, strict=True)
                ),
                f"{case['input']['id']}: scalar {field} differs",
            )
        require(
            abs(metrics["pct_of_pot"] - case["exploitability_pct_of_pot"]) <= 1e-9,
            "Scalar residual differs",
        )
        cells = 0
        max_error = 0.0
        for node in case["nodes"]:
            if node["kind"] != "decision":
                continue
            for hand in node["hands"]:
                values = oracle.action_values(node["history_labels"], hand["cards"])
                available = (
                    values["own_history_reach"] > 0
                    and values["compatible_opponent_mass"] > 0
                )
                require(
                    hand["ev_available"] == available,
                    "Conditional EV availability differs",
                )
                for field, measured in (
                    ("own_history_reach", "own_reach"),
                    ("compatible_opponent_mass", "opponent_mass"),
                ):
                    require(
                        abs(values[field] - hand[measured]) <= 1e-9,
                        f"Scalar {measured} differs",
                    )
                if available:
                    errors = [
                        abs(a - b)
                        for a, b in zip(
                            values["counterfactual_action_ev"],
                            hand["action_expected_values"],
                            strict=True,
                        )
                    ]
                    max_error = max(max_error, *errors)
                    require(
                        all(e <= 1e-9 for e in errors),
                        "Scalar conditional action EV differs",
                    )
                    cells += len(errors)
                else:
                    require(
                        hand["action_expected_values"] == [],
                        "Unavailable action EV must be empty",
                    )
        reports.append(
            {
                "id": case["input"]["id"],
                "scalar_metrics": metrics,
                "checked_action_ev_cells": cells,
                "max_action_ev_absolute_error": max_error,
            }
        )
    return {"schema_version": 1, "status": "passed", "cases": reports}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    require(
        args.capture.stat().st_size <= 64 * 1024 * 1024, "Capture exceeds size limit"
    )
    report = verify(tomllib.loads(args.capture.read_text(encoding="utf-8")))
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, allow_nan=False, indent=2)
        stream.write("\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
