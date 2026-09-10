"""The scalar turn oracle: exact inside a runout, reported values across the chance layer."""

from __future__ import annotations

import random
import unittest
from itertools import combinations

import _fixture
from oracle import MissingReportedValue, Oracle, five, key, seven

RIVER_HISTORY = ("check", "check", f"chance:{_fixture.RUNOUT}")
HERO = ("Ad", "Ah")


def brute_force_seven(cards):
    return max(five(hand) for hand in combinations(cards, 5))


class HandEvaluationTests(unittest.TestCase):
    def test_seven_matches_a_brute_force_five_card_maximum(self):
        deck = [rank + suit for rank in "23456789TJQKA" for suit in "cdhs"]
        rng = random.Random(20260906)
        for _ in range(300):
            hand = tuple(rng.sample(deck, 7))
            self.assertEqual(seven(hand), brute_force_seven(hand))

    def test_known_category_order(self):
        wheel = five(("As", "2c", "3d", "4h", "5s"))
        flush = five(("As", "Js", "8s", "4s", "2s"))
        quads = five(("As", "Ac", "Ad", "Ah", "2s"))
        self.assertLess(wheel, flush)
        self.assertLess(flush, quads)


class RangeExpansionTests(unittest.TestCase):
    def test_board_cards_are_removed(self):
        weights = Oracle(_fixture.reference_capture()["cases"][0], True).weights
        self.assertEqual(len(weights[0]), len(_fixture.OOP_HANDS))
        self.assertEqual(len(weights[1]), len(_fixture.IP_HANDS))
        self.assertNotIn(key(("Ac", "Ad")), weights[0])


class ExactRunoutTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.oracle = Oracle(self.reference["cases"][0], True)

    def test_action_values_inside_a_runout_are_independent(self):
        values = self.oracle.action_values(RIVER_HISTORY, HERO)
        self.assertEqual(
            values["continuation_sources"], ["exact_fold", "independent_showdown"]
        )
        self.assertAlmostEqual(values["counterfactual_action_ev"][0], 5.0, places=9)
        self.assertAlmostEqual(values["counterfactual_action_ev"][1], 7.43, places=9)

    def test_own_reach_follows_the_exported_policy(self):
        values = self.oracle.action_values(RIVER_HISTORY, HERO)
        self.assertAlmostEqual(values["own_history_reach"], 0.75, places=9)
        self.assertAlmostEqual(values["compatible_opponent_mass"], 6.0, places=9)

    def test_hero_may_not_hold_the_dealt_runout(self):
        # A runout of Ad makes the AdAh combo dead on the river; its history is unreachable.
        oracle = Oracle(_fixture.reference_capture(runout="Ad")["cases"][0], True)
        with self.assertRaises(ValueError):
            oracle.action_values(("check", "check", "chance:Ad"), HERO)


class TurnRoundTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.oracle = Oracle(self.reference["cases"][0], True)

    def test_turn_round_values_use_the_reported_chance_node_value(self):
        values = self.oracle.action_values((), HERO)
        self.assertIn("reported_chance_node_ev", values["continuation_sources"])
        self.assertAlmostEqual(values["counterfactual_action_ev"][0], -1.0, places=9)
        self.assertAlmostEqual(values["counterfactual_action_ev"][1], 2.5, places=9)

    def test_metrics_do_not_claim_an_exploitability(self):
        metrics = self.oracle.metrics()
        self.assertNotIn("best_response_values", metrics)
        self.assertEqual(metrics["compatible_root_mass"], 72.0)
        self.assertGreater(metrics["reported_value_leaves"], 0)


class ReportedValueTests(unittest.TestCase):
    """What the capture reports at a leaf the walk cannot cross, and what happens
    when it reports nothing. Before `PostflopStrategy::node_values` existed the
    capture reported nothing and the walk substituted zero, which is how the root
    row `2c2d` of `turn_100bb_dry_rainbow` read 2.23 chips instead of 14.96."""

    def project(self, **edit):
        """A project capture, optionally with its chance-node rows edited."""
        case = _fixture.project_capture(_fixture.reference_capture())["cases"][0]
        for node in case["nodes"]:
            if node["kind"] == "chance":
                for hand in node["hands"]:
                    hand.update(edit)
                    if edit.get("ev_available") is False:
                        hand.pop("expected_value", None)
        return case

    def root_check_value(self, reported):
        case = self.project(expected_value=reported)
        return Oracle(case).action_values((), HERO)["counterfactual_action_ev"][0]

    def test_the_reported_value_carries_the_continuation_at_its_own_reach(self):
        # The in-position player checks behind half the time, so half of the
        # check line's value is whatever the capture says the river deal is
        # worth. That coefficient, not the value, is the oracle's own work.
        self.assertAlmostEqual(self.root_check_value(0.0), -1.0, places=12)
        self.assertAlmostEqual(self.root_check_value(2.0), 0.0, places=12)
        self.assertAlmostEqual(
            self.root_check_value(2.0) - self.root_check_value(0.0), 1.0, places=12
        )

    def test_a_capture_that_reports_no_values_is_refused(self):
        case = self.project()
        for node in case["nodes"]:
            if node["kind"] == "chance":
                node["hands"] = []
        oracle = Oracle(case)
        with self.assertRaises(MissingReportedValue) as refusal:
            oracle.action_values((), HERO)
        self.assertIn("no value for player 0", str(refusal.exception))

    def test_an_unavailable_value_is_refused_rather_than_read_as_zero(self):
        oracle = Oracle(self.project(ev_available=False))
        with self.assertRaises(MissingReportedValue):
            oracle.action_values((), HERO)

    def test_a_row_claiming_a_value_it_does_not_report_is_refused(self):
        case = self.project()
        for node in case["nodes"]:
            if node["kind"] == "chance":
                node["hands"][0].pop("expected_value")
        with self.assertRaises(ValueError):
            Oracle(case)

    def test_no_compatible_opposing_hand_needs_no_reported_value(self):
        # Every in-position hand is a queen or a jack, so an opponent range of
        # one hand leaves the rest of the walk nothing to be worth anything
        # against: that zero is arithmetic, not a stand-in for a missing value.
        case = self.project(ev_available=False)
        oracle = Oracle(case)
        oracle.weights[1] = {key(_fixture.IP_HANDS[0]): 1.0}
        oracle.hands[1] = [key(_fixture.IP_HANDS[0])]
        values = oracle.action_values((), ("Qc", "Qd"))
        self.assertEqual(values["compatible_opponent_mass"], 0.0)
        self.assertIsNone(values["counterfactual_action_ev"])


class ProjectSideTests(unittest.TestCase):
    def test_project_and_reference_oracles_agree_on_identical_policies(self):
        reference = _fixture.reference_capture()
        project = _fixture.project_capture(reference)
        theirs = Oracle(reference["cases"][0], True)
        ours = Oracle(project["cases"][0])
        for history in ((), RIVER_HISTORY):
            a = theirs.action_values(history, HERO)
            b = ours.action_values(history, HERO)
            self.assertEqual(a["continuation_sources"], b["continuation_sources"])
            for x, y in zip(
                a["counterfactual_action_ev"],
                b["counterfactual_action_ev"],
                strict=True,
            ):
                self.assertAlmostEqual(x, y, places=9)


if __name__ == "__main__":
    unittest.main()
