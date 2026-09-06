"""The per-combo review runs end to end and labels where its numbers came from."""

from __future__ import annotations

import unittest

import _fixture
from review_combos import interpret, review

RIVER_HISTORY = ["check", "check", f"chance:{_fixture.RUNOUT}", "bet:3"]


def captures(refined_project_check, initial_project_check):
    reference = _fixture.reference_capture()
    initial_reference = _fixture.reference_capture()
    return {
        "project": _fixture.project_capture(
            reference, root_check_frequency=refined_project_check
        ),
        "reference": reference,
        "initial_project": _fixture.project_capture(
            initial_reference, root_check_frequency=initial_project_check
        ),
        "initial_reference": initial_reference,
    }


class ReviewTests(unittest.TestCase):
    def test_matching_captures_produce_no_rows(self):
        report = review(**captures(None, None))
        self.assertEqual(report["street"], "turn")
        self.assertEqual(report["cases"][0]["reviewed_union_rows"], 0)
        self.assertEqual(report["cases"][0]["remaining_frequency_rows"], 0)

    def test_a_turn_round_row_is_labelled_as_a_reported_continuation(self):
        report = review(**captures(0.40, 0.40))
        case = report["cases"][0]
        self.assertEqual(case["reviewed_union_rows"], len(_fixture.OOP_HANDS))
        self.assertEqual(case["remaining_frequency_rows"], len(_fixture.OOP_HANDS))
        row = case["rows"][0]
        self.assertEqual(row["street"], "turn")
        self.assertIsNone(row["runout"])
        self.assertIn(
            "reported_chance_node_ev", row["project_refined"]["continuation_sources"]
        )
        self.assertIn("turn-round row", row["interpretation"])
        self.assertIsNotNone(row["project_refined"]["counterfactual_action_ev"])
        self.assertIsNotNone(row["reference_refined"]["conditional_action_gaps"])

    def test_a_row_that_only_differed_initially_is_still_reviewed(self):
        report = review(**captures(None, 0.40))
        case = report["cases"][0]
        self.assertEqual(case["reviewed_union_rows"], len(_fixture.OOP_HANDS))
        self.assertEqual(case["remaining_frequency_rows"], 0)
        self.assertIn("within two percentage points", case["rows"][0]["interpretation"])

    def test_metrics_carry_the_oracle_scope_rather_than_an_exploitability(self):
        report = review(**captures(0.40, 0.40))
        for metrics in report["cases"][0]["refined_metrics"]:
            self.assertIn("scope", metrics)
            self.assertNotIn("best_response_values", metrics)

    def test_the_review_file_is_the_shape_compare_checks(self):
        report = review(**captures(0.40, 0.40))
        for row in report["cases"][0]["rows"]:
            self.assertIn("history", row)
            self.assertIn("cards", row)
            self.assertNotIn("review_reasoning", row)  # a reviewer adds it


class InterpretationTests(unittest.TestCase):
    def base(self, **overrides):
        value = {
            "own_history_reach": 1.0,
            "compatible_opponent_mass": 12.0,
            "continuation_sources": ["exact_fold", "independent_showdown"],
        }
        value.update(overrides)
        return value

    def test_small_differences_are_closed(self):
        text = interpret([0.01], self.base(), self.base())
        self.assertIn("within two percentage points", text)

    def test_zero_reference_reach_is_named(self):
        text = interpret([0.5], self.base(), self.base(own_history_reach=0.0))
        self.assertIn("own history reach is exactly zero", text)

    def test_zero_opposing_mass_is_named(self):
        text = interpret([0.5], self.base(compatible_opponent_mass=0.0), self.base())
        self.assertIn("Compatible opposing reach is zero", text)

    def test_an_exported_runout_row_says_the_values_are_independent(self):
        text = interpret([0.5], self.base(), self.base())
        self.assertIn("independently recomputed", text)


if __name__ == "__main__":
    unittest.main()
