"""Measure Leduc accumulation order and export early information-set traces."""

import argparse
import csv
import hashlib
import importlib.metadata
import json
import math
import re
from pathlib import Path

import pyspiel
from open_spiel.python.algorithms import cfr, expected_game_score, exploitability


class ReverseChance:
    """Preserve every history and probability but reverse chance traversal order."""

    def __init__(self, state):
        self.state = state

    def __getattr__(self, name):
        return getattr(self.state, name)

    def child(self, action):
        return ReverseChance(self.state.child(action))

    def chance_outcomes(self):
        return list(reversed(self.state.chance_outcomes()))


def coordinates(key):
    """Map the official textual information state to public-history coordinates."""
    fields = dict(re.findall(r"\[([^:\]]+): ([^\]]*)\]", key))
    actions = {"0": "f", "1": "c", "2": "r"}
    history = "".join(actions[a] for a in fields["Round1"].split())
    if "Public" in fields:
        history += "/" + fields["Public"] + "/"
        history += "".join(actions[a] for a in fields["Round2"].split())
    return history, int(fields["Observer"]), int(fields["Private"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", choices=["cfr", "cfr_plus", "dcfr"], required=True)
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--reverse-chance", action="store_true")
    parser.add_argument("--root-counterfactual-scale", type=float, default=1.0)
    parser.add_argument("--trace-through", type=int, default=0)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if (
        not math.isfinite(args.root_counterfactual_scale)
        or args.root_counterfactual_scale <= 0
    ):
        parser.error("root-counterfactual-scale must be finite and positive")
    game = pyspiel.load_game("leduc_poker(players=2,suit_isomorphism=false)")
    solver = (cfr.CFRPlusSolver if args.variant == "cfr_plus" else cfr.CFRSolver)(game)
    if args.reverse_chance:
        solver._root_node = ReverseChance(solver._root_node)
    original_traversal = solver._compute_counterfactual_regret_for_player

    def scaled_traversal(state, policies, reach_probabilities, player):
        if not state.history():
            reach_probabilities = reach_probabilities.copy()
            reach_probabilities[-1] *= args.root_counterfactual_scale
        return original_traversal(state, policies, reach_probabilities, player)

    solver._compute_counterfactual_regret_for_player = scaled_traversal
    metadata = {
        "variant": args.variant,
        "reverse_chance": args.reverse_chance,
        "root_counterfactual_scale": args.root_counterfactual_scale,
        "game_parameters": dict(game.get_parameters()),
        "open_spiel_version": importlib.metadata.version("open-spiel"),
        "executed_upstream_cfr_sha256": hashlib.sha256(
            Path(cfr.__file__).read_bytes()
        ).hexdigest(),
        "executed_sensitivity_script_sha256": hashlib.sha256(
            Path(__file__).read_bytes()
        ).hexdigest(),
    }
    rows = []
    checkpoints = []
    for iteration in range(1, args.iterations + 1):
        solver.evaluate_and_update_policy()
        if args.variant == "dcfr":
            positive = iteration**1.5 / (iteration**1.5 + 1)
            for node in solver._info_state_nodes.values():
                for action, value in node.cumulative_regret.items():
                    node.cumulative_regret[action] *= positive if value > 0 else 0.5
                for action in node.cumulative_policy:
                    node.cumulative_policy[action] *= (iteration / (iteration + 1)) ** 2
        if iteration <= args.trace_through:
            for key, node in solver._info_state_nodes.items():
                history, player, hand = coordinates(key)
                current = solver.current_policy().policy_for_key(key)
                multiplicity = 4 if "/" in history else 5
                for action in node.legal_actions:
                    rows.append(
                        [
                            iteration,
                            history,
                            player,
                            hand,
                            "fcr"[action],
                            node.cumulative_regret[action]
                            * 30
                            / args.root_counterfactual_scale,
                            current[action],
                            node.cumulative_policy[action] / multiplicity,
                        ]
                    )
        if iteration in [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, args.iterations]:
            policy = solver.average_policy()
            point = {
                "iteration": iteration,
                "nash_conv": float(exploitability.nash_conv(game, policy)),
                "player_0_value": float(
                    expected_game_score.policy_value(
                        game.new_initial_state(), [policy, policy]
                    )[0]
                ),
            }
            checkpoints.append(point)
            args.output.write_text(
                json.dumps(
                    {**metadata, "checkpoints": checkpoints},
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
                newline="\n",
            )
            print(
                args.variant, "reverse=" + str(args.reverse_chance), point, flush=True
            )
    if rows:
        with args.output.with_suffix(".csv").open(
            "w", encoding="utf-8", newline=""
        ) as stream:
            writer = csv.writer(stream, lineterminator="\n")
            writer.writerow(
                [
                    "iteration",
                    "history",
                    "player",
                    "hand",
                    "action",
                    "regret",
                    "current",
                    "strategy_sum",
                ]
            )
            writer.writerows(rows)


if __name__ == "__main__":
    main()
