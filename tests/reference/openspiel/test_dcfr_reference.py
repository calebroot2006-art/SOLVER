"""The project's DCFR reference must match OpenSpiel's own DCFRSolver."""

import contextlib
import hashlib
import importlib.metadata
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import capture
import dcfr_reference
import pyspiel
from open_spiel.python.algorithms import discounted_cfr, exploitability

from dcfr_reference import ALPHA, BETA, GAMMA, apply_dcfr_discount, make_solver


class DcfrReferenceMatchesUpstream(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if importlib.metadata.version("open-spiel") != "2.0.2":
            raise AssertionError("Expected OpenSpiel 2.0.2")
        source = Path(discounted_cfr.__file__).read_bytes().replace(b"\r\n", b"\n")
        expected = "b101887acc5a57cde1cea2d9fe7218501a32f60cfe982a331e302eaeffb93efb"
        if hashlib.sha256(source).hexdigest() != expected:
            raise AssertionError(
                "Upstream discounted_cfr.py differs from reviewed source"
            )

    def assert_matches(self, ours, upstream, iteration):
        average = ours.average_policy()
        reference_average = upstream.average_policy()
        for key, node in ours._info_state_nodes.items():
            theirs = upstream._info_state_nodes[key]
            mine = ours.current_policy().policy_for_key(key)
            reference = upstream.current_policy().policy_for_key(key)
            for action in node.legal_actions:
                for name, actual, expected in (
                    (
                        "regret",
                        node.cumulative_regret[action],
                        theirs.cumulative_regret[action],
                    ),
                    (
                        "scaled average sum",
                        node.cumulative_policy[action],
                        theirs.cumulative_policy[action] / (iteration + 1) ** GAMMA,
                    ),
                ):
                    self.assertLessEqual(
                        abs(actual - expected),
                        1e-12 + 1e-12 * abs(expected),
                        (iteration, key, action, name),
                    )
                self.assertLessEqual(
                    abs(mine[action] - reference[action]),
                    1e-12,
                    (iteration, key, action, "current"),
                )
                self.assertLessEqual(
                    abs(
                        average.policy_for_key(key)[action]
                        - reference_average.policy_for_key(key)[action]
                    ),
                    1e-12,
                    (iteration, key, action, "average probability"),
                )

    def check(self, game_string, iterations):
        game = pyspiel.load_game(game_string)
        ours = make_solver(game, "dcfr")
        upstream = discounted_cfr.DCFRSolver(game, alpha=ALPHA, beta=BETA, gamma=GAMMA)
        for iteration in range(1, iterations + 1):
            ours.evaluate_and_update_policy()
            apply_dcfr_discount(ours, iteration)
            upstream.evaluate_and_update_policy()
            self.assert_matches(ours, upstream, iteration)
            average = exploitability.nash_conv(game, ours.average_policy())
            expected = exploitability.nash_conv(game, upstream.average_policy())
            self.assertLessEqual(
                abs(average - expected),
                1e-12 + 1e-12 * abs(expected),
                (game_string, iteration, "average nash_conv"),
            )

    def test_kuhn_thirty_iterations(self):
        self.check("kuhn_poker(players=2)", 30)

    def test_leduc_five_iterations(self):
        self.check("leduc_poker(players=2,suit_isomorphism=false)", 5)

    def test_unknown_variant_is_rejected(self):
        with self.assertRaises(ValueError):
            make_solver(pyspiel.load_game("kuhn_poker(players=2)"), "mccfr")

    def test_invalid_discount_iteration_is_rejected_without_mutation(self):
        solver = make_solver(pyspiel.load_game("kuhn_poker(players=2)"), "dcfr")
        for iteration in (0, -1, 1.5, True, float("nan")):
            with self.subTest(iteration=iteration), self.assertRaises(ValueError):
                apply_dcfr_discount(solver, iteration)
        self.assertTrue(
            all(
                not node.cumulative_regret for node in solver._info_state_nodes.values()
            )
        )

    def test_changed_averaging_discount_is_detected(self):
        with mock.patch.object(dcfr_reference, "GAMMA", 1.0):
            with self.assertRaises(AssertionError):
                self.check("kuhn_poker(players=2)", 5)

    def test_changed_average_policy_is_detected(self):
        game = pyspiel.load_game("kuhn_poker(players=2)")
        ours = make_solver(game, "dcfr")
        upstream = discounted_cfr.DCFRSolver(game, alpha=ALPHA, beta=BETA, gamma=GAMMA)
        for iteration in range(1, 4):
            ours.evaluate_and_update_policy()
            apply_dcfr_discount(ours, iteration)
            upstream.evaluate_and_update_policy()
        self.assert_matches(ours, upstream, 3)
        average = ours.average_policy()
        changed = False
        for key, node in ours._info_state_nodes.items():
            row = average.policy_for_key(key)
            low = min(node.legal_actions, key=lambda action: row[action])
            high = max(node.legal_actions, key=lambda action: row[action])
            if row[high] - row[low] > 0.01:
                row[low], row[high] = row[high], row[low]
                changed = True
                break
        self.assertTrue(changed)
        with mock.patch.object(ours, "average_policy", return_value=average):
            with self.assertRaisesRegex(AssertionError, "average probability"):
                self.assert_matches(ours, upstream, 3)

    def test_capture_preserves_original_early_checkpoints(self):
        with tempfile.TemporaryDirectory(
            prefix="astra-reference-capture-"
        ) as temporary:
            output = Path(temporary) / "capture.json"
            for game in ("kuhn", "leduc"):
                for variant in ("cfr", "cfr_plus", "dcfr"):
                    with self.subTest(game=game, variant=variant):
                        arguments = [
                            "capture.py",
                            "--game",
                            game,
                            "--variant",
                            variant,
                            "--max-iterations",
                            "5",
                            "--output",
                            str(output),
                        ]
                        with mock.patch(
                            "sys.argv", arguments
                        ), contextlib.redirect_stdout(io.StringIO()):
                            capture.main()
                        actual = json.loads(output.read_text(encoding="utf-8"))
                        original = json.loads(
                            Path(__file__)
                            .with_name(f"{game}_{variant}.json")
                            .read_text(encoding="utf-8")
                        )
                        self.assertEqual(
                            actual["checkpoints"], original["checkpoints"][:3]
                        )
                        self.assertTrue(actual["complete"])
                        self.assertEqual(
                            actual["executed_local_sources_sha256"][
                                "dcfr_reference.py"
                            ],
                            dcfr_reference.source_sha256(),
                        )


if __name__ == "__main__":
    unittest.main()
