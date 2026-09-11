"""Acceptance regressions for root values, current oracle inputs and reach evidence."""

import copy
import unittest

import _fixture
from compare import (
    capture_evidence_failures,
    classify_rows,
    compare,
    joint_report,
    same_weight,
)
from oracle import Oracle, expand
from review_combos import review
from review_rule import classify, load_rules

RULES = load_rules()
REVISION = "0" * 40


def refresh_root(case):
    root = case["nodes"][0]
    value = sum(
        sum(p * v for p, v in zip(h["strategy"], h["action_expected_values"]))
        for h in root["hands"]
    ) / len(root["hands"])
    case["root_centered_expected_values"] = [value, -value]
    residual = case["input"]["starting_pot"] * case["exploitability_pct_of_pot"] / 100
    case["best_response_values"] = [value + residual, -value + residual]


class GateEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(self.reference)
        self.empty = {"schema_version": 2, "street": "turn", "rule": RULES, "cases": []}

    def gate(self, project=None, record=None):
        return joint_report(
            project or self.project,
            self.reference,
            record or self.empty,
            REVISION,
            RULES,
        )

    def test_unmodified_control_passes_shipped_rules(self):
        self.assertTrue(self.gate()["accepted"])

    def test_false_root_values_are_rejected_without_frequency_differences(self):
        self.project["cases"][0]["root_centered_expected_values"] = [1000.0, -1000.0]
        result = self.gate()
        self.assertFalse(result["accepted"])
        self.assertEqual(result["cases"][0]["frequency_rows_requiring_review"], 0)
        self.assertIn(
            "root_value_consistency", {f["check"] for f in result["gate_failures"]}
        )

    def test_root_vector_dimension_and_finiteness_are_required(self):
        for values in ([1.0], [1.0, -1.0, 0.0], [float("nan"), 0.0]):
            with self.subTest(values=values):
                project = copy.deepcopy(self.project)
                project["cases"][0]["root_centered_expected_values"] = values
                with self.assertRaises(ValueError):
                    self.gate(project)

    def test_non_zero_sum_root_values_fail(self):
        self.project["cases"][0]["root_centered_expected_values"][1] += 0.01
        self.assertFalse(self.gate()["accepted"])

    def test_root_policy_value_must_fit_its_own_br_interval(self):
        case = self.project["cases"][0]
        case["best_response_values"] = [0.01, 0.01]
        result = self.gate()
        self.assertTrue(
            any(
                "best-response interval" in f["reason"] for f in result["gate_failures"]
            )
        )

    def test_consistent_root_shift_outside_measured_residual_bound_fails(self):
        case = self.project["cases"][0]
        for row in case["nodes"][0]["hands"]:
            row["action_expected_values"] = [
                v + 0.1 for v in row["action_expected_values"]
            ]
        refresh_root(case)
        result = self.gate()
        self.assertEqual(
            {f["check"] for f in result["gate_failures"]}, {"root_value_agreement"}
        )

    def test_small_consistent_root_shift_within_residual_bound_passes(self):
        case = self.project["cases"][0]
        for row in case["nodes"][0]["hands"]:
            row["action_expected_values"] = [
                v + 0.001 for v in row["action_expected_values"]
            ]
        refresh_root(case)
        self.assertTrue(self.gate()["accepted"])

    def test_reached_root_missing_evs_cannot_be_excused(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.4)
        for row in project["cases"][0]["nodes"][0]["hands"]:
            row.update(ev_available=False, action_expected_values=[])
        result = self.gate(project)
        self.assertFalse(result["accepted"])
        self.assertIn("EV availability", result["gate_failures"][0]["reason"])
        self.assertNotIn("review", result)  # Refusal precedes classification.

    def test_false_zero_root_reach_cannot_be_excused(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.4)
        for row in project["cases"][0]["nodes"][0]["hands"]:
            row["own_reach"] = 0.0
        result = self.gate(project)
        self.assertFalse(result["accepted"])
        self.assertIn("own_reach contradicts", result["gate_failures"][0]["reason"])

    def test_false_compatible_mass_cannot_be_excused(self):
        self.project["cases"][0]["nodes"][0]["hands"][0]["opponent_mass"] = 0.0
        result = self.gate()
        self.assertFalse(result["accepted"])
        self.assertIn("opponent_mass contradicts", result["gate_failures"][0]["reason"])

    def test_reached_reference_root_missing_evs_cannot_be_excused(self):
        root = self.reference["cases"][0]["nodes"][0]
        root["ev_available"] = [False] * len(root["ev_available"])
        root["action_expected_values"] = [None] * len(root["action_expected_values"])
        self.assertFalse(self.gate()["accepted"])

    def oracle_control(self):
        case = self.project["cases"][0]
        hand = case["nodes"][0]["hands"][0]
        hand["strategy"] = [0.68, 0.32]
        hand["action_expected_values"] = Oracle(case).action_values((), hand["cards"])[
            "counterfactual_action_ev"
        ]
        self.assertEqual(hand["action_expected_values"], [-1.0, 2.5])
        _fixture.refresh_project_reach(case)
        refresh_root(case)
        record = review(self.project, self.reference, RULES)
        self.assertTrue(self.gate(record=record)["accepted"])
        return record

    def test_sub_two_point_downstream_policy_change_invalidates_oracle(self):
        record = self.oracle_control()
        case = self.project["cases"][0]
        node = next(n for n in case["nodes"] if n["history_labels"] == ["check"])
        for hand in node["hands"]:
            hand["strategy"] = [0.51, 0.49]
        _fixture.refresh_project_reach(
            case
        )  # Reach is current; only the parent EV is stale.
        result = self.gate(record=record)
        self.assertFalse(result["accepted"])
        self.assertEqual(result["stale_review_rows"], [])
        failures = [
            f for f in result["gate_failures"] if f["check"] == "oracle_agreement"
        ]
        self.assertEqual(len(failures), 1)
        self.assertIn("current walk", failures[0]["reason"])
        row = result["cases"][0]["differences"][0]
        self.assertAlmostEqual(row["current_oracle_action_ev"][0], -0.98, places=12)

    def test_changed_chance_continuation_invalidates_oracle(self):
        record = self.oracle_control()
        case = self.project["cases"][0]
        node = next(
            n for n in case["nodes"] if n["history_labels"] == ["check", "check"]
        )
        node["hands"][0]["expected_value"] += 100.0
        result = self.gate(record=record)
        self.assertFalse(result["accepted"])
        self.assertEqual(result["stale_review_rows"], [])
        row = result["cases"][0]["differences"][0]
        self.assertEqual(row["current_oracle_action_ev"], [49.0, 2.5])

    def test_real_zero_own_action_reach_is_consistent(self):
        case = self.project["cases"][0]
        node = next(n for n in case["nodes"] if n["history_labels"] == ["check"])
        for hand in node["hands"]:
            hand["strategy"] = [0.0, 1.0]
        _fixture.refresh_project_reach(case)
        self.assertEqual(capture_evidence_failures(self.project, self.reference), [])
        river = next(
            n
            for n in case["nodes"]
            if n["history_labels"] == ["check", "check", "chance:4c"]
        )
        self.assertTrue(
            all(
                not h["ev_available"] and h["opponent_mass"] == 0
                for h in river["hands"]
            )
        )
        ip = next(
            n
            for n in case["nodes"]
            if n["history_labels"] == ["check", "check", "chance:4c", "check"]
        )
        self.assertTrue(
            all(not h["ev_available"] and h["own_reach"] == 0 for h in ip["hands"])
        )

    def test_runout_blockers_remove_dead_hands_without_calling_them_zero(self):
        reference = _fixture.reference_capture(runout="Ad")
        project = _fixture.project_capture(reference)
        self.assertEqual(capture_evidence_failures(project, reference), [])
        node = next(
            n
            for n in project["cases"][0]["nodes"]
            if n["kind"] == "decision" and n["runout"]
        )
        self.assertTrue(all("Ad" not in h["cards"] for h in node["hands"]))

    def tiny_mass_capture(self):
        """Minimal evidence-unit capture: dominant AhAs blocks itself on both sides.

        Only the 1e-15 KK weights make compatible deals. A dominant root hand
        therefore has mass 3e-15 but normalized information-set reach one half.
        Raw f32 reference presentation can retain these tiny positive weights.
        """
        board = ["Ac", "Ad", "Kh", "Qh"]
        ranges = ["AA,KK:0.000000000000001"] * 2
        hands = [list(expand(text, board)) for text in ranges]
        inputs = {
            "id": "tiny_mass",
            "board": board,
            "ranges": ranges,
            "starting_pot": 10,
            "effective_stack": 40,
        }
        rows = [
            {
                "cards": list(h),
                "strategy": [0.4, 0.6],
                "action_expected_values": [-0.6, 0.4],
                "ev_available": True,
                "own_reach": 1.0 if h == ("Ah", "As") else 1e-15,
                "opponent_mass": 3e-15 if h == ("Ah", "As") else 1.0,
            }
            for h in hands[0]
        ]
        node = {
            "history_labels": [],
            "kind": "decision",
            "street": "turn",
            "runout": "",
            "player": 0,
            "actions": ["check", "allin:40"],
            "contributions": [0, 0],
            "hands": rows,
        }
        case = {
            "input": inputs,
            "nodes": [node],
            "best_response_values": [0.01, 0.01],
            "root_centered_expected_values": [0.0, 0.0],
            "exploitability_pct_of_pot": 0.1,
            "compatible_weight": 6e-15,
        }
        reference = copy.deepcopy(case)
        refnode = reference["nodes"][0]
        refnode.pop("hands")
        count = len(hands[0])
        refnode.update(
            actions=[{"label": a} for a in node["actions"]],
            strategy=[0.75] * count + [0.25] * count,
            action_expected_values=[4.75] * count + [5.75] * count,
            ev_available=[True] * count,
            wasm_empty_range_flag=0,
        )
        reference["private_cards"] = [
            [{"cards": list(h)} for h in side] for side in hands
        ]
        reference["root_expected_values"] = [5.0, 5.0]
        return {"cases": [case]}, {"cases": [reference], "presentation_mode": "raw_f32"}

    def test_tiny_positive_mass_cannot_be_forged_into_an_unreached_row(self):
        project, reference = self.tiny_mass_capture()
        self.assertEqual(capture_evidence_failures(project, reference), [])
        hand = next(
            h
            for h in project["cases"][0]["nodes"][0]["hands"]
            if h["cards"] == ["Ah", "As"]
        )
        row = {
            "history": [],
            "cards": hand["cards"],
            "project_strategy": [0.4, 0.6],
            "reference_strategy": [0.75, 0.25],
            "project_action_ev": [-0.6, 0.4],
            "reference_action_ev": [-0.25, 0.75],
            "project_own_reach": 1.0,
            "project_opponent_mass": 3e-15,
        }
        true = classify(row, 10, 6e-15, RULES)
        self.assertEqual(true["reach"], 0.5)
        self.assertEqual(true["category"], "real_gap")
        for forged in (0.0, 1e-100, 1e-22):
            with self.subTest(forged=forged):
                hand["opponent_mass"] = forged
                failures = capture_evidence_failures(project, reference)
                self.assertTrue(
                    any("opponent_mass contradicts" in f["reason"] for f in failures)
                )
                # The old absolute floor admitted this exact contradiction and
                # classification then called the material discrepancy unreached.
                false = classify(
                    row | {"project_opponent_mass": forged}, 10, 6e-15, RULES
                )
                self.assertEqual(false["category"], "unreached")

    def test_root_mass_agreement_has_no_absolute_floor(self):
        self.assertLess(abs(1e-8 - 6e-15), 1e-6)  # The previous check accepted it.
        self.assertFalse(same_weight(1e-8, 6e-15))
        self.assertFalse(same_weight(0.0, 6e-15))
        self.assertFalse(same_weight(1e-100, 6e-15))
        self.assertFalse(same_weight(5e-324, 1e-323))
        self.assertTrue(same_weight(6e-15 * (1 + 1e-12), 6e-15))

    def test_classification_normalizes_by_reconstructed_root_mass(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.4)
        report = compare(project, self.reference)
        project["cases"][0]["compatible_weight"] = 1e100
        classify_rows(report, project, RULES)
        self.assertAlmostEqual(report["cases"][0]["differences"][0]["reach"], 1 / 6)
        self.assertEqual(report["cases"][0]["differences"][0]["category"], "real_gap")

    def test_private_card_blockers_remove_only_incompatible_opponents(self):
        case = copy.deepcopy(self.project["cases"][0])
        case["input"]["ranges"] = ["AA,KK", "AA,KK"]
        case["nodes"] = [case["nodes"][0]]
        oracle = Oracle(case)
        self.assertEqual(oracle.mass, 18.0)
        self.assertEqual(oracle.reach_evidence((), 0)[("Ad", "Ah")], (1.0, 3.0))

    def test_range_scaling_matches_project_inclusion_weights(self):
        from compare import compatible_mass

        case = copy.deepcopy(self.project["cases"][0])
        case["input"]["ranges"] = ["AA:0.25,KK:0.5", "QQ:0.5,JJ:0.5"]
        oracle = Oracle(case)
        evidence = oracle.reach_evidence((), 0)
        self.assertEqual(evidence[("Ad", "Ah")], (0.5, 12.0))
        self.assertEqual(evidence[("Kc", "Kh")], (1.0, 12.0))
        self.assertEqual(compatible_mass(case["input"]), 54.0)

    def test_positive_project_path_underflow_is_a_refusal(self):
        case = self.project["cases"][0]
        case["nodes"][0]["hands"][0]["strategy"] = [1e-200, 1.0]
        river = next(
            n
            for n in case["nodes"]
            if n["history_labels"] == ["check", "check", "chance:4c"]
        )
        river["hands"][0]["strategy"] = [1e-200, 1.0]
        with self.assertRaisesRegex(ValueError, "Positive project reach underflow"):
            Oracle(case).path_weights(("check", "check", "chance:4c", "check"))

    def test_f32_underflow_and_rounded_zero_are_distinct(self):
        case = self.reference["cases"][0]
        root = case["nodes"][0]
        count = len(case["private_cards"][0])
        root["strategy"][0], root["strategy"][count] = 1e-30, 1.0
        river = next(
            n
            for n in case["nodes"]
            if n["history_labels"] == ["check", "check", "chance:4c"]
        )
        river["strategy"][0], river["strategy"][count] = 1e-30, 1.0
        oracle = Oracle(case, True)
        history = ("check", "check", "chance:4c", "check")
        self.assertEqual(oracle.path_weights(history)[0][0][("Ad", "Ah")], 0.0)
        root["strategy"][0] = 0.0
        rounded = Oracle(case, True).reference_reach_bounds(("check",), 0.5e-6)[0][
            ("Ad", "Ah")
        ]
        self.assertEqual(rounded[0], 0.0)
        self.assertGreater(rounded[1], 0.0)


if __name__ == "__main__":
    unittest.main()
