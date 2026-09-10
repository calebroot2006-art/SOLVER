"""Both comparison modes: reference-only validation, and the joint project-versus-reference gate."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import _fixture
from compare import (
    called_all_in_runouts,
    compare,
    compatible_mass,
    convergence_failures,
    isomorphic_runout_pairs,
    joint_report,
    ranges_are_suit_symmetric,
    restate_project_labels,
    revision_failures,
    stale_reviews,
    street_relative_labels,
    summarize_reference,
    swap_suits,
    uncovered_rows,
)

REVISION = "0" * 40


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
        # The fixture board has no suit symmetry, so there is nothing to pair up here.
        self.assertEqual(case["isomorphic_runout_pairs"], [])

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


class LabelConventionTests(unittest.TestCase):
    """Our wagers name a total commitment; the reference's name this street's chips."""

    def test_a_wager_is_restated_by_the_actors_standing_contribution(self):
        node = {
            "kind": "decision",
            "player": 0,
            "contributions": [4, 4],
            "actions": ["check", "bet:10", "raise:41"],
        }
        self.assertEqual(street_relative_labels(node), ["check", "bet:6", "raise:37"])

    def test_a_root_node_is_left_alone(self):
        node = {
            "kind": "decision",
            "player": 0,
            "contributions": [0, 0],
            "actions": ["check", "bet:4", "allin:195"],
        }
        self.assertEqual(street_relative_labels(node), ["check", "bet:4", "allin:195"])

    def test_a_wager_below_the_standing_contribution_is_refused(self):
        node = {
            "kind": "decision",
            "player": 1,
            "contributions": [0, 9],
            "actions": ["fold", "call", "bet:9"],
        }
        with self.assertRaises(ValueError):
            street_relative_labels(node)

    def test_restating_rewrites_histories_and_is_done_once(self):
        project = {
            "cases": [
                {
                    "nodes": [
                        {
                            "history_labels": [],
                            "kind": "decision",
                            "street": "turn",
                            "player": 0,
                            "contributions": [0, 0],
                            "actions": ["check", "bet:4"],
                        },
                        {
                            "history_labels": ["bet:4"],
                            "kind": "decision",
                            "street": "turn",
                            "player": 1,
                            "contributions": [4, 0],
                            "actions": ["fold", "call"],
                        },
                        {
                            "history_labels": ["bet:4", "call"],
                            "kind": "decision",
                            "street": "river",
                            "player": 0,
                            "contributions": [4, 4],
                            "actions": ["check", "bet:10"],
                        },
                    ]
                }
            ]
        }
        self.assertEqual(restate_project_labels(project), 1)
        nodes = project["cases"][0]["nodes"]
        self.assertEqual(nodes[2]["history_labels"], ["bet:4", "call"])
        self.assertEqual(nodes[2]["actions"], ["check", "bet:6"])
        # A second pass changes nothing, so a repeated comparison is not a second subtraction.
        self.assertEqual(restate_project_labels(project), 0)
        self.assertEqual(nodes[2]["actions"], ["check", "bet:6"])


class CalledAllInTests(unittest.TestCase):
    """A called all-in runs the board out on our side and ends the hand on theirs."""

    @staticmethod
    def _case(child_kind="terminal", terminal="showdown"):
        return {
            "nodes": [
                {
                    "history_labels": ["allin:40", "call"],
                    "kind": "chance",
                    "street": "turn",
                    "contributions": [40, 40],
                    "possible_cards": ["4c", "5c", "6c"],
                    "actions": [],
                },
                {
                    "history_labels": ["allin:40", "call", "chance:4c"],
                    "kind": child_kind,
                    "street": "river",
                    "contributions": [40, 40],
                    "terminal": terminal,
                    "actions": [],
                },
            ]
        }

    def test_a_run_out_with_no_decision_is_recognised(self):
        found = called_all_in_runouts(self._case())
        self.assertEqual(list(found), [("allin:40", "call")])
        self.assertEqual(
            found[("allin:40", "call")], [("allin:40", "call", "chance:4c")]
        )

    def test_an_ordinary_deal_that_opens_betting_is_not_one(self):
        self.assertEqual(called_all_in_runouts(self._case(child_kind="decision")), {})

    def test_a_fold_terminal_under_a_deal_is_not_one(self):
        self.assertEqual(called_all_in_runouts(self._case(terminal="fold")), {})


class JointGateTests(unittest.TestCase):
    """Every rule the joint mode refuses on, and the one shape it accepts."""

    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(self.reference)

    def gate(self, project=None, reference=None, review=None, revision=REVISION):
        return joint_report(
            project if project is not None else self.project,
            reference if reference is not None else self.reference,
            review,
            revision,
        )

    def test_a_matching_converged_pair_is_accepted(self):
        report = self.gate()
        self.assertTrue(report["accepted"])
        self.assertEqual(report["gate_failures"], [])
        self.assertEqual(report["rows_missing_review_reasoning"], [])
        self.assertTrue(report["gate"]["revision_checked_against_workflow"])

    def test_an_above_target_project_solve_is_refused(self):
        project = copy.deepcopy(self.project)
        case = project["cases"][0]
        case["exploitability_pct_of_pot"] = 0.4
        case["stop_reason"] = "IterationCap"
        case["iterations"] = case["input"]["max_iterations"]
        case["best_response_values"] = [0.04, 0.04]
        report = self.gate(project=project)
        self.assertFalse(report["accepted"])
        checks = {f["check"] for f in report["gate_failures"]}
        self.assertIn("reached_target", checks)
        self.assertEqual(report["gate_failures"][0]["side"], "project")

    def test_an_above_target_reference_solve_is_refused(self):
        reference = _fixture.reference_capture(stop_reason="iteration_cap")
        project = _fixture.project_capture(reference)
        report = self.gate(project=project, reference=reference)
        self.assertFalse(report["accepted"])
        failure = next(f for f in report["gate_failures"] if f["side"] == "reference")
        self.assertEqual(failure["check"], "reached_target")
        self.assertEqual(failure["stop_reason"], "iteration_cap")

    def test_a_converged_solve_that_claims_the_wrong_stop_reason_is_refused(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["stop_reason"] = "Cancelled"
        with self.assertRaises(ValueError):
            self.gate(project=project)

    def test_a_capture_that_lies_about_reaching_the_target_is_refused(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["reached_target"] = False
        report = self.gate(project=project)
        self.assertFalse(report["accepted"])
        self.assertEqual(report["gate_failures"][0]["check"], "reported_reached_target")

    def test_a_changed_case_input_is_refused(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["input"]["starting_pot"] = _fixture.POT + 1
        with self.assertRaises(ValueError):
            self.gate(project=project)

    def test_an_unexplained_frequency_difference_is_refused(self):
        project = _fixture.project_capture(self.reference, root_check_frequency=0.40)
        report = self.gate(project=project)
        self.assertFalse(report["accepted"])
        self.assertEqual(
            len(report["rows_missing_review_reasoning"]), len(_fixture.OOP_HANDS)
        )

    def test_a_capture_with_no_commit_is_refused_even_without_an_expected_one(self):
        project = copy.deepcopy(self.project)
        project["cases"][0]["reached_target"] = True
        project["project_revision"] = "local-unbound-capture"
        report = self.gate(project=project, revision=None)
        self.assertFalse(report["accepted"])
        self.assertEqual(report["gate_failures"][0]["check"], "project_revision")

    def test_a_capture_of_another_commit_is_refused(self):
        report = self.gate(revision="1" * 40)
        self.assertFalse(report["accepted"])
        self.assertEqual(
            report["gate_failures"][0]["reason"],
            "the capture measured a different commit",
        )

    def test_a_missing_project_capture_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "reference.json").write_text(
                json.dumps(self.reference), encoding="utf-8"
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("compare.py")),
                    "--reference",
                    str(root / "reference.json"),
                    "--project",
                    str(root / "never-written.toml"),
                    "--output",
                    str(root / "out.json"),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or was never written", result.stderr)


class StaleReviewTests(unittest.TestCase):
    """A reasoning sentence is about numbers; re-solve either side and it is about nothing."""

    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(
            self.reference, root_check_frequency=0.40
        )
        self.report = compare(self.project, self.reference)

    def _review(self, mutate=None):
        rows = []
        for row in self.report["cases"][0]["differences"]:
            entry = {
                "history": row["history"],
                "cards": row["cards"],
                "actions": row["actions"],
                "final_frequency_differences": list(
                    row["absolute_frequency_difference"]
                ),
                "project_refined": {"strategy": list(row["project_strategy"])},
                "reference_refined": {"strategy": list(row["reference_strategy"])},
                "review_reasoning": "Both actions are within a hundredth of a chip.",
            }
            rows.append(entry)
        if mutate is not None:
            mutate(rows[0])
        return {"cases": [{"id": "turn_fixture", "rows": rows}]}

    def test_a_current_review_passes(self):
        review = self._review()
        self.assertEqual(uncovered_rows(self.report, review), [])
        self.assertEqual(stale_reviews(self.report, review), [])
        report = joint_report(self.project, self.reference, review, REVISION)
        self.assertTrue(report["accepted"])

    def test_a_review_recorded_against_an_older_solve_is_rejected(self):
        def bump(row):
            row["project_refined"]["strategy"][0] += 0.05

        review = self._review(bump)
        stale = stale_reviews(self.report, review)
        self.assertEqual(len(stale), 1)
        self.assertEqual(stale[0]["field"], "project_refined.strategy")
        report = joint_report(self.project, self.reference, review, REVISION)
        self.assertFalse(report["accepted"])

    def test_a_review_recording_a_different_menu_is_rejected(self):
        def rename(row):
            row["actions"] = ["check", "bet:99"]

        stale = stale_reviews(self.report, self._review(rename))
        self.assertEqual([entry["field"] for entry in stale], ["actions"])

    def test_a_review_that_records_no_values_cannot_be_checked(self):
        review = {
            "cases": [
                {
                    "id": "turn_fixture",
                    "rows": [
                        {
                            "history": row["history"],
                            "cards": row["cards"],
                            "review_reasoning": "Looks fine.",
                        }
                        for row in self.report["cases"][0]["differences"]
                    ],
                }
            ]
        }
        stale = stale_reviews(self.report, review)
        self.assertEqual(len(stale), len(_fixture.OOP_HANDS))
        self.assertIn("records no row values", stale[0]["reason"])
        self.assertFalse(
            joint_report(self.project, self.reference, review, REVISION)["accepted"]
        )


class ConvergenceAndRevisionTests(unittest.TestCase):
    def test_both_sides_are_held_to_the_cases_own_target(self):
        reference = _fixture.reference_capture()
        project = _fixture.project_capture(reference)
        self.assertEqual(convergence_failures(project, reference), [])

    def test_a_capture_with_no_revision_field_is_refused(self):
        self.assertEqual(
            revision_failures({}, None)[0]["check"],
            "project_revision",
        )


if __name__ == "__main__":
    unittest.main()
