"""The rule that sorts turn policy differences, and the record it writes down."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import _fixture
from review_combos import review
from review_rule import (
    RuleError,
    action_gap,
    classify,
    default_rules_path,
    load_rules,
    same_rule,
    switch_loss,
)

SHIPPED = load_rules()
# The fixture's action EVs are made up rather than solved, so the oracle's honest
# recomputation of them disagrees by chips. The tests that exercise the record open the
# agreement tolerance; the one below proves the shipped tolerance refuses the fixture.
RULES = SHIPPED | {
    "real_gap_budget_pot_fraction": 0.05,
    # Above the 1.5 chips the fixture's fabricated EVs are out by. A rules file cannot
    # carry a tolerance this loose; the constructed dict here can.
    "oracle_agreement_chips": 2.0,
    "sha256": "test-rule",
}


def captures(root_check_frequency):
    reference = _fixture.reference_capture()
    return (
        _fixture.project_capture(reference, root_check_frequency=root_check_frequency),
        reference,
    )


class ShippedRuleTests(unittest.TestCase):
    def test_the_shipped_thresholds_are_the_decision_14_candidates(self):
        self.assertEqual(SHIPPED["indifference_pot_fraction"], 0.01)
        self.assertEqual(SHIPPED["reach_floor"], 1e-06)
        self.assertEqual(SHIPPED["real_gap_budget_pot_fraction"], 0.005)

    def test_the_file_says_the_numbers_are_not_confirmed(self):
        self.assertIn("not confirmed", SHIPPED["status"])
        self.assertIn("Decision 14 candidate", SHIPPED["status"])

    def test_the_hash_is_the_hash_of_the_file_it_read(self):
        import hashlib

        digest = hashlib.sha256(default_rules_path().read_bytes()).hexdigest()
        self.assertEqual(SHIPPED["sha256"], digest)

    def test_a_record_generated_under_another_threshold_is_not_the_same_rule(self):
        self.assertTrue(same_rule(SHIPPED, SHIPPED))
        self.assertFalse(same_rule(SHIPPED | {"reach_floor": 0.1}, SHIPPED))
        self.assertFalse(same_rule(SHIPPED | {"sha256": "0" * 64}, SHIPPED))
        self.assertFalse(same_rule(None, SHIPPED))

    def write(self, text):
        folder = tempfile.mkdtemp()
        path = Path(folder) / "rules.json"
        path.write_text(text, encoding="utf-8")
        return path

    def test_a_rules_file_missing_a_threshold_is_refused(self):
        path = self.write(json.dumps({"schema_version": 1, "reach_floor": 1e-6}))
        with self.assertRaises(RuleError):
            load_rules(path)

    def test_a_threshold_outside_zero_to_one_is_refused(self):
        body = {name: 0.01 for name in ("indifference_pot_fraction", "reach_floor")}
        body |= {"schema_version": 1, "real_gap_budget_pot_fraction": 2.0}
        with self.assertRaises(RuleError):
            load_rules(self.write(json.dumps(body)))

    def test_a_field_the_rule_does_not_read_is_refused(self):
        body = json.loads(default_rules_path().read_text(encoding="utf-8"))
        body["indifference_chips"] = 1.0
        with self.assertRaises(RuleError):
            load_rules(self.write(json.dumps(body)))

    def test_a_file_that_is_not_a_version_1_rule_is_refused(self):
        with self.assertRaises(RuleError):
            load_rules(self.write('{"schema_version": 9}'))
        with self.assertRaises(RuleError):
            load_rules(self.write("not json"))


class SwitchLossTests(unittest.TestCase):
    """What adopting the other mix costs, on the EVs of whoever would adopt it."""

    def test_a_switch_that_loses_and_one_that_gains_both_count(self):
        self.assertAlmostEqual(switch_loss([1, 0], [0, 1], [0.0, 1.0]), 1.0)
        self.assertAlmostEqual(switch_loss([0, 1], [1, 0], [0.0, 1.0]), 1.0)

    def test_two_mixes_over_equal_actions_cost_nothing_to_swap(self):
        self.assertEqual(switch_loss([0.9, 0.1], [0.1, 0.9], [2.0, 2.0]), 0.0)

    def test_a_side_with_no_ev_has_no_loss_to_report(self):
        self.assertIsNone(switch_loss([1, 0], [0, 1], None))

    def test_the_action_gap_is_the_widest_pair(self):
        self.assertEqual(action_gap([1.0, -2.0, 0.5]), 3.0)
        self.assertIsNone(action_gap(None))
        self.assertIsNone(action_gap([]))


class ClassificationTests(unittest.TestCase):
    """One row at a time, with the numbers chosen so the category is arithmetic."""

    def row(self, project, reference, **overrides):
        value = {
            "history": [],
            "cards": ["Ad", "Ah"],
            "project_strategy": project,
            "reference_strategy": reference,
            "project_action_ev": [0.0, 1.0],
            "reference_action_ev": [0.0, 1.0],
            "project_own_reach": 1.0,
            "project_opponent_mass": 12.0,
        }
        return value | overrides

    def classify(self, **kwargs):
        return classify(self.row(**kwargs), 10.0, 72.0, RULES)

    def test_a_cheap_switch_is_indifferent(self):
        verdict = self.classify(project=[0.70, 0.30], reference=[0.75, 0.25])
        self.assertEqual(verdict["category"], "indifferent")
        self.assertAlmostEqual(verdict["max_switch_loss_chips"], 0.05)
        self.assertIn("worth the same", verdict["review_reasoning"])
        self.assertIsNone(verdict["reach_weighted_loss_chips"])

    def test_an_expensive_switch_at_a_reached_history_is_a_real_gap(self):
        verdict = self.classify(project=[0.40, 0.60], reference=[0.75, 0.25])
        self.assertEqual(verdict["category"], "real_gap")
        self.assertAlmostEqual(verdict["max_switch_loss_chips"], 0.35)
        self.assertAlmostEqual(verdict["reach"], 1 / 6)
        self.assertAlmostEqual(verdict["reach_weighted_loss_chips"], 0.35 / 6)
        self.assertIn("counted against the gate's budget", verdict["review_reasoning"])

    def test_the_same_switch_below_the_reach_floor_is_bounded_instead(self):
        verdict = self.classify(
            project=[0.40, 0.60], reference=[0.75, 0.25], project_own_reach=1e-9
        )
        self.assertEqual(verdict["category"], "unreached")
        self.assertEqual(verdict["reason"], "below_reach_floor")
        self.assertAlmostEqual(verdict["bound_chips"], verdict["reach"] * 0.35)
        self.assertIn("below the floor", verdict["review_reasoning"])

    def test_a_missing_reference_ev_is_unreached_and_bounded_by_the_other_side(self):
        verdict = self.classify(
            project=[0.40, 0.60], reference=[0.75, 0.25], reference_action_ev=None
        )
        self.assertEqual(verdict["reason"], "ev_absent")
        self.assertIn("reference capture reports no action EV", verdict["review_reasoning"])
        self.assertAlmostEqual(verdict["bound_chips"], verdict["reach"] * 1.0)

    def test_a_side_whose_actions_are_all_equal_bounds_the_row_at_zero(self):
        # The gap is exactly 0.0, which is a bound, not a missing bound.
        verdict = self.classify(
            project=[0.40, 0.60],
            reference=[0.75, 0.25],
            project_action_ev=None,
            reference_action_ev=[3.0, 3.0],
        )
        self.assertEqual(verdict["category"], "unreached")
        self.assertEqual(verdict["max_action_gap_chips"], 0.0)
        self.assertEqual(verdict["bound_chips"], 0.0)
        self.assertNotIn("bounds nothing", verdict["review_reasoning"])

    def test_the_absent_side_is_named_and_both_are_named_when_both_are_absent(self):
        one = self.classify(
            project=[0.40, 0.60], reference=[0.75, 0.25], reference_action_ev=None
        )
        self.assertIn("The reference capture reports", one["review_reasoning"])
        both = self.classify(
            project=[0.40, 0.60],
            reference=[0.75, 0.25],
            project_action_ev=None,
            reference_action_ev=None,
        )
        self.assertIn("The project and the reference capture", both["review_reasoning"])

    def test_a_row_with_no_ev_on_either_side_bounds_nothing_and_says_so(self):
        verdict = self.classify(
            project=[0.40, 0.60],
            reference=[0.75, 0.25],
            project_action_ev=None,
            reference_action_ev=None,
        )
        self.assertEqual(verdict["category"], "unreached")
        self.assertIsNone(verdict["bound_chips"])
        self.assertIn("bounds nothing", verdict["review_reasoning"])

    def test_a_case_with_no_compatible_weight_is_refused(self):
        with self.assertRaises(RuleError):
            classify(self.row([0.4, 0.6], [0.75, 0.25]), 10.0, 0.0, RULES)

    def test_a_case_with_no_pot_is_refused(self):
        with self.assertRaises(RuleError):
            classify(self.row([0.4, 0.6], [0.75, 0.25]), 0.0, 72.0, RULES)


class RecordTests(unittest.TestCase):
    """What ends up in per-combo-review.json, and what deliberately does not."""

    def test_indifferent_rows_are_counted_and_not_stored(self):
        project, reference = captures(0.70)
        report = review(project, reference, RULES)
        case = report["cases"][0]
        self.assertEqual(report["schema_version"], 2)
        self.assertEqual(case["counts"]["indifferent"], len(_fixture.OOP_HANDS))
        self.assertEqual(case["counts"]["real_gap"], 0)
        self.assertEqual(case["rows"], [])
        self.assertEqual(case["real_gap_reach_weighted_loss_chips"], 0.0)
        self.assertTrue(case["within_budget"])

    def test_real_gap_rows_are_stored_with_the_values_that_date_them(self):
        project, reference = captures(0.40)
        case = review(project, reference, RULES)["cases"][0]
        self.assertEqual(case["counts"]["real_gap"], len(_fixture.OOP_HANDS))
        self.assertEqual(len(case["rows"]), len(_fixture.OOP_HANDS))
        row = case["rows"][0]
        for field in (
            "history",
            "cards",
            "actions",
            "frequency_differences",
            "project_strategy",
            "reference_strategy",
            "reach",
            "reach_weighted_loss_chips",
            "review_reasoning",
        ):
            self.assertIn(field, row)

    def test_a_row_whose_recomputation_disagrees_is_refused(self):
        # The fixture reports action EVs of 0 and 1 chip that no walk of its own tree
        # produces. That is the shape of the failure this evidence exists to catch: a
        # convention error on both sides would leave every row in A and the rule would
        # never notice.
        project, reference = captures(0.40)
        with self.assertRaises(ValueError) as refusal:
            review(project, reference, SHIPPED | {"real_gap_budget_pot_fraction": 0.05})
        self.assertIn("oracle and the capture disagree", str(refusal.exception))

    def test_a_real_gap_row_carries_its_independent_recomputation(self):
        project, reference = captures(0.40)
        row = review(project, reference, RULES)["cases"][0]["rows"][0]
        self.assertEqual(len(row["oracle_action_ev"]), len(row["actions"]))
        self.assertGreaterEqual(row["oracle_max_abs_difference_chips"], 0.0)
        self.assertIn("reported_chance_node_ev", row["oracle_continuation_sources"])

    def test_the_record_names_the_rule_it_was_generated_under(self):
        project, reference = captures(0.40)
        report = review(project, reference, RULES)
        self.assertEqual(report["rule"], RULES)
        self.assertEqual(report["street"], "turn")

    def test_the_threshold_free_totals_are_reported(self):
        project, reference = captures(0.40)
        case = review(project, reference, RULES)["cases"][0]
        # Six rows, each 0.35 chips at a reach of one sixth, all of them real gaps.
        self.assertAlmostEqual(case["real_gap_reach_weighted_loss_chips"], 0.35)
        self.assertEqual(case["indifferent_reach_weighted_loss_chips"], 0.0)
        self.assertAlmostEqual(case["all_rows_reach_weighted_loss_chips"], 0.35)
        self.assertAlmostEqual(case["all_rows_reach_weighted_loss_pot_fraction"], 0.035)

    def test_indifferent_rows_carry_their_own_loss_into_the_total(self):
        project, reference = captures(0.70)
        case = review(project, reference, RULES)["cases"][0]
        # Six rows at 0.05 chips and a reach of one sixth: 0.05 chips in total.
        self.assertAlmostEqual(case["indifferent_reach_weighted_loss_chips"], 0.05)
        self.assertEqual(case["real_gap_reach_weighted_loss_chips"], 0.0)
        self.assertAlmostEqual(case["all_rows_reach_weighted_loss_chips"], 0.05)

    def test_matching_captures_leave_nothing_to_record(self):
        reference = _fixture.reference_capture()
        report = review(_fixture.project_capture(reference), reference, RULES)
        case = report["cases"][0]
        self.assertEqual(case["counts"]["rows"], 0)
        self.assertEqual(case["rows"], [])


if __name__ == "__main__":
    unittest.main()
