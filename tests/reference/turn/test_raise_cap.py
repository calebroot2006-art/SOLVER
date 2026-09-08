"""Guards on the derived `removed_lines` pruning. No WASM build is needed.

The amounts asserted here are worked out by hand from postflop-solver's `push_actions`
(`src/action_tree.rs:526-737`) and cross-checked against a real capture: run 34074994221 saw
`bet:4`, `raise:23`, `raise:80` and `allin:195` on one turn street of the committed cases,
which is the unpruned chain the first two tests below rebuild.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import _fixture
import capture
import compare
import raise_cap

ROOT = Path(__file__).resolve().parent
CASES = json.loads((ROOT / "cases.json").read_text(encoding="utf-8"))["cases"]

# One bet and one raise per street: 18 lines, the same 18 for every committed case, because
# the three cases differ only in their board and the amounts depend on the pot and the stack.
EXPECTED_ONE_RAISE = [
    "X-X-X-B4-R23-R80",
    "X-X-X-B8-R35-R116",
    "X-X-B4-R23-R80",
    "X-X-B8-R35-R116",
    "X-B4-C-X-B6-R37-R130",
    "X-B4-C-X-B14-R61-A191",
    "X-B4-C-B6-R37-R130",
    "X-B4-C-B14-R61-A191",
    "X-B4-R23-C-X-B19-R114-A172",
    "X-B4-R23-C-B19-R114-A172",
    "X-B4-R23-R80",
    "B4-C-X-B6-R37-R130",
    "B4-C-X-B14-R61-A191",
    "B4-C-B6-R37-R130",
    "B4-C-B14-R61-A191",
    "B4-R23-C-X-B19-R114-A172",
    "B4-R23-C-B19-R114-A172",
    "B4-R23-R80",
]


class RoundingTests(unittest.TestCase):
    def test_ties_go_away_from_zero_like_rust(self):
        # Python's own round() gives 2 and -2 for these; Rust's f64::round gives 3 and -3.
        self.assertEqual(raise_cap.round_half_away(2.5), 3)
        self.assertEqual(raise_cap.round_half_away(-2.5), -3)
        self.assertEqual(raise_cap.round_half_away(3.63), 4)
        self.assertEqual(raise_cap.round_half_away(8.25), 8)
        self.assertEqual(raise_cap.round_half_away(0.0), 0)
        with self.assertRaises(ValueError):
            raise_cap.round_half_away(float("inf"))

    def test_the_turn_menu_amounts_match_the_measured_capture(self):
        case = CASES[0]
        config = raise_cap.config_from_case(case)
        info = raise_cap.Info.new(config.effective_stack)
        opening = raise_cap.push_actions("turn", 0, 0, info, config)
        # 33% of an 11-chip pot rounds to 4; `a` is the whole 195 behind.
        self.assertEqual(opening, [("check", None), ("bet", 4), ("allin", 195)])
        after_bet = info.create_next(0, ("bet", 4))
        facing = raise_cap.push_actions("turn", 1, 0, after_bet, config)
        self.assertEqual(facing, [("fold", None), ("call", None), ("raise", 23)])
        after_raise = after_bet.create_next(1, ("raise", 23))
        facing_raise = raise_cap.push_actions("turn", 0, 4, after_raise, config)
        self.assertEqual(facing_raise, [("fold", None), ("call", None), ("raise", 80)])
        # The fourth wager clamps to the stack, so upstream stores it as an all-in.
        after_reraise = after_raise.create_next(0, ("raise", 80))
        self.assertEqual(
            raise_cap.push_actions("turn", 1, 23, after_reraise, config),
            [("fold", None), ("call", None), ("allin", 195)],
        )

    def test_facing_an_all_in_offers_no_raise(self):
        case = CASES[0]
        config = raise_cap.config_from_case(case)
        info = raise_cap.Info.new(config.effective_stack).create_next(0, ("allin", 195))
        self.assertEqual(
            raise_cap.push_actions("turn", 1, 0, info, config),
            [("fold", None), ("call", None)],
        )


class SizeMenuTests(unittest.TestCase):
    def test_percentages_and_all_in_parse_and_sort(self):
        self.assertEqual(raise_cap.parse_sizes(""), [])
        self.assertEqual(
            raise_cap.parse_sizes("75%,33%"), [("pot", 0.33), ("pot", 0.75)]
        )
        self.assertEqual(
            raise_cap.parse_sizes("33%,a"), [("pot", 0.33), ("allin", 0.0)]
        )

    def test_size_kinds_the_derivation_cannot_express_are_rejected(self):
        for text in ("50", "2e", "60c", "3x", "0.5"):
            with self.assertRaises(ValueError, msg=text):
                raise_cap.parse_sizes(text)

    def test_donk_sizes_are_refused_rather_than_ignored(self):
        case = json.loads(json.dumps(CASES[0]))
        case["menus"]["turn"]["oop_donk"] = "50%"
        with self.assertRaises(ValueError):
            raise_cap.config_from_case(case)


class DerivationTests(unittest.TestCase):
    def test_every_committed_case_prunes_the_same_eighteen_lines(self):
        for case in CASES:
            self.assertEqual(case["max_raises"], 1, case["id"])
            self.assertEqual(
                raise_cap.derive_removed_lines(case), EXPECTED_ONE_RAISE, case["id"]
            )

    def test_the_pruned_tree_holds_one_bet_and_one_raise_per_street(self):
        for case in CASES:
            self.assertEqual(raise_cap.max_street_wagers(case), 2, case["id"])

    def test_the_unpruned_tree_reaches_four_wagers_on_a_street(self):
        # Which is why a cap of one cannot be captured without pruning: run 34079922254.
        case = CASES[0]
        self.assertEqual(raise_cap.derive_removed_lines(case, 32), [])
        self.assertEqual(raise_cap.max_street_wagers(case, 32), 4)

    def test_a_cap_of_zero_leaves_a_bet_that_can_only_be_folded_to_or_called(self):
        case = CASES[0]
        lines = raise_cap.derive_removed_lines(case, 0)
        self.assertEqual(raise_cap.max_street_wagers(case, 0), 1)
        self.assertTrue(
            all(line.split("-")[-1].startswith(("R", "A")) for line in lines)
        )
        self.assertIn("B4-R23", lines)
        self.assertNotIn("B4-R23-R80", lines)

    def test_no_derived_line_sits_under_another(self):
        # Order-independence: removing one line must never delete another line's parent.
        lines = [line.split("-") for line in raise_cap.derive_removed_lines(CASES[0])]
        for index, line in enumerate(lines):
            for other in lines[:index] + lines[index + 1 :]:
                self.assertNotEqual(line[: len(other)], other)

    def test_every_removed_line_ends_on_the_wager_the_cap_forbids(self):
        # A line can span two streets, so count the wagers at its tail, not in the whole
        # line: `X-B4-C-X-B6-R37-R130` is one turn wager and then three on the river.
        for line in raise_cap.derive_removed_lines(CASES[0]):
            tokens = line.split("-")
            self.assertTrue(tokens[-1][0] in "RA", line)
            trailing = 0
            for token in reversed(tokens):
                if token[0] not in "BRA":
                    break
                trailing += 1
            self.assertEqual(trailing, 3, line)

    def test_a_cap_outside_zero_to_thirty_two_is_rejected(self):
        for cap in (-1, 33, 1.0, True):
            with self.assertRaises(ValueError, msg=str(cap)):
                raise_cap.derive_removed_lines(CASES[0], cap)

    def test_the_argument_string_is_comma_separated(self):
        lines = raise_cap.derive_removed_lines(CASES[0])
        argument = raise_cap.removed_lines_argument(lines)
        self.assertEqual(argument.split(","), lines)
        with self.assertRaises(ValueError):
            raise_cap.removed_lines_argument(["B4,R23"])
        with self.assertRaises(ValueError):
            raise_cap.removed_lines_argument([""])

    def test_the_fixture_pot_and_stack_derive_their_own_amounts(self):
        # A 10-chip pot and a 40-chip stack: 33% is 3, and the third wager is already all-in.
        lines = raise_cap.derive_removed_lines(_fixture.case_input(), 1)
        self.assertIn("B3-R19-A40", lines)
        self.assertEqual(raise_cap.max_street_wagers(_fixture.case_input(), 1), 2)


class InputContractTests(unittest.TestCase):
    """`max_raises` is now a setting the capture acts on, so its range check moved."""

    def payload(self, **overrides):
        case = json.loads(json.dumps(CASES[0]))
        case.update(overrides)
        return {
            "schema_version": 1,
            "street": "turn",
            "ranges_provenance": "x" * 60,
            "cases": [case],
        }

    def test_zero_raises_is_accepted(self):
        capture.validate_inputs(self.payload(max_raises=0))

    def test_thirty_two_raises_is_accepted(self):
        capture.validate_inputs(self.payload(max_raises=32))

    def test_a_negative_or_oversized_cap_is_rejected(self):
        for cap in (-1, 33):
            with self.assertRaises(ValueError, msg=str(cap)):
                capture.validate_inputs(self.payload(max_raises=cap))

    def test_a_boolean_cap_is_rejected(self):
        with self.assertRaises(ValueError):
            capture.validate_inputs(self.payload(max_raises=True))

    def test_the_committed_cases_still_pass_the_input_contract(self):
        capture.validate_inputs(
            json.loads((ROOT / "cases.json").read_text(encoding="utf-8"))
        )


class OutputContractTests(unittest.TestCase):
    def setUp(self):
        self.reference = _fixture.reference_capture()
        self.payload = {"cases": [case["input"] for case in self.reference["cases"]]}

    def validate(self, reference):
        capture.validate_output(
            reference, {"cases": [c["input"] for c in reference["cases"]]}
        )

    def test_the_fixture_capture_declares_the_lines_the_derivation_gives(self):
        self.validate(self.reference)

    def test_a_capture_that_pruned_something_else_is_rejected(self):
        reference = _fixture.reference_capture()
        reference["cases"][0]["removed_lines"] = ["B3-R19-A40"]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_a_capture_missing_the_field_is_rejected(self):
        reference = _fixture.reference_capture()
        del reference["cases"][0]["removed_lines"]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_a_lowered_cap_needs_its_derived_lines(self):
        reference = _fixture.reference_capture()
        case = reference["cases"][0]
        case["input"]["max_raises"] = 1
        with self.assertRaises(ValueError):
            self.validate(reference)
        case["removed_lines"] = raise_cap.derive_removed_lines(case["input"])
        self.validate(reference)

    def test_a_second_raise_on_one_street_fails_the_export_check(self):
        reference = _fixture.reference_capture()
        case = reference["cases"][0]
        case["input"]["max_raises"] = 1
        case["removed_lines"] = raise_cap.derive_removed_lines(case["input"])
        node = next(
            node
            for node in case["nodes"]
            if node["history_labels"][-2:] == ["bet:3", "raise:10"]
        )
        node["history_labels"] = node["history_labels"] + ["raise:30"]
        node["history"] = node["history"] + [2]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_a_wager_still_offered_at_the_cap_fails_the_export_check(self):
        reference = _fixture.reference_capture()
        case = reference["cases"][0]
        case["input"]["max_raises"] = 1
        case["removed_lines"] = raise_cap.derive_removed_lines(case["input"])
        node = next(
            node
            for node in case["nodes"]
            if node["history_labels"][-2:] == ["bet:3", "raise:10"]
        )
        node["actions"] = node["actions"] + [
            {"kind": "raise", "label": "raise:30", "amount": 30}
        ]
        with self.assertRaises(ValueError):
            self.validate(reference)

    def test_a_river_bet_is_not_charged_against_the_turn(self):
        # Two streets, each at the cap, is legal; the tally restarts at every deal.
        labels = ["bet:4", "raise:23", "call", "chance:4c", "bet:6", "raise:37"]
        self.assertEqual(capture.street_wagers(labels), [2, 2])
        self.assertEqual(capture.street_wagers([]), [0])
        self.assertEqual(capture.street_wagers(["check", "check", "chance:4c"]), [0, 0])


class SummaryTests(unittest.TestCase):
    """The comparison's own record of the pruning, read from the committed artifact."""

    def test_the_summary_records_the_cap_and_the_lines(self):
        report = compare.summarize_reference(_fixture.reference_capture())
        case = report["cases"][0]
        self.assertEqual(case["max_raises"], 32)
        self.assertEqual(case["removed_lines"], [])
        self.assertEqual(case["max_street_wagers"], 2)

    def test_the_comparison_names_the_case_and_history_it_rejects(self):
        case = _fixture.reference_capture()["cases"][0]
        case["input"]["max_raises"] = 1
        compare.require_raise_cap(case)
        node = next(
            node
            for node in case["nodes"]
            if node["history_labels"][-2:] == ["bet:3", "raise:10"]
        )
        node["history_labels"] = node["history_labels"] + ["raise:30"]
        with self.assertRaises(ValueError) as caught:
            compare.require_raise_cap(case)
        self.assertIn("turn_fixture", str(caught.exception))
        self.assertIn("max_raises 1", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
