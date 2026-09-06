"""Synthetic display-protocol guards; optional checks of separately captured data.

Set RIVER_PRESENTATION_PAIR_DIR to the directory containing refined-cases.json
and raw-refined-cases.json to also test the measured pair and its mutations.
"""

import copy
import math
import os
from pathlib import Path
import unittest

import capture
import verify_presentation as verify


def synthetic_nodes():
    # Independently authored protocol cells, never persisted as solver evidence.
    raw = {
        "history": [],
        "history_labels": [],
        "kind": "decision",
        "player": 0,
        "actions": [{"kind": "check", "label": "check", "amount": None}],
        "terminal": None,
        "fold_winner": None,
        "contributions": [0, 0],
        "reported_contributions": [0, 0],
        "wasm_empty_range_flag": 0,
        "reach_weights": [[1, 0.0000002], [1]],
        "normalized_weights": [[0.9, 0.0000003], [1]],
        "ev_available": [[True, True], [True]],
        "equity": [[0.25, 0.75], [0.5]],
        "expected_values": [[2.1256789, 7.25], [5]],
        "strategy": [1, 1],
        "action_expected_values": [2.1256789, 7.25],
    }
    default = copy.deepcopy(raw)
    default["reach_weights"][0][1] = 0
    default["normalized_weights"][0][1] = 0
    default["ev_available"][0][1] = False
    default["equity"][0][1] = None
    default["expected_values"][0] = [2.12568, None]
    default["action_expected_values"] = [2.12568, None]
    return default, raw


class DisplayGuards(unittest.TestCase):
    def test_rounding_bands_signed_values_and_half_ties(self):
        for value, expected in (
            (0.1234567, 0.123457),
            (1.234567, 1.23457),
            (12.34567, 12.3457),
            (123.4567, 123.457),
            (1234.567, 1234.57),
            (12345.67, 12345.7),
            (-12.3456789, -12.345679),
            (0.0000025, 0.000003),
            (-0.0000025, -0.000003),
            (math.nextafter(0.0000005, 0), 0),
        ):
            with self.subTest(value=value):
                self.assertEqual(verify.display_round(value), expected)
        for invalid in (None, True, float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                verify.display_round(invalid)
        self.assertGreater(verify.DISPLAY_CUTOFF, 0.0005)

    def test_tiny_mass_and_null_cells(self):
        default, raw = synthetic_nodes()
        matched = verify.compare_node(default, raw, [2, 1])
        self.assertEqual(matched["strategy_cells"], 2)
        self.assertEqual(matched["ev_cells"], 2)
        self.assertEqual(matched["additional_raw_ev_cells"], 1)
        self.assertEqual(matched["additional_raw_action_ev_cells"], 1)
        for data in (default, raw):
            data["normalized_weights"][0][1] = 0
            data["ev_available"][0][1] = False
            data["equity"][0][1] = None
            data["expected_values"][0][1] = None
            data["action_expected_values"][1] = None
        verify.compare_node(default, raw, [2, 1])
        raw["expected_values"][0][1] = 0
        with self.assertRaisesRegex(ValueError, "unavailable"):
            verify.compare_node(default, raw, [2, 1])

    def test_display_empty_range_hides_normalized_weights_and_all_evs(self):
        default, raw = synthetic_nodes()
        raw["reach_weights"][0] = [0.000001, 0.0000002]
        default["reach_weights"][0] = [0, 0]
        default["wasm_empty_range_flag"] = 1
        default["normalized_weights"] = [None, None]
        default["ev_available"] = [[False, False], [False]]
        default["equity"] = [[None, None], [None]]
        default["expected_values"] = [[None, None], [None]]
        default["action_expected_values"] = [None, None]
        matched = verify.compare_node(default, raw, [2, 1])
        self.assertEqual(matched["additional_raw_ev_cells"], 3)
        self.assertEqual(matched["normalized_cells"], 0)
        default["normalized_weights"] = [[0, 0], [0]]
        with self.assertRaisesRegex(ValueError, "empty normalized"):
            verify.compare_node(default, raw, [2, 1])

    def test_changed_shared_cells_flags_and_dimensions_fail(self):
        for field, update in (
            ("strategy", [0.99999, 1]),
            ("action_expected_values", [2.12569, None]),
            ("equity", [[0.250001, None], [0.5]]),
            ("expected_values", [[2.12569, None], [5]]),
            ("reach_weights", [[0.999999, 0], [1]]),
            ("normalized_weights", [[0.899999, 0], [1]]),
            ("wasm_empty_range_flag", 1),
            ("history_labels", ["bet:5"]),
            ("ev_available", [[True, True], [True]]),
            ("action_expected_values", []),
            ("normalized_weights", [[0.9], [1]]),
        ):
            default, raw = synthetic_nodes()
            default[field] = update
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify.compare_node(default, raw, [2, 1])

    @unittest.skipUnless(
        os.environ.get("RIVER_PRESENTATION_PAIR_DIR"),
        "Measured pair directory not provided",
    )
    def test_measured_pair_and_malformed_regressions(self):
        directory = Path(os.environ["RIVER_PRESENTATION_PAIR_DIR"])
        default = capture.read_json(
            directory / "refined-cases.json", capture.MAX_OUTPUT_BYTES
        )
        raw = capture.read_json(
            directory / "raw-refined-cases.json", capture.MAX_OUTPUT_BYTES
        )
        result = verify.verify(default, raw)
        self.assertEqual(result["totals"]["nodes"], 69)
        self.assertEqual(result["totals"]["strategy_cells"], 4208)
        for path, value in (
            (("presentation_mode",), "upstream_display"),
            (("provenance", "original_wrapper_sha256"), "0" * 64),
            (("provenance", "instrumented_wrapper_sha256"), "0" * 64),
            (("provenance", "instrumentation_id"), "unexpected_v2"),
            (("provenance", "engine_revision"), "0" * 40),
            (("provenance", "capture_python_sha256"), "0" * 64),
            (("provenance", "wasm_sha256"), "missing"),
            (("provenance", "build_adjustments"), []),
            (("cases", 0, "iterations"), 19999),
            (("cases", 0, "input", "effective_stack"), 21),
            (("cases", 0, "checkpoints", 0, "exploitability_chips"), -1),
            (("cases", 0, "root_expected_values"), [0, 0]),
            (("cases", 0, "nodes", 0, "strategy", 0), 0.123),
        ):
            changed = copy.deepcopy(raw)
            target = changed
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = value
            with self.subTest(path=path), self.assertRaises(ValueError):
                verify.verify(default, changed)
        missing = copy.deepcopy(raw)
        del missing["provenance"]["resolved_dependencies"]
        with self.assertRaisesRegex(ValueError, "Missing provenance"):
            verify.verify(default, missing)
        no_checks_default = copy.deepcopy(default)
        no_checks_raw = copy.deepcopy(raw)
        for data in (no_checks_default, no_checks_raw):
            data["cases"][0]["checkpoints"] = []
        with self.assertRaisesRegex(ValueError, "checkpoint schedule"):
            verify.verify(no_checks_default, no_checks_raw)


if __name__ == "__main__":
    unittest.main()
