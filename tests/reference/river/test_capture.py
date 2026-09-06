"""Protocol/input guard tests; no numerical reference output is manufactured."""

import copy
import io
import json
from itertools import combinations
import unittest
from unittest.mock import MagicMock

import capture


class CaptureGuards(unittest.TestCase):
    def setUp(self):
        self.payload = capture.read_json(
            capture.ROOT / "cases.json", capture.MAX_INPUT_BYTES
        )

    def test_authored_cases(self):
        capture.validate_inputs(self.payload)
        self.assertEqual(
            [c["effective_stack"] for c in self.payload["cases"]], [20, 100, 200]
        )
        capture.validate_inputs(
            capture.read_json(
                capture.ROOT / "diagnostics.json", capture.MAX_INPUT_BYTES
            )
        )

    def test_input_mutations(self):
        for field, bad in (
            ("board", ["Ac"] * 5),
            ("ranges", ["AA:0.5", "KK"]),
            ("max_iterations", 20001),
            ("max_iterations", True),
            ("check_every", 0),
            ("starting_pot", -1),
            ("target_pct_of_pot", float("nan")),
            ("bets", ["1%", "50%"]),
            ("raises", ["a", "100%"]),
            ("id", "../../outside"),
        ):
            with self.subTest(field=field, bad=bad):
                payload = copy.deepcopy(self.payload)
                payload["cases"][0][field] = bad
                with self.assertRaises(ValueError):
                    capture.validate_inputs(payload)

    def test_duplicates_and_unknown_fields(self):
        payload = copy.deepcopy(self.payload)
        payload["cases"].append(payload["cases"][0])
        with self.assertRaises(ValueError):
            capture.validate_inputs(payload)
        self.payload["cases"][0]["rake"] = 0
        with self.assertRaises(ValueError):
            capture.validate_inputs(self.payload)

    def test_json_bounds_and_nonfinite(self):
        path = MagicMock()
        path.is_file.return_value = True
        for invalid in ('{"x":NaN}', '{"x":Infinity}', '{"x":1e999}', '{"x":1,"x":2}'):
            path.stat.return_value.st_size = len(invalid)
            path.open.return_value = io.BytesIO(invalid.encode())
            with self.assertRaises(ValueError):
                capture.read_json(path, 1000)
        path.stat.return_value.st_size = len(json.dumps(self.payload))
        with self.assertRaises(ValueError):
            capture.read_json(path, 10)

    def test_manifest_replacement_requires_known_source(self):
        path = MagicMock()
        path.read_text.return_value = "same same"
        with self.assertRaises(ValueError):
            capture.replace_once(path, "same", "new")
        with self.assertRaises(ValueError):
            capture.replace_once(path, "missing", "new")
        path.write_text.assert_not_called()

    def test_output_guard_rejects_missing_branches_and_unavailable_zero_evs(self):
        # Synthetic schema data for testing the validator, never saved as poker evidence.
        case = capture.read_json(capture.ROOT / "diagnostics.json", 65536)["cases"][0]
        private = []
        for ids in ([49, 50, 51], [40, 41, 42, 43]):
            private.append(
                [
                    {
                        "ids": list(pair),
                        "cards": [
                            "23456789TJQKA"[i // 4] + "cdhs"[i % 4] for i in pair
                        ],
                    }
                    for pair in combinations(ids, 2)
                ]
            )
        counts = [len(entries) for entries in private]
        nodes = []
        for depth in range(3):
            terminal = depth == 2
            player = None if terminal else depth % 2
            actions = (
                []
                if terminal
                else [{"kind": "check", "label": "check", "amount": None}]
            )
            nodes.append(
                {
                    "history": [0] * depth,
                    "history_labels": ["check"] * depth,
                    "kind": "terminal" if terminal else "decision",
                    "player": player,
                    "actions": actions,
                    "strategy": [] if terminal else [1] * counts[player],
                    "action_expected_values": (
                        [] if terminal else [None] * counts[player]
                    ),
                    "reach_weights": [[0] * n for n in counts],
                    "equity": [[None] * n for n in counts],
                    "expected_values": [[None] * n for n in counts],
                    "ev_available": [[False] * n for n in counts],
                }
            )
        output = {
            "schema_version": 1,
            "capture_version": 1,
            "execution_stop_policy": "target_or_cap",
            "runtime": {"node": capture.NODE_VERSION},
            "cases": [
                {
                    "input": case,
                    "iterations": 0,
                    "exploitability_chips": 0,
                    "exploitability_pct_of_pot": 0,
                    "stop_reason": "target",
                    "execution_stop_policy": "target_or_cap",
                    "private_cards": private,
                    "nodes": nodes,
                }
            ],
        }
        payload = {"schema_version": 1, "cases": [case]}
        capture.validate_output(output, payload)
        legacy = copy.deepcopy(output)
        del legacy["execution_stop_policy"]
        del legacy["cases"][0]["execution_stop_policy"]
        capture.validate_output(legacy, payload)
        with self.assertRaisesRegex(ValueError, "Incorrect execution stop policy"):
            capture.validate_output(legacy, payload, finish_budget=True)
        fixed = copy.deepcopy(output)
        fixed["execution_stop_policy"] = "fixed_iteration_budget"
        fixed["cases"][0].update(
            {
                "execution_stop_policy": "fixed_iteration_budget",
                "stop_reason": "fixed_iteration_budget",
            }
        )
        with self.assertRaisesRegex(ValueError, "Incomplete iteration budget"):
            capture.validate_output(fixed, payload, finish_budget=True)
        fixed["cases"][0]["iterations"] = case["max_iterations"]
        capture.validate_output(fixed, payload, finish_budget=True)
        with self.assertRaisesRegex(ValueError, "Incorrect execution stop policy"):
            capture.validate_output(fixed, payload)
        missing = copy.deepcopy(output)
        missing["cases"][0]["nodes"].pop()
        with self.assertRaisesRegex(ValueError, "omitted a branch"):
            capture.validate_output(missing, payload)
        filled = copy.deepcopy(output)
        filled["cases"][0]["nodes"][0]["expected_values"][0][0] = 0
        with self.assertRaisesRegex(ValueError, "Unavailable EV was filled"):
            capture.validate_output(filled, payload)


if __name__ == "__main__":
    unittest.main()
