"""Hand-ranking and information-boundary regressions for the scalar reference."""

import copy
from pathlib import Path
import tomllib
import unittest

from oracle import Oracle, expand, five, seven
from review_combos import evidence
from verify_project import verify


class ScalarOracle(unittest.TestCase):
    def test_invalid_exported_probabilities_are_rejected(self):
        path = Path(__file__).parent / "measured/82f8f4c/project-linux-diagnostics.toml"
        original = tomllib.loads(path.read_text())["cases"][1]
        for row in ([-0.25, 1.25], [float("nan"), 1], [float("inf"), 0], [1]):
            case = copy.deepcopy(original)
            case["nodes"][0]["hands"][0]["strategy"] = row
            with self.assertRaisesRegex(ValueError, "probability row"):
                Oracle(case)

    def test_exported_reach_fields_are_verified(self):
        path = Path(__file__).parent / "measured/82f8f4c/project-linux-diagnostics.toml"
        original = tomllib.loads(path.read_text())
        for field in ("own_reach", "opponent_mass"):
            payload = copy.deepcopy(original)
            payload["cases"][0]["nodes"][0]["hands"][0][field] += 0.01
            with self.assertRaisesRegex(ValueError, field):
                verify(payload)

    def test_wheel_flush_full_house_and_seven_card_selection(self):
        wheel = tuple("As 2s 3s 4s 5s".split())
        six_high = tuple("2h 3h 4h 5h 6h".split())
        self.assertGreater(five(six_high), five(wheel))
        self.assertGreater(five(wheel), five(tuple("Ac Ad Ah As Kc".split())))
        self.assertGreater(
            five(tuple("Ac Ad Ah Ks Kc".split())), five(tuple("Kc Kd Kh As Ac".split()))
        )
        self.assertEqual(seven(wheel + ("Kd", "Qh")), five(wheel))

    def test_response_choice_cannot_see_the_opponents_private_hand(self):
        board = "2c 3d 7h 9s Tc".split()
        case = {
            "input": {"board": board, "ranges": ["KK", "AA,QQ"], "starting_pot": 10},
            "nodes": [
                {
                    "history_labels": [],
                    "kind": "decision",
                    "player": 0,
                    "actions": ["fold", "call"],
                    "contributions": [0, 5],
                    "hands": [
                        {"cards": h, "strategy": [0.5, 0.5]}
                        for h in expand("KK", board)
                    ],
                },
                {
                    "history_labels": ["fold"],
                    "kind": "terminal",
                    "terminal": "fold",
                    "fold_winner": 1,
                    "contributions": [0, 0],
                },
                {
                    "history_labels": ["call"],
                    "kind": "terminal",
                    "terminal": "showdown",
                    "contributions": [5, 5],
                },
            ],
        }
        oracle = Oracle(case)
        self.assertEqual(oracle.value(0), -2.5)
        self.assertEqual(oracle.value(0, True), 0)
        # A forbidden per-villain choice would fold to AA and call QQ for +2.5.
        self.assertNotEqual(oracle.value(0, True), 2.5)

    def test_actual_project_metrics_and_all_available_action_evs(self):
        root = Path(__file__).parent / "measured" / "82f8f4c"
        for platform in ("linux", "windows"):
            for group in ("diagnostics", "cases"):
                payload = tomllib.loads(
                    (root / f"project-{platform}-{group}.toml").read_text()
                )
                self.assertEqual(verify(payload)["status"], "passed")

    def test_single_information_set_gain_matches_a_full_policy_deviation(self):
        path = Path(__file__).parent / "measured/82f8f4c/project-linux-cases.toml"
        case = tomllib.loads(path.read_text())["cases"][2]
        oracle = Oracle(case)
        history = ("check", "bet:5")
        cards = next(iter(oracle.rows[history]))
        original = oracle.rows[history][cards]
        detail = evidence(oracle, history, cards, original)
        values = detail["counterfactual_action_ev"]
        best = values.index(max(values))
        player = oracle.nodes[history]["player"]
        before = oracle.value(player)
        oracle.rows[history][cards] = [float(a == best) for a in range(len(values))]
        actual = oracle.value(player) - before
        self.assertAlmostEqual(actual, detail["root_best_action_gain_chips"], places=12)


if __name__ == "__main__":
    unittest.main()
