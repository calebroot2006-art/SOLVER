"""Guards on the turn capture's input and output contracts. No WASM build is needed."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

import _fixture
import capture

ROOT = Path(__file__).resolve().parent


class RangeExpansionTests(unittest.TestCase):
    def test_committed_cases_parse_and_stay_in_range(self):
        payload = json.loads((ROOT / "cases.json").read_text(encoding="utf-8"))
        capture.validate_inputs(payload)
        for case in payload["cases"]:
            for text in case["ranges"]:
                self.assertGreaterEqual(
                    len(capture.physical_hands(text, case["board"])), 400
                )

    def test_interval_forms_match_the_project_parser_semantics(self):
        weights = capture.expand_range("22-44")
        self.assertEqual(len(weights), 18)
        self.assertEqual(len(capture.expand_range("A2s-A5s")), 16)
        self.assertEqual(len(capture.expand_range("98s-65s")), 16)
        self.assertEqual(len(capture.expand_range("22+")), 78)
        self.assertEqual(len(capture.expand_range("ATo+")), 48)

    def test_weights_are_kept_and_conflicts_are_rejected(self):
        weights = capture.expand_range("JJ:0.5,TT")
        self.assertEqual(sorted(set(weights.values())), [0.5, 1.0])
        with self.assertRaises(ValueError):
            capture.expand_range("AA:0.5,AA:0.25")

    def test_zero_weight_combos_are_dropped(self):
        self.assertEqual(capture.expand_range("AA:0"), {})


class InputContractTests(unittest.TestCase):
    def payload(self, **overrides):
        return {
            "schema_version": 1,
            "street": "turn",
            "ranges_provenance": "x" * 60,
            "cases": [_fixture.case_input(**overrides)],
        }

    def test_a_fixture_style_case_needs_a_real_range(self):
        # The tiny fixture ranges are deliberately below the input floor.
        with self.assertRaises(ValueError):
            capture.validate_inputs(self.payload())

    def test_five_card_board_is_rejected(self):
        payload = self.payload(board=["Ac", "Kd", "7s", "2h", "9d"])
        with self.assertRaises(ValueError):
            capture.validate_inputs(payload)

    def test_export_runout_on_the_board_is_rejected(self):
        payload = self.payload(export_runouts=["Ac"])
        with self.assertRaises(ValueError):
            capture.validate_inputs(payload)

    def test_nonempty_flop_menu_is_rejected(self):
        case = _fixture.case_input()
        case["menus"]["flop"]["oop_bet"] = "50%"
        with self.assertRaises(ValueError):
            capture.validate_inputs(
                {
                    "schema_version": 1,
                    "street": "turn",
                    "ranges_provenance": "x" * 60,
                    "cases": [case],
                }
            )

    def test_donk_size_is_rejected_while_unsupported(self):
        case = _fixture.case_input()
        case["menus"]["river"]["oop_donk"] = "50%"
        with self.assertRaises(ValueError):
            capture.validate_inputs(
                {
                    "schema_version": 1,
                    "street": "turn",
                    "ranges_provenance": "x" * 60,
                    "cases": [case],
                }
            )


class OutputContractTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.payload = {"cases": [case["input"] for case in self.reference["cases"]]}

    def validate(self, reference=None):
        capture.validate_output(reference or self.reference, self.payload)

    def test_the_fixture_capture_passes(self):
        self.validate()

    def test_missing_runout_branch_is_rejected(self):
        reference = copy.deepcopy(self.reference)
        nodes = reference["cases"][0]["nodes"]
        reference["cases"][0]["nodes"] = [
            node
            for node in nodes
            if node["history_labels"][:3] != ["check", "check", "chance:4c"]
        ]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_chance_child_must_be_an_exported_runout(self):
        reference = copy.deepcopy(self.reference)
        for node in reference["cases"][0]["nodes"]:
            if node["kind"] == "chance":
                node["exported_runouts"] = ["5c"]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_merged_count_must_agree_with_the_representative_count(self):
        reference = copy.deepcopy(self.reference)
        for node in reference["cases"][0]["nodes"]:
            if node["kind"] == "chance":
                node["isomorphic_merged_cards"] = 0
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_ip_may_not_act_first_after_a_chance_node(self):
        reference = copy.deepcopy(self.reference)
        for node in reference["cases"][0]["nodes"]:
            if node["history_labels"] == ["check", "check", "chance:4c"]:
                node["player"] = 1
                node["reach_weights"] = [1.0] * len(_fixture.IP_HANDS)
                node["ev_available"] = [True] * len(_fixture.IP_HANDS)
                node["strategy"] = [1.0] * (2 * len(_fixture.IP_HANDS))
                node["action_expected_values"] = [1.0] * (2 * len(_fixture.IP_HANDS))
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_strategy_rows_must_sum_to_one(self):
        reference = copy.deepcopy(self.reference)
        reference["cases"][0]["nodes"][0]["strategy"][0] = 0.1
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_private_cards_must_match_the_input_ranges(self):
        reference = copy.deepcopy(self.reference)
        reference["cases"][0]["private_cards"][0].pop()
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_stop_reason_must_follow_the_measured_residual(self):
        reference = copy.deepcopy(self.reference)
        reference["cases"][0]["exploitability_chips"] = 1.0
        reference["cases"][0]["exploitability_pct_of_pot"] = 10.0
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_iteration_cap_must_use_the_whole_budget(self):
        reference = _fixture.reference_capture(stop_reason="iteration_cap")
        payload = {"cases": [case["input"] for case in reference["cases"]]}
        capture.validate_output(reference, payload)
        reference["cases"][0]["iterations"] = 3
        with self.assertRaises(ValueError):
            capture.validate_output(reference, payload)

    def test_river_terminals_carry_no_reported_values(self):
        reference = copy.deepcopy(self.reference)
        for node in reference["cases"][0]["nodes"]:
            if node["kind"] == "terminal" and node["street"] == "river":
                node["expected_values"] = [[0.0] * 6, [0.0] * 12]
                break
        with self.assertRaises(ValueError):
            self.validate(reference)


if __name__ == "__main__":
    unittest.main()
