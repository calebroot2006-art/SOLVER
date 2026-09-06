"""Mutations must fail the independent snapshot acceptance checks."""

import csv
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify_snapshots as verifier
from open_spiel.python.algorithms import cfr, expected_game_score, exploitability
from sensitivity import coordinates


def write_fixture(directory, solver, iteration):
    path = directory / f"leduc_cfr_{iteration:04}.csv"
    with path.open("w", encoding="utf-8", newline="") as stream:
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
        for key, node in solver._info_state_nodes.items():
            history, player, hand = coordinates(key)
            current = solver.current_policy().policy_for_key(key)
            multiplicity = 4 if "/" in history else 5
            for action in node.legal_actions:
                writer.writerow(
                    [
                        iteration,
                        history,
                        player,
                        hand,
                        "fcr"[action],
                        node.cumulative_regret[action] * 30,
                        current[action],
                        node.cumulative_policy[action] / multiplicity,
                    ]
                )
    game = verifier.checked_game()
    with path.with_suffix(".metrics.csv").open(
        "w", encoding="utf-8", newline=""
    ) as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(
            [
                "schema_version",
                "game",
                "players",
                "suit_isomorphism",
                "starting_player",
                "action_mapping",
                "iteration",
                "profile",
                "player_0_value",
                "br0",
                "br1",
                "nash_conv",
            ]
        )
        for name, policy in [
            ("current", solver.current_policy()),
            ("average", solver.average_policy()),
        ]:
            values = expected_game_score.policy_value(
                game.new_initial_state(), [policy, policy]
            )
            residual = exploitability.nash_conv(
                game, policy, return_only_nash_conv=False
            )
            writer.writerow(
                [
                    1,
                    "leduc_poker",
                    2,
                    "false",
                    0,
                    "false",
                    iteration,
                    name,
                    values[0],
                    residual.player_improvements[0] + values[0],
                    residual.player_improvements[1] + values[1],
                    residual.nash_conv,
                ]
            )


def mutate_field(path, field, transform):
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream)
        fields, rows = reader.fieldnames, list(reader)
    rows[0][field] = transform(rows[0][field])
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


class SnapshotAcceptanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.storage = tempfile.TemporaryDirectory(prefix="astra-snapshot-baseline-")
        cls.baseline = Path(cls.storage.name)
        solver = cfr.CFRSolver(verifier.checked_game())
        write_fixture(cls.baseline, solver, 0)
        solver.evaluate_and_update_policy()
        write_fixture(cls.baseline, solver, 1)

    @classmethod
    def tearDownClass(cls):
        cls.storage.cleanup()

    def setUp(self):
        self.storage = tempfile.TemporaryDirectory(prefix="astra-snapshot-mutation-")
        self.directory = Path(self.storage.name) / "capture"
        shutil.copytree(self.baseline, self.directory)
        self.after = self.directory / "leduc_cfr_0001.csv"

    def tearDown(self):
        self.storage.cleanup()

    def test_valid_replay_and_independent_metrics_pass(self):
        result = verifier.verify(self.directory, "cfr", 0)
        self.assertLess(result["max_absolute_difference"]["current"], 1e-12)
        verifier.evaluate_snapshot(self.after, "cfr")

    def test_changed_regrets_are_rejected(self):
        mutate_field(self.after, "regret", lambda x: str(float(x) + 0.1))
        with self.assertRaisesRegex(AssertionError, "regret"):
            verifier.verify(self.directory, "cfr", 0)

    def test_changed_averaging_is_rejected(self):
        mutate_field(self.after, "strategy_sum", lambda x: str(float(x) + 0.1))
        with self.assertRaisesRegex(AssertionError, "strategy_sum"):
            verifier.verify(self.directory, "cfr", 0)

    def test_each_changed_metric_is_rejected(self):
        metrics = self.after.with_suffix(".metrics.csv")
        original = metrics.read_bytes()
        for name in ["player_0_value", "br0", "br1", "nash_conv"]:
            with self.subTest(metric=name):
                metrics.write_bytes(original)
                mutate_field(metrics, name, lambda x: str(float(x) + 0.1))
                with self.assertRaisesRegex(AssertionError, name):
                    verifier.evaluate_snapshot(self.after, "cfr")

    def test_missing_next_snapshot_is_rejected(self):
        self.after.unlink()
        with self.assertRaises(FileNotFoundError):
            verifier.verify(self.directory, "cfr", 0)

    def test_incomplete_capture_cannot_report_success(self):
        with self.assertRaisesRegex(ValueError, "Snapshot set mismatch"):
            verifier.validate_directory(self.directory)
        self.assertEqual(len(verifier.REQUIRED_ITERATIONS), 18)
        self.assertIn(2000, verifier.REQUIRED_ITERATIONS)
        self.assertIn(10000, verifier.REQUIRED_ITERATIONS)

    def test_wrong_action_mapping_is_rejected(self):
        mutate_field(
            self.after.with_suffix(".metrics.csv"), "action_mapping", lambda _: "true"
        )
        with self.assertRaisesRegex(ValueError, "action mapping"):
            verifier.evaluate_snapshot(self.after, "cfr")

    def test_nonfinite_snapshot_is_rejected(self):
        mutate_field(self.after, "regret", lambda _: "nan")
        with self.assertRaisesRegex(ValueError, "Nonfinite"):
            verifier.verify(self.directory, "cfr", 0)

    def test_changed_reference_source_is_rejected(self):
        with (
            mock.patch.object(verifier, "EXPECTED_CFR_SHA256", "0" * 64),
            self.assertRaisesRegex(ValueError, "source hash"),
        ):
            verifier.checked_game()

    def test_changed_reference_version_is_rejected(self):
        with (
            mock.patch.object(
                verifier.importlib.metadata, "version", return_value="0.0.0"
            ),
            self.assertRaisesRegex(ValueError, "version"),
        ):
            verifier.checked_game()


if __name__ == "__main__":
    unittest.main()
