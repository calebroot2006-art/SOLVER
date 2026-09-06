"""Advance OpenSpiel once from an exact Rust snapshot and compare the next state."""

import argparse
import csv
import hashlib
import importlib.metadata
import json
import math
from pathlib import Path

import pyspiel
from open_spiel.python.algorithms import cfr, expected_game_score, exploitability

from sensitivity import coordinates

ACTION = {"f": 0, "c": 1, "r": 2}


def read_snapshot(path):
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    result = {}
    for row in rows:
        key = (row["history"], int(row["player"]), int(row["hand"]))
        action = ACTION[row["action"]]
        if action in result.get(key, {}):
            raise ValueError(f"Duplicate information-set action: {key}, {action}")
        values = {
            name: float(row[name]) for name in ["regret", "current", "strategy_sum"]
        }
        if not all(math.isfinite(value) for value in values.values()):
            raise ValueError(f"Nonfinite snapshot value at {key}, {action}")
        if not 0 <= values["current"] <= 1 or values["strategy_sum"] < 0:
            raise ValueError(f"Invalid strategy probability or accumulator at {key}")
        result.setdefault(key, {})[action] = values
    return result


def evaluate_snapshot(path, variant):
    """Evaluate the actual Rust strategy, without advancing or recomputing regrets."""
    snapshot = read_snapshot(path)
    game = pyspiel.load_game("leduc_poker(players=2,suit_isomorphism=false)")
    solver = (cfr.CFRPlusSolver if variant == "cfr_plus" else cfr.CFRSolver)(game)
    if len(snapshot) != len(solver._info_state_nodes):
        raise ValueError("Snapshot information-set count does not match OpenSpiel")
    for key, node in solver._info_state_nodes.items():
        coordinate = coordinates(key)
        data = snapshot[coordinate]
        if set(data) != set(node.legal_actions):
            raise ValueError(f"Legal action mismatch: {coordinate}")
        current = solver.current_policy().policy_for_key(key)
        if abs(sum(row["current"] for row in data.values()) - 1) > 1e-12:
            raise ValueError(f"Unnormalized current strategy at {coordinate}")
        for action, row in data.items():
            current[action] = row["current"]
            # A common factor per information set cancels when averaging.
            node.cumulative_policy[action] = row["strategy_sum"]
    result = {}
    for name, policy in [
        ("current", solver.current_policy()),
        ("average", solver.average_policy()),
    ]:
        result[name] = {
            "nash_conv": float(exploitability.nash_conv(game, policy)),
            "player_0_value": float(
                expected_game_score.policy_value(
                    game.new_initial_state(), [policy, policy]
                )[0]
            ),
        }
    return {
        "snapshot": path.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        **result,
    }


def verify(directory, variant, iteration):
    before = read_snapshot(directory / f"leduc_{variant}_{iteration:04}.csv")
    after = read_snapshot(directory / f"leduc_{variant}_{iteration + 1:04}.csv")
    game = pyspiel.load_game("leduc_poker(players=2,suit_isomorphism=false)")
    solver = (cfr.CFRPlusSolver if variant == "cfr_plus" else cfr.CFRSolver)(game)
    if len(before) != len(solver._info_state_nodes) or before.keys() != after.keys():
        raise ValueError("Snapshot information sets do not match the official game")
    for key, node in solver._info_state_nodes.items():
        coordinate = coordinates(key)
        data = before[coordinate]
        if set(data) != set(node.legal_actions):
            raise ValueError(f"Legal action mismatch: {coordinate}")
        multiplicity = 4 if "/" in coordinate[0] else 5
        policy = solver.current_policy().policy_for_key(key)
        for action, row in data.items():
            node.cumulative_regret[action] = row["regret"] / 30
            node.cumulative_policy[action] = row["strategy_sum"] * multiplicity
            policy[action] = row["current"]
    solver._iteration = iteration
    solver.evaluate_and_update_policy()
    if variant == "dcfr":
        t = iteration + 1
        positive = t**1.5 / (t**1.5 + 1)
        for node in solver._info_state_nodes.values():
            for action, regret in node.cumulative_regret.items():
                node.cumulative_regret[action] *= positive if regret > 0 else 0.5
            for action in node.cumulative_policy:
                node.cumulative_policy[action] *= (t / (t + 1)) ** 2
    maxima = {"regret": 0.0, "current": 0.0, "strategy_sum": 0.0}
    worst = []
    for key, node in solver._info_state_nodes.items():
        coordinate = coordinates(key)
        data = after[coordinate]
        multiplicity = 4 if "/" in coordinate[0] else 5
        policy = solver.current_policy().policy_for_key(key)
        comparison = []
        for action, row in data.items():
            reference = {
                "regret": node.cumulative_regret[action] * 30,
                "current": policy[action],
                "strategy_sum": node.cumulative_policy[action] / multiplicity,
            }
            for name, value in reference.items():
                maxima[name] = max(maxima[name], abs(value - row[name]))
            comparison.append(
                {
                    "action": "fcr"[action],
                    "before": before[coordinate][action],
                    "rust_after": row,
                    "openspiel_after": reference,
                }
            )
        difference = max(
            abs(x["rust_after"]["current"] - x["openspiel_after"]["current"])
            for x in comparison
        )
        if difference > 1e-12:
            worst.append(
                {
                    "history": coordinate[0],
                    "player": coordinate[1],
                    "hand": coordinate[2],
                    "policy_difference": difference,
                    "actions": comparison,
                }
            )
    worst.sort(key=lambda x: x["policy_difference"], reverse=True)
    return {
        "variant": variant,
        "before_iteration": iteration,
        "after_iteration": iteration + 1,
        "information_sets": len(before),
        "max_absolute_difference": maxima,
        "policy_rows_over_1e_minus_12": len(worst),
        "worst_policy_rows": worst[:5],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    results = []
    for variant in ["cfr", "cfr_plus", "dcfr"]:
        for iteration in [0, 1, 50, 100, 200, 1000]:
            result = verify(args.directory, variant, iteration)
            results.append(result)
            print(variant, iteration, result["max_absolute_difference"], flush=True)
    actual_metrics = []
    for variant in ["cfr", "cfr_plus", "dcfr"]:
        for path in sorted(args.directory.glob(f"leduc_{variant}_*.csv")):
            # The vanilla glob also matches cfr_plus; require a numeric suffix.
            if not path.stem.removeprefix(f"leduc_{variant}_").isdigit():
                continue
            result = evaluate_snapshot(path, variant)
            actual_metrics.append(result)
            print(path.name, "actual average", result["average"], flush=True)
    output = {
        "open_spiel_version": importlib.metadata.version("open-spiel"),
        "executed_upstream_cfr_sha256": hashlib.sha256(
            Path(cfr.__file__).read_bytes()
        ).hexdigest(),
        "executed_verifier_sha256": hashlib.sha256(
            Path(__file__).read_bytes()
        ).hexdigest(),
        "one_step_comparisons": results,
        "actual_policy_metrics": actual_metrics,
    }
    args.output.write_text(
        json.dumps(output, indent=2) + "\n", encoding="utf-8", newline="\n"
    )


if __name__ == "__main__":
    main()
