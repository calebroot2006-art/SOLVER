"""Both comparison modes: reference-only validation, and project versus reference."""

from __future__ import annotations

import copy
import unittest

import _fixture
from compare import (
    compare,
    compatible_mass,
    isomorphic_runout_pairs,
    ranges_are_suit_symmetric,
    summarize_reference,
    swap_suits,
    uncovered_rows,
)


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


class IsomorphicRunoutTests(unittest.TestCase):
    def test_swapping_suits_only_touches_the_two_named_suits(self):
        self.assertEqual(swap_suits("Ac", "c", "d"), "Ad")
        self.assertEqual(swap_suits("Ad", "c", "d"), "Ac")
        self.assertEqual(swap_suits("Ah", "c", "d"), "Ah")

    def test_rank_class_ranges_are_symmetric_under_any_suit_swap(self):
        case = _fixture.case_input()
        for first, second in (("c", "d"), ("h", "s"), ("c", "s")):
            self.assertTrue(ranges_are_suit_symmetric(case, first, second))

    def test_a_range_naming_one_suit_breaks_the_symmetry(self):
        case = _fixture.case_input(ranges=["AhKd,AA", "QQ,JJ"])
        self.assertFalse(ranges_are_suit_symmetric(case, "c", "d"))
        self.assertFalse(ranges_are_suit_symmetric(case, "d", "h"))
        self.assertTrue(ranges_are_suit_symmetric(case, "c", "s"))

    def test_a_weight_that_differs_by_suit_breaks_the_symmetry(self):
        case = _fixture.case_input(ranges=["AhKd:0.5,AhKc,AA", "QQ,JJ"])
        self.assertFalse(ranges_are_suit_symmetric(case, "c", "d"))

    def test_a_board_with_no_suit_symmetry_reports_no_pairs(self):
        reference = _fixture.reference_capture()
        self.assertEqual(isomorphic_runout_pairs(reference["cases"][0]), [])

    def test_twin_runouts_must_agree_after_the_swap(self):
        # Ac Ad 7s 2h is fixed by swapping clubs and diamonds, so 4c and 4d must merge.
        paired = _paired_board_capture()
        report = isomorphic_runout_pairs(paired)
        self.assertEqual(len(report), 1)
        self.assertEqual(report[0]["runouts"], ["4c", "4d"])
        self.assertGreater(report[0]["compared_cells"], 0)
        self.assertEqual(report[0]["max_absolute_difference"], 0.0)

    def test_a_broken_merge_fails_loudly(self):
        paired = _paired_board_capture()
        for node in paired["nodes"]:
            if node["kind"] == "decision" and node["runout"] == "4d":
                node["strategy"] = [1.0 - value for value in node["strategy"]]
                break
        with self.assertRaises(ValueError):
            isomorphic_runout_pairs(paired)


def _paired_board_capture():
    """A minimal case whose board is fixed by swapping clubs and diamonds.

    Only the fields `isomorphic_runout_pairs` reads are built. Both runouts share one set of
    node bodies, with the diamond rows written as the club rows read through the swap, so a
    correct merge is the baseline that the next test breaks on purpose.
    """
    hands = [
        ["Kc", "Kd"],
        ["Kc", "Kh"],
        ["Kc", "Ks"],
        ["Kd", "Kh"],
        ["Kd", "Ks"],
        ["Kh", "Ks"],
    ]
    queens = [[card.replace("K", "Q") for card in hand] for hand in hands]
    private = [hands, queens]
    order = [
        hands.index(sorted(swap_suits(card, "c", "d") for card in hand))
        for hand in hands
    ]
    actions = [
        {"kind": "check", "label": "check", "amount": None},
        {"kind": "bet", "label": "bet:3", "amount": 3},
    ]
    club = [index / 10 for index in range(len(hands))]
    strategy = {
        "4c": club + [1 - value for value in club],
        "4d": [club[position] for position in order]
        + [1 - club[position] for position in order],
    }
    nodes = [
        {
            "history_labels": ["check", "check"],
            "kind": "chance",
            "street": "turn",
            "runout": None,
            "possible_cards": ["4c", "4d", "5c"],
            "representative_action_count": 2,
            "isomorphic_merged_cards": 1,
            "exported_runouts": ["4c", "4d"],
        }
    ]
    for runout in ("4c", "4d"):
        nodes.append(
            {
                "history_labels": ["check", "check", f"chance:{runout}"],
                "kind": "decision",
                "street": "river",
                "runout": runout,
                "player": 0,
                "actions": actions,
                "strategy": strategy[runout],
            }
        )
    return {
        "input": _fixture.case_input(
            board=["Ac", "Ad", "7s", "2h"],
            ranges=["KK", "QQ"],
            export_runouts=["4c", "4d"],
        ),
        "private_cards": [
            [{"cards": hand} for hand in private[player]] for player in (0, 1)
        ],
        "nodes": nodes,
    }


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
