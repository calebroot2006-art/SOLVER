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
VARIANTS = ("cfr", "cfr_plus", "dcfr")
REQUIRED_ITERATIONS = (
    0,
    1,
    2,
    5,
    10,
    20,
    50,
    51,
    100,
    101,
    200,
    201,
    500,
    1000,
    1001,
    2000,
    5000,
    10000,
)
EXPECTED_CFR_LF_SHA256 = (
    "56f8f472a83166c4e3312016e709d7ece1a78ba34ca1684d842a26cbc8d89fc7"
)
GAME_PARAMETERS = {
    "players": 2,
    "suit_isomorphism": False,
    "starting_player": 0,
    "action_mapping": False,
}


def cfr_source_hashes():
    source = Path(cfr.__file__).read_bytes()
    return {
        "raw_sha256": hashlib.sha256(source).hexdigest(),
        "canonical_lf_sha256": hashlib.sha256(
            source.replace(b"\r\n", b"\n")
        ).hexdigest(),
    }


def checked_game():
    if importlib.metadata.version("open-spiel") != "2.0.2":
        raise ValueError("Reference version must be OpenSpiel 2.0.2")
    if cfr_source_hashes()["canonical_lf_sha256"] != EXPECTED_CFR_LF_SHA256:
        raise ValueError(
            "Reference cfr.py source hash differs from the reviewed capture"
        )
    game = pyspiel.load_game("leduc_poker(players=2,suit_isomorphism=false)")
    if dict(game.get_parameters()) != GAME_PARAMETERS:
        raise ValueError(
            "Reference game parameters differ from the reviewed Leduc game"
        )
    return game


def validate_directory(directory):
    expected = set()
    for variant in VARIANTS:
        for iteration in REQUIRED_ITERATIONS:
            stem = f"leduc_{variant}_{iteration:04}"
            expected.update([stem + ".csv", stem + ".metrics.csv"])
    actual = {path.name for path in directory.glob("*.csv")}
    if actual != expected:
        raise ValueError(
            f"Snapshot set mismatch; missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}"
        )


def read_metrics(path):
    with path.with_suffix(".metrics.csv").open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    if len(rows) != 2 or {row["profile"] for row in rows} != {"current", "average"}:
        raise ValueError("Exactly one current and one average metric row is required")
    expected_iteration = int(path.stem.rsplit("_", 1)[1])
    result = {}
    metadata = {
        "schema_version": "1",
        "game": "leduc_poker",
        "players": "2",
        "suit_isomorphism": "false",
        "starting_player": "0",
        "action_mapping": "false",
        "iteration": str(expected_iteration),
    }
    for row in rows:
        if any(row.get(key) != value for key, value in metadata.items()):
            raise ValueError(
                "Snapshot game, action mapping, schema or iteration metadata differs"
            )
        values = {
            name: float(row[name])
            for name in ["player_0_value", "br0", "br1", "nash_conv"]
        }
        if not all(math.isfinite(value) for value in values.values()):
            raise ValueError("Nonfinite Rust metric")
        result[row["profile"]] = values
    return result


def read_snapshot(path):
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    result = {}
    expected_iteration = int(path.stem.rsplit("_", 1)[1])
    for row in rows:
        if int(row["iteration"]) != expected_iteration:
            raise ValueError("Snapshot row iteration differs from its filename")
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
    game = checked_game()
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
    rust_metrics = read_metrics(path)
    for name, policy in [
        ("current", solver.current_policy()),
        ("average", solver.average_policy()),
    ]:
        utilities = expected_game_score.policy_value(
            game.new_initial_state(), [policy, policy]
        )
        residual = exploitability.nash_conv(game, policy, return_only_nash_conv=False)
        result[name] = {
            "nash_conv": float(residual.nash_conv),
            "player_0_value": float(utilities[0]),
            "br0": float(residual.player_improvements[0] + utilities[0]),
            "br1": float(residual.player_improvements[1] + utilities[1]),
        }
        for metric, value in result[name].items():
            if (
                not math.isfinite(value)
                or abs(value - rust_metrics[name][metric]) > 1e-12
            ):
                raise AssertionError(
                    f"Independent metric mismatch {path.name} {name} {metric}: OpenSpiel={value}, Rust={rust_metrics[name][metric]}"
                )
        budget, target = {
            "cfr": (10000, 0.005),
            "cfr_plus": (1000, 0.001),
            "dcfr": (2000, 0.0001),
        }[variant]
        iteration = int(path.stem.rsplit("_", 1)[1])
        if (
            name == "average"
            and iteration >= budget
            and result[name]["nash_conv"] >= target
        ):
            raise AssertionError(
                f"Absolute accuracy target missed by {path.name}: {result[name]['nash_conv']} >= {target}"
            )
    return {
        "snapshot": path.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "metrics_sha256": hashlib.sha256(
            path.with_suffix(".metrics.csv").read_bytes()
        ).hexdigest(),
        "rust_metrics": rust_metrics,
        **result,
    }


def verify(directory, variant, iteration):
    before = read_snapshot(directory / f"leduc_{variant}_{iteration:04}.csv")
    after = read_snapshot(directory / f"leduc_{variant}_{iteration + 1:04}.csv")
    game = checked_game()
    solver = (cfr.CFRPlusSolver if variant == "cfr_plus" else cfr.CFRSolver)(game)
    if len(before) != len(solver._info_state_nodes) or before.keys() != after.keys():
        raise ValueError("Snapshot information sets do not match the official game")
    for key, node in solver._info_state_nodes.items():
        coordinate = coordinates(key)
        data = before[coordinate]
        if set(data) != set(node.legal_actions):
            raise ValueError(f"Legal action mismatch: {coordinate}")
        if abs(sum(row["current"] for row in data.values()) - 1) > 1e-12:
            raise ValueError(f"Unnormalized current strategy at {coordinate}")
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
    failures = []
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
                tolerance = 1e-12 if name == "current" else 1e-12 + 1e-12 * abs(value)
                if not math.isfinite(value) or abs(value - row[name]) > tolerance:
                    failures.append(
                        f"{coordinate} action={action} {name}: OpenSpiel={value}, Rust={row[name]}, tolerance={tolerance}"
                    )
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
    if failures:
        raise AssertionError(
            "Shared-state replay mismatch: " + "\n".join(failures[:10])
        )
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
    checked_game()
    validate_directory(args.directory)
    results = []
    for variant in VARIANTS:
        for iteration in [0, 1, 50, 100, 200, 1000]:
            result = verify(args.directory, variant, iteration)
            results.append(result)
            print(variant, iteration, result["max_absolute_difference"], flush=True)
    actual_metrics = []
    for variant in VARIANTS:
        for path in sorted(args.directory.glob(f"leduc_{variant}_*.csv")):
            # The vanilla glob also matches cfr_plus; require a numeric suffix.
            if not path.stem.removeprefix(f"leduc_{variant}_").isdigit():
                continue
            result = evaluate_snapshot(path, variant)
            actual_metrics.append(result)
            print(path.name, "actual average", result["average"], flush=True)
    source_hashes = cfr_source_hashes()
    output = {
        "open_spiel_version": importlib.metadata.version("open-spiel"),
        "executed_upstream_cfr_sha256": source_hashes["raw_sha256"],
        "executed_upstream_cfr_canonical_lf_sha256": source_hashes[
            "canonical_lf_sha256"
        ],
        "executed_verifier_sha256": hashlib.sha256(
            Path(__file__).read_bytes()
        ).hexdigest(),
        "one_step_comparisons": results,
        "actual_policy_metrics": actual_metrics,
        "verified": True,
        "tolerances": {
            "replay_accumulators_absolute": 1e-12,
            "replay_accumulators_relative": 1e-12,
            "current_policy_absolute": 1e-12,
            "independent_metrics_absolute": 1e-12,
        },
    }
    args.output.write_text(
        json.dumps(output, indent=2) + "\n", encoding="utf-8", newline="\n"
    )


if __name__ == "__main__":
    main()
