"""Both comparison modes: reference-only validation, and project versus reference."""

from __future__ import annotations

import copy
import unittest

import _fixture
from compare import compare, compatible_mass, summarize_reference, uncovered_rows


class ReferenceOnlyTests(unittest.TestCase):
    def test_summary_reports_convergence_and_runout_bookkeeping(self):
        report = summarize_reference(_fixture.reference_capture())
        self.assertEqual(report["mode"], "reference_only")
        case = report["cases"][0]
        self.assertEqual(case["stop_reason"], "target")
        self.assertTrue(case["reached_target"])
        self.assertEqual(case["exploitability_pct_of_pot"], 0.1)
        self.assertEqual(case["dealable_runouts"], [3])
        self.assertEqual(case["isomorphic_merged_runouts"], [1])
        self.assertEqual(case["exported_runouts"], [_fixture.RUNOUT])
        self.assertEqual(case["private_hand_counts"], [6, 12])

    def test_iteration_cap_is_reported_not_hidden(self):
        report = summarize_reference(
            _fixture.reference_capture(stop_reason="iteration_cap")
        )
        self.assertEqual(report["cases"][0]["stop_reason"], "iteration_cap")
        self.assertFalse(report["cases"][0]["reached_target"])

    def test_a_wrong_engine_revision_is_refused(self):
        reference = _fixture.reference_capture()
        reference["provenance"]["engine_revision"] = "0" * 40
        with self.assertRaises(ValueError):
            summarize_reference(reference)

    def test_a_river_capture_is_refused(self):
        reference = _fixture.reference_capture()
        reference["street"] = "river"
        with self.assertRaises(ValueError):
            summarize_reference(reference)


class CompatibleMassTests(unittest.TestCase):
    def test_weighted_pairs_are_counted(self):
        self.assertEqual(compatible_mass(_fixture.case_input()), 72.0)
        weighted = _fixture.case_input(ranges=["AA:0.5,KK", "QQ,JJ"])
        self.assertEqual(compatible_mass(weighted), 54.0)


class FullComparisonTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(self.reference)

    def test_identical_policies_need_no_review(self):
        report = compare(self.project, self.reference)
        self.assertEqual(report["frequency_review"], "within_two_percentage_points")
        case = report["cases"][0]
        self.assertEqual(case["frequency_rows_requiring_review"], 0)
        self.assertEqual(case["max_frequency_difference"], 0.0)
        self.assertGreater(case["committed_fold_origin_cells"], 0)
        self.assertEqual(case["chance_nodes"][0]["reference_isomorphic_merged"], 1)
        self.assertEqual(case["chance_nodes"][0]["project_isomorphic_merged"], 0)

    def test_rows_over_two_points_are_listed(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.40)
        report = compare(project, self.reference)
        self.assertEqual(report["frequency_review"], "required")
        rows = report["cases"][0]["differences"]
        self.assertEqual(len(rows), len(_fixture.OOP_HANDS))
        self.assertEqual(rows[0]["street"], "turn")
        self.assertIsNone(rows[0]["runout"])
        self.assertAlmostEqual(
            max(rows[0]["absolute_frequency_difference"]), 0.35, places=9
        )

    def test_two_point_rule_ignores_smaller_differences(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.76)
        report = compare(project, self.reference)
        self.assertEqual(report["frequency_review"], "within_two_percentage_points")

    def test_a_different_dealable_runout_set_fails(self):
        reference = copy.deepcopy(self.reference)
        for node in reference["cases"][0]["nodes"]:
            if node["kind"] == "chance":
                node["possible_cards"] = [_fixture.RUNOUT, "5c", "7c"]
                node["representative_action_count"] = 2
                node["isomorphic_merged_cards"] = 1
        with self.assertRaises(ValueError):
            compare(self.project, reference)

    def test_a_missing_public_history_fails(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["nodes"].pop()
        with self.assertRaises(ValueError):
            compare(project, self.reference)


class ReviewGateTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(
            self.reference, root_check_frequency=0.40
        )
        self.report = compare(self.project, self.reference)

    def test_missing_reasoning_is_reported(self):
        missing = uncovered_rows(self.report, None)
        self.assertEqual(len(missing), len(_fixture.OOP_HANDS))

    def test_blank_reasoning_does_not_count(self):
        review = {
            "cases": [
                {
                    "id": "turn_fixture",
                    "rows": [
                        {
                            "history": row["history"],
                            "cards": row["cards"],
                            "review_reasoning": "  ",
                        }
                        for row in self.report["cases"][0]["differences"]
                    ],
                }
            ]
        }
        self.assertEqual(
            len(uncovered_rows(self.report, review)), len(_fixture.OOP_HANDS)
        )

    def test_committed_reasoning_clears_every_row(self):
        review = {
            "cases": [
                {
                    "id": "turn_fixture",
                    "rows": [
                        {
                            "history": row["history"],
                            "cards": row["cards"],
                            "review_reasoning": "Checked by hand against the action gaps.",
                        }
                        for row in self.report["cases"][0]["differences"]
                    ],
                }
            ]
        }
        self.assertEqual(uncovered_rows(self.report, review), [])
        self.assertEqual(
            self.report["cases"][0]["differences"][0]["review_status"], "reviewed"
        )


if __name__ == "__main__":
    unittest.main()
