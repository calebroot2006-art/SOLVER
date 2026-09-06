"""Comparison guards against measured, provenance-bound first-capture fixtures."""

import copy
import json
from pathlib import Path
import tomllib
import unittest

from compare import compare


class ComparisonGuards(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).parent / "measured" / "82f8f4c"
        cls.project = tomllib.loads((root / "project-linux-cases.toml").read_text())
        cls.reference = json.loads((root / "reference-cases.json").read_text())

    def test_real_capture_matches_trees_but_requires_frequency_review(self):
        report = compare(self.project, self.reference)
        self.assertEqual(report["structural_and_convergence_checks"], "passed")
        self.assertEqual(report["frequency_review"], "required")
        self.assertGreater(
            sum(c["committed_fold_origin_cells"] for c in report["cases"]), 0
        )
        for case in report["cases"]:
            self.assertGreater(case["matched_policy_rows"], 0)
            for row in case["differences"]:
                self.assertEqual(row["review_status"], "requires_per_combo_review")

    def test_missing_and_repeated_history_are_not_intersection_matches(self):
        for repeat in (False, True):
            project = copy.deepcopy(self.project)
            nodes = project["cases"][0]["nodes"]
            if repeat:
                nodes.append(nodes[-1])
            else:
                nodes.pop()
            with self.assertRaises(ValueError):
                compare(project, self.reference)

    def test_missing_physical_combo_is_rejected(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["nodes"][0]["hands"].pop()
        with self.assertRaisesRegex(ValueError, "physical combo set"):
            compare(project, self.reference)

    def test_metric_units_mass_and_stopping_are_checked(self):
        for field, value, message in (
            ("best_response_values", [0, 0], "metric units"),
            ("compatible_weight", 1, "compatible mass"),
            ("stop_reason", "IterationCap", "target/cap"),
        ):
            project = copy.deepcopy(self.project)
            project["cases"][0][field] = value
            with self.assertRaisesRegex(ValueError, message):
                compare(project, self.reference)

    def test_fixed_budget_cannot_claim_an_early_capture(self):
        project = copy.deepcopy(self.project)
        project["execution_stop_policy"] = "fixed_iteration_budget"
        project["cases"][0]["stop_reason"] = "fixed_iteration_budget"
        with self.assertRaisesRegex(ValueError, "fixed budget is incomplete"):
            compare(project, self.reference)

    def test_nonfinite_output_and_wrong_reference_revision_are_rejected(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["root_centered_expected_values"][0] = float("nan")
        with self.assertRaisesRegex(ValueError, "Nonfinite"):
            compare(project, self.reference)
        reference = copy.deepcopy(self.reference)
        reference["provenance"]["engine_revision"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "Wrong reference engine"):
            compare(self.project, reference)


if __name__ == "__main__":
    unittest.main()
