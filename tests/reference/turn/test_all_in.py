"""Called-turn-all-in values from all legal private-pair river deals."""

import copy
import math
import unittest
from itertools import product

import _fixture
from compare import called_all_in_checks, joint_report, reference_all_in_roundoff
from oracle import AllInEquities, all_in_equities, expand, weighted_value_bounds
from review_rule import load_rules


class EquityTests(unittest.TestCase):
    def test_known_win_tie_and_blocked_river_counts(self):
        board = ("Ac", "Kd", "Qh", "Js")
        hero = ("2c", "Tc")
        tie, loser = ("3d", "Td"), ("8h", "8s")
        table = AllInEquities(board, ((hero,), (tie, loser)))
        self.assertEqual(table.private_pairs, 2)
        self.assertEqual(table.share(0, hero, tie), 0.5)  # Both have Broadway.
        # Only the three unheld tens put Broadway on the board: 41 wins, 3 ties.
        self.assertEqual(table.share(0, hero, loser), 85 / 88)
        self.assertAlmostEqual(table.share(1, loser, hero), 3 / 88, places=15)
        self.assertNotIn("Tc", table.possible_cards)
        self.assertNotIn("2c", table.possible_cards)
        self.assertEqual(len(table.possible_cards), 46)

    def test_quads_win_every_river_and_overlapping_private_cards_are_excluded(self):
        hero = ("Ah", "As")
        table = AllInEquities(
            ("Ac", "Ad", "Kh", "Qh"), ((hero,), (("Kc", "Kd"), ("As", "Kc")))
        )
        self.assertEqual(table.private_pairs, 1)
        self.assertEqual(len(table.possible_cards), 44)
        self.assertEqual(table.share(0, hero, ("Kc", "Kd")), 1)
        with self.assertRaises(ValueError):
            table.share(0, hero, ("As", "Kc"))

    def test_cache_is_bounded_and_keyed_only_by_board_and_hands(self):
        self.assertEqual(all_in_equities.cache_info().maxsize, 3)
        board = ("Ac", "Ad", "Kh", "Qh")
        hands = ((("Ah", "As"),), (("Kc", "Kd"),))
        self.assertIs(all_in_equities(board, hands), all_in_equities(board, hands))

    def test_reference_weight_interval_extrema_match_all_box_corners(self):
        values = [-5.0, 2.0, 20.0]
        for bounds in ([(1, 2), (2, 3), (0.1, 0.2)], [(0, 2), (0, 3), (0, 0.2)]):
            corners = [
                sum(w * v for w, v in zip(weights, values)) / sum(weights)
                for weights in product(*bounds)
                if sum(weights) > 0
            ]
            low, high = weighted_value_bounds(values, bounds)
            self.assertAlmostEqual(low, min(corners), places=12)
            self.assertAlmostEqual(high, max(corners), places=12)
        self.assertIsNone(weighted_value_bounds([100], [(0, 0)]))


class OpponentReachTests(unittest.TestCase):
    @staticmethod
    def capture(call_tens, call_eights, reference):
        # This isolated numerical fixture starts with IP facing a jam. Root
        # action EVs are unused: the unit under test checks only the called leaf.
        board = ["Ac", "Kd", "Qh", "Js"]
        hands = [sorted(expand(r, board)) for r in ("TT", "TT,88")]
        probabilities = [call_tens if h[0][0] == "T" else call_eights for h in hands[1]]
        # TT ties the only compatible TT. Against each of six 88 combos, TT
        # wins on 42 rivers and ties on the two unheld tens: equity 43/44.
        win = 90 * (43 / 44) - 45
        value = win * (6 * call_eights) / (call_tens + 6 * call_eights)
        values = [[value] * 6, [0 if h[0][0] == "T" else -win for h in hands[1]]]
        root = {
            "history_labels": [],
            "kind": "decision",
            "street": "turn",
            "runout": "",
            "player": 1,
            "actions": ["fold", "call"],
            "contributions": [40, 0],
        }
        leaf = {
            "history_labels": ["call"],
            "kind": "terminal",
            "street": "turn",
            "runout": "",
            "terminal": "showdown",
            "contributions": [40, 40],
            "actions": [],
            "player": -1,
            "wasm_empty_range_flag": 0,
        }
        if reference:
            root["strategy"] = [1 - p for p in probabilities] + probabilities
            leaf["expected_values"] = [[v + 45 for v in side] for side in values]
            leaf["ev_available"] = [[True] * len(side) for side in hands]
        else:
            root["hands"] = [
                {"cards": h, "strategy": [1 - p, p]}
                for h, p in zip(hands[1], probabilities)
            ]
            leaf["hands"] = [
                {
                    "player": player,
                    "cards": h,
                    "ev_available": True,
                    "expected_value": value,
                }
                for player in (0, 1)
                for h, value in zip(hands[player], values[player])
            ]
        case = {
            "input": {
                "id": "mixed",
                "board": board,
                "ranges": ["TT", "TT,88"],
                "starting_pot": 10,
                "effective_stack": 40,
            },
            "private_cards": [[{"cards": h} for h in side] for side in hands],
            "nodes": [root, leaf],
        }
        return {"cases": [case], "presentation_mode": "upstream_display"}

    def test_each_capture_uses_its_own_compatible_opponent_policy(self):
        project = self.capture(0.25, 0.75, False)
        reference = self.capture(0.2, 0.8, True)
        reports, failures = called_all_in_checks(project, reference)
        self.assertEqual(failures, [])
        self.assertEqual(reports[0]["unique_private_pairs"], 42)
        self.assertEqual(reports[0]["histories"][0]["project_checked_values"], 18)
        # Changing just the opponent policy makes the unchanged conditional EV false.
        project["cases"][0]["nodes"][0]["hands"][0]["strategy"] = [0.3, 0.7]
        self.assertIn(
            "project all-in EV differs",
            str(called_all_in_checks(project, reference)[1]),
        )


class AllInGateTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.project = _fixture.project_capture(self.reference)
        self.rules = load_rules()
        self.review = {
            "schema_version": 2,
            "street": "turn",
            "rule": self.rules,
            "cases": [],
        }

    def gate(self):
        return joint_report(
            self.project, self.reference, self.review, "0" * 40, self.rules
        )

    def checks(self):
        return called_all_in_checks(self.project, self.reference)

    def parent(self, reference=False):
        capture = self.reference if reference else self.project
        return next(
            n
            for n in capture["cases"][0]["nodes"]
            if n["history_labels"] == ["allin:40", "call"]
        )

    def as_chance_representation(self):
        case = self.project["cases"][0]
        for node in list(case["nodes"]):
            if (
                node["kind"] != "terminal"
                or node["terminal"] != "showdown"
                or node["street"] != "turn"
            ):
                continue
            node.update(
                kind="chance",
                terminal="",
                possible_cards=[
                    r + s
                    for r in "23456789TJQKA"
                    for s in "cdhs"
                    if r + s not in _fixture.BOARD
                ],
            )
            case["nodes"].append(
                {
                    "history_labels": node["history_labels"] + ["chance:4c"],
                    "kind": "terminal",
                    "terminal": "showdown",
                    "street": "river",
                    "runout": "4c",
                    "contributions": [40, 40],
                    "actions": [],
                    "player": -1,
                    "fold_winner": -1,
                }
            )

    def test_both_project_representations_check_all_hands_and_runouts(self):
        for chance in (False, True):
            with self.subTest(chance=chance):
                if chance:
                    self.as_chance_representation()
                result = self.gate()
                self.assertTrue(result["accepted"], result["gate_failures"])
                report = result["called_all_in_oracle"][0]
                self.assertEqual(report["unique_private_pairs"], 72)
                self.assertEqual(report["unique_pair_runouts"], 72 * 44)
                self.assertEqual(len(report["histories"]), 2)
                for history in report["histories"]:
                    self.assertEqual(history["project_checked_values"], 18)
                    self.assertEqual(history["reference_checked_values"], 18)
                    self.assertLess(history["project_max_abs_error_chips"], 1e-12)
                    self.assertEqual(
                        history["reference_max_interval_violation_chips"], 0
                    )

    def test_project_parent_ev_mutation_rejected_in_both_representations(self):
        baseline = copy.deepcopy(self.project)
        for chance in (False, True):
            with self.subTest(chance=chance):
                self.project = copy.deepcopy(baseline)
                if chance:
                    self.as_chance_representation()
                self.parent()["hands"][0]["expected_value"] += 1.0
                result = self.gate()
                self.assertFalse(result["accepted"])
                self.assertIn("project all-in EV differs", str(result["gate_failures"]))
                self.assertEqual(
                    result["cases"][0]["frequency_rows_requiring_review"], 0
                )

    def test_reference_parent_ev_mutation_rejected(self):
        self.parent(reference=True)["expected_values"][0][0] += 1.0
        result = self.gate()
        self.assertFalse(result["accepted"])
        self.assertIn("reference all-in EV", str(result["gate_failures"]))
        self.assertEqual(result["cases"][0]["frequency_rows_requiring_review"], 0)

    def test_conditional_value_does_not_depend_on_own_action_reach(self):
        case = self.project["cases"][0]
        for row in case["nodes"][0]["hands"]:
            row["strategy"] = [1.0, 0.0]
        _fixture.refresh_project_reach(case)
        reports, failures = self.checks()
        self.assertEqual(failures, [])
        # OOP never jams, but every OOP parent value is conditional on IP's call.
        rows = [h for h in self.parent()["hands"] if h["player"] == 0]
        self.assertTrue(
            all(h["ev_available"] and h["expected_value"] == 45 for h in rows)
        )
        self.assertEqual(reports[0]["histories"][1]["project_checked_values"], 6)
        self.assertEqual(reports[0]["histories"][1]["project_unavailable_values"], 12)

    def test_false_missing_project_value_is_not_a_zero(self):
        self.parent()["hands"][0].update(ev_available=False)
        self.parent()["hands"][0].pop("expected_value")
        self.assertFalse(self.gate()["accepted"])

    def test_false_missing_reference_value_is_not_excused(self):
        node = self.parent(reference=True)
        node["ev_available"][0][0] = False
        node["expected_values"][0][0] = None
        result = self.gate()
        self.assertFalse(result["accepted"])
        self.assertIn("reference all-in availability", str(result["gate_failures"]))

    def test_reference_rounded_ev_is_enclosed_but_material_error_is_not(self):
        node = self.parent(reference=True)
        node["expected_values"][0][0] = 90 + 2**-18
        self.assertTrue(self.gate()["accepted"])
        node["expected_values"][0][0] = 90.01
        self.assertFalse(self.gate()["accepted"])

    def test_all_in_public_river_omission_or_duplication_is_rejected(self):
        self.as_chance_representation()
        parent = self.parent()
        parent["possible_cards"].pop()
        self.assertIn("public river set", str(self.checks()[1]))
        parent["possible_cards"].append(parent["possible_cards"][0])
        self.assertIn("public river set", str(self.checks()[1]))

    def test_nonfinite_parent_value_is_rejected(self):
        self.parent()["hands"][0]["expected_value"] = math.inf
        with self.assertRaises(ValueError):
            self.gate()

    def test_turn_showdown_without_called_stack_is_rejected(self):
        self.parent()["contributions"] = [39, 39]
        self.assertIn("not a called all-in", str(self.checks()[1]))

    def test_raw_subnormal_normalization_is_an_explicit_refusal(self):
        # A positive raw f32 normalized weight can be subnormal. The relative
        # normalization proof cannot accept it as an infinite-width interval.
        self.assertEqual(
            reference_all_in_roundoff(
                45, 2**-140, 2**-140, 2**-140, 1, 1, 2**-130, 1, False
            ),
            math.inf,
        )
        self.assertEqual(
            reference_all_in_roundoff(2**31, 1, 1, 1, 1, 1, 2**-126, 1, False),
            math.inf,
        )
        self.reference["presentation_mode"] = "raw_f32"
        root = self.reference["cases"][0]["nodes"][0]
        count = len(_fixture.OOP_HANDS)
        root["strategy"] = [1.0] * count + [2**-140] * count
        self.assertIn("finite normal-range enclosure", str(self.checks()[1]))

    def test_tiny_rounded_joint_mass_cannot_claim_an_available_value(self):
        ref_case = self.reference["cases"][0]
        root = ref_case["nodes"][0]
        count = len(_fixture.OOP_HANDS)
        root["strategy"] = [1 - 1e-9] * count + [1e-9] * count
        node = self.parent(reference=True)
        # Even allowing the omitted half-micro probability, this individual
        # joint mass is below display precision. The whole side also truncates.
        node["wasm_empty_range_flag"] = 1
        node["ev_available"] = [[False] * len(side) for side in _fixture.HANDS]
        node["expected_values"] = [None, None]
        self.assertEqual(self.checks()[1], [])
        node["ev_available"][0][0] = True
        node["expected_values"][0] = [90.0] * count
        self.assertIn("all-in availability", str(self.checks()[1]))

    def test_selected_child_board_mismatch_is_rejected(self):
        self.as_chance_representation()
        self.project["cases"][0]["nodes"][-1]["runout"] = "5c"
        self.assertIn("named river", str(self.checks()[1]))


if __name__ == "__main__":
    unittest.main()
