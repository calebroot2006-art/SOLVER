"""Capture the Python OpenSpiel CFR reference without using the Rust solver."""

import argparse
import hashlib
import importlib.metadata
import json
import platform
from datetime import datetime, timezone
from pathlib import Path
from time import perf_counter

import pyspiel
from open_spiel.python.algorithms import cfr, expected_game_score, exploitability

from dcfr_reference import ALPHA, BETA, GAMMA, apply_dcfr_discount, make_solver
from dcfr_reference import source_sha256 as dcfr_source_sha256

CHECKPOINTS = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000]
GAMES = {
    "kuhn": "kuhn_poker(players=2)",
    "leduc": "leduc_poker(players=2,suit_isomorphism=false)",
}


def main():
    """Write each checkpoint immediately, retaining progress if interrupted."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", required=True, choices=GAMES)
    parser.add_argument("--variant", required=True, choices=["cfr", "cfr_plus", "dcfr"])
    parser.add_argument("--max-iterations", type=int, default=10000)
    parser.add_argument("--output", type=Path, help="Override the capture destination")
    args = parser.parse_args()
    if args.max_iterations < 1:
        parser.error("max-iterations must be positive")
    version = importlib.metadata.version("open-spiel")
    if version != "2.0.2":
        raise RuntimeError(f"Expected pinned OpenSpiel 2.0.2, found {version}")
    game = pyspiel.load_game(GAMES[args.game])
    solver = make_solver(game, args.variant)
    constructor = type(solver)
    source = Path(cfr.__file__).read_bytes()
    payload = {
        "game": GAMES[args.game],
        "game_parameters": dict(game.get_parameters()),
        "variant": args.variant,
        "open_spiel_version": version,
        "python_version": platform.python_version(),
        "platform": platform.platform(),
        "captured_utc": datetime.now(timezone.utc).isoformat(),
        "implementation": f"{constructor.__module__}.{constructor.__name__}",
        "cfr_python_sha256": hashlib.sha256(source).hexdigest(),
        "executed_local_sources_sha256": {
            "capture.py": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "dcfr_reference.py": dcfr_source_sha256(),
        },
        "iteration_definition": "one evaluate_and_update_policy; players alternate 0 then 1",
        "averaging": "linear iteration t" if args.variant == "cfr_plus" else "uniform",
        "units": "chips per hand; nash_conv is sum of both best-response gains",
        "checkpoints": [],
        "complete": False,
    }
    if args.variant == "dcfr":
        payload["discount_extension"] = {"alpha": ALPHA, "beta": BETA, "gamma": GAMMA}
        payload[
            "implementation"
        ] += " + project whole-accumulator DCFR discount extension"
        payload["averaging"] = "discount whole accumulated sum by (t/(t+1))**2"
    output = args.output or Path(__file__).parent / f"{args.game}_{args.variant}.json"
    started = perf_counter()
    for iteration in range(1, args.max_iterations + 1):
        solver.evaluate_and_update_policy()
        if args.variant == "dcfr":
            apply_dcfr_discount(solver, iteration)
        if iteration not in CHECKPOINTS and iteration != args.max_iterations:
            continue
        policy = solver.average_policy()
        residual = float(exploitability.nash_conv(game, policy))
        value = float(
            expected_game_score.policy_value(
                game.new_initial_state(), [policy, policy]
            )[0]
        )
        point = {
            "iteration": iteration,
            "nash_conv": residual,
            "player_0_value": value,
        }
        payload["checkpoints"].append(point)
        payload["elapsed_seconds"] = perf_counter() - started
        payload["complete"] = iteration == args.max_iterations
        output.write_text(
            json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n"
        )
        print(
            args.game,
            args.variant,
            point,
            "seconds",
            round(payload["elapsed_seconds"], 3),
            flush=True,
        )


if __name__ == "__main__":
    main()
