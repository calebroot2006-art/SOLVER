"""The scalar turn oracle: exact inside a runout, reported values across the chance layer."""

from __future__ import annotations

import random
import unittest
from itertools import combinations

import _fixture
from oracle import Oracle, five, key, seven

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
