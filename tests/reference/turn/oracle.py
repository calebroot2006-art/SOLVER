"""Independent scalar evaluation of an exported turn capture, over explicit physical deals.

Scope, stated plainly because it is narrower than the river oracle's:

* Inside an exported river runout the evaluation is exact and independent. Terminal values
  come from a brute-force seven-card comparison against the five-card board, and the reach
  along the path is recomputed from the exported policies alone.
* A history in the turn round has continuations through 44 unexported runouts, so the walk
  stops at the chance node (and at an all-in called on the turn) and uses the value that
  capture reported there, converted out of the wrapper's display origin. Those rows are
  labelled `reported_chance_node_ev`; they are evidence, not an independent recomputation.
  A capture that reports no value there gets a `MissingReportedValue`, never a zero.
* This oracle therefore does not compute exploitability. Exploitability for a turn solve
  comes from the project's f64 best-response walk over every runout, and from the
  reference's own `exploitability()`; both are recorded by the capture.
"""

from __future__ import annotations

import math
import struct
from functools import lru_cache
from itertools import combinations

RANKS = "23456789TJQKA"
SUITS = "cdhs"


class MissingReportedValue(ValueError):
    """A leaf the walk cannot cross reported no value for the hand that needs one.

    Substituting zero here is the failure this class exists to prevent. A zero is a
    number a reviewer will read as evidence, and on the `3ffdae5` capture it turned
    the root row `2c2d` of `turn_100bb_dry_rainbow` from 14.96 chips into 2.23. The
    walk stops at a chance node because it cannot enumerate the 44 unexported
    runouts itself; if the capture did not say what the continuation is worth, there
    is no value to report and the oracle says so instead of inventing one.
    """


def key(cards):
    return tuple(sorted(cards))


def five(cards):
    ranks = sorted((RANKS.index(c[0]) + 2 for c in cards), reverse=True)
    counts = sorted(((ranks.count(r), r) for r in set(ranks)), reverse=True)
    unique = sorted(set(ranks), reverse=True)
    straight = 0
    if len(unique) == 5:
        if unique[0] - unique[4] == 4:
            straight = unique[0]
        elif unique == [14, 5, 4, 3, 2]:
            straight = 5
    flush = len({c[1] for c in cards}) == 1
    if flush and straight:
        return (8, straight)
    if counts[0][0] == 4:
        return (7, counts[0][1], counts[1][1])
    if [n for n, _ in counts] == [3, 2]:
        return (6, counts[0][1], counts[1][1])
    if flush:
        return (5, *ranks)
    if straight:
        return (4, straight)
    if counts[0][0] == 3:
        return (
            3,
            counts[0][1],
            *sorted((r for n, r in counts if n == 1), reverse=True),
        )
    pairs = sorted((r for n, r in counts if n == 2), reverse=True)
    kickers = sorted((r for n, r in counts if n == 1), reverse=True)
    if len(pairs) == 2:
        return (2, *pairs, *kickers)
    if pairs:
        return (1, *pairs, *kickers)
    return (0, *ranks)


@lru_cache(maxsize=262144)
def seven(cards):
    return max(five(hand) for hand in combinations(cards, 5))


def parse_class(text):
    if not 2 <= len(text) <= 3 or text[0] not in RANKS or text[1] not in RANKS:
        raise ValueError(f"Invalid range class: {text}")
    high, low = RANKS.index(text[0]), RANKS.index(text[1])
    kind = text[2] if len(text) == 3 else ""
    if high < low or kind not in ("", "s", "o") or (high == low and kind):
        raise ValueError(f"Invalid range class: {text}")
    return high, low, kind


def expand_expression(expression):
    if "-" in expression:
        parts = expression.split("-")
        if len(parts) != 2:
            raise ValueError(f"Invalid interval: {expression}")
        start, end = parse_class(parts[0]), parse_class(parts[1])
        if start[2] != end[2] or (start[0] == start[1]) != (end[0] == end[1]):
            raise ValueError(f"Invalid interval: {expression}")
        if start[0] == start[1]:
            low, high = sorted((start[0], end[0]))
            return [(rank, rank, "") for rank in range(low, high + 1)]
        low, high = sorted((start[1], end[1]))
        if start[0] == end[0]:
            return [(start[0], value, start[2]) for value in range(low, high + 1)]
        if start[0] - start[1] != end[0] - end[1]:
            raise ValueError(f"Invalid interval: {expression}")
        gap = start[0] - start[1]
        return [(value + gap, value, start[2]) for value in range(low, high + 1)]
    if expression.endswith("+"):
        high, low, kind = parse_class(expression[:-1])
        if high == low:
            return [(rank, rank, "") for rank in range(high, 13)]
        return [(high, value, kind) for value in range(low, high)]
    if len(expression) == 4:
        raise ValueError("The scalar oracle does not accept explicit two-card combos")
    return [parse_class(expression)]


def expand(text, board):
    """Weights keyed by (low label, high label) for combos not blocked by `board`."""
    deck = [rank + suit for rank in RANKS for suit in SUITS]
    result = {}
    for part in text.split(","):
        if not part.strip():
            raise ValueError("Empty comma item in range")
        for token in part.split():
            expression, _, suffix = token.partition(":")
            weight = float(suffix) if suffix else 1.0
            if not math.isfinite(weight) or not 0.0 <= weight <= 1.0:
                raise ValueError(f"Invalid weight: {token}")
            classes = expand_expression(expression)
            for low, high in combinations(deck, 2):
                suited = low[1] == high[1]
                for item in classes:
                    if (
                        RANKS.index(low[0]) == item[1]
                        and RANKS.index(high[0]) == item[0]
                        and (item[2] == "" or (item[2] == "s") == suited)
                    ):
                        pair = key((low, high))
                        if pair in result and result[pair] != weight:
                            raise ValueError(f"Conflicting assignment for {token}")
                        result[pair] = weight
                        break
    return {
        pair: weight
        for pair, weight in result.items()
        if weight > 0 and not set(pair).intersection(board)
    }


class Oracle:
    """One recursion per own hand; opponent hands are summed before maximizing."""

    def __init__(self, case, reference=False):
        self.input = case["input"]
        self.board = list(self.input["board"])
        self.reference = reference
        self.weights = [expand(text, self.board) for text in self.input["ranges"]]
        self.hands = [sorted(weights) for weights in self.weights]
        self.nodes = {tuple(node["history_labels"]): node for node in case["nodes"]}
        if len(self.nodes) != len(case["nodes"]):
            raise ValueError("Repeated scalar oracle history")
        self.private = case.get("private_cards") if reference else None
        self.rows = {}
        self.raw_rows = {}
        self._path_cache = {}
        self._compatible_cache = {}
        self.reported = {}
        self.max_normalization_adjustment = 0.0
        for history, node in self.nodes.items():
            if node["kind"] == "decision":
                self.rows[history] = self._policy_rows(history, node)
            elif self._is_leaf_with_reported_values(node):
                self.reported[history] = self._reported_values(node)
        self.ranks = {}
        self.mass = sum(
            self.weights[0][a] * self.weights[1][b]
            for a in self.hands[0]
            for b in self.hands[1]
            if not set(a).intersection(b)
        )
        if not self.mass:
            raise ValueError("Empty compatible private deals")

    # --- construction helpers ----------------------------------------------------------

    def node_board(self, node):
        return self.board + ([node["runout"]] if node.get("runout") else [])

    def live_hands(self, player, node):
        runout = node.get("runout")
        if not runout:
            return self.hands[player]
        return [hand for hand in self.hands[player] if runout not in hand]

    @staticmethod
    def _is_leaf_with_reported_values(node):
        return node["kind"] == "chance" or (
            node["kind"] == "terminal"
            and node["terminal"] == "showdown"
            and node["street"] == "turn"
        )

    def _reference_index(self, player):
        return {
            key(entry["cards"]): index
            for index, entry in enumerate(self.private[player])
        }

    def _policy_rows(self, history, node):
        player = node["player"]
        actions = len(node["actions"])
        if self.reference:
            index = self._reference_index(player)
            count = len(self.private[player])
            raw = {
                hand: [
                    node["strategy"][a * count + index[hand]] for a in range(actions)
                ]
                for hand in self.live_hands(player, node)
            }
        else:
            raw = {key(hand["cards"]): list(hand["strategy"]) for hand in node["hands"]}
            if len(raw) != len(node["hands"]):
                raise ValueError("Repeated scalar oracle private hand")
        if set(raw) != set(self.live_hands(player, node)):
            raise ValueError(f"Scalar oracle physical hand set differs at {history}")
        self.raw_rows[history] = raw
        rows = {}
        for hand, row in raw.items():
            if len(row) != actions or any(
                not math.isfinite(v) or not 0 <= v <= 1 for v in row
            ):
                raise ValueError("Invalid reported probability row")
            total = sum(row)
            if total <= 0 or abs(total - 1) > 0.00002:
                raise ValueError("Invalid reported row normalization")
            normalized = [v / total for v in row]
            self.max_normalization_adjustment = max(
                self.max_normalization_adjustment,
                *(abs(a - b) for a, b in zip(row, normalized, strict=True)),
            )
            rows[hand] = normalized
        return rows

    def _reported_values(self, node):
        """Per-player, per-hand centered expected values reported at a leaf we cannot walk.

        The reference reports the wrapper's display EV, whose origin is half the starting pot
        plus the player's own contribution; the project reports the centered value directly.
        """
        pot = self.input["starting_pot"]
        values = [{}, {}]
        for player in (0, 1):
            origin = pot / 2 + node["contributions"][player]
            if self.reference:
                index = self._reference_index(player)
                row = node["expected_values"][player]
                available = node["ev_available"][player]
                for hand in self.live_hands(player, node):
                    position = index[hand]
                    value = row[position] if row is not None else None
                    values[player][hand] = (
                        None
                        if value is None or not available[position]
                        else value - origin
                    )
            else:
                for entry in node.get("hands", ()):
                    if entry["player"] != player:
                        continue
                    if entry["ev_available"] and "expected_value" not in entry:
                        raise ValueError(
                            "A capture row claims an available value and reports none"
                        )
                    values[player][key(entry["cards"])] = (
                        entry["expected_value"] if entry["ev_available"] else None
                    )
        return values

    # --- evaluation --------------------------------------------------------------------

    def rank(self, board, hand):
        cache_key = (tuple(board), hand)
        value = self.ranks.get(cache_key)
        if value is None:
            value = seven(tuple(board) + hand)
            self.ranks[cache_key] = value
        return value

    @staticmethod
    def actions(node):
        return [a if isinstance(a, str) else a["label"] for a in node["actions"]]

    def terminal(self, node, player, hero, opponent, sources):
        pot = self.input["starting_pot"]
        chips = node["contributions"]
        board = self.node_board(node)
        if node["terminal"] == "showdown" and node["street"] == "turn":
            return self.leaf(node, player, hero, opponent, sources)
        sources.add(
            "independent_showdown" if node["terminal"] == "showdown" else "exact_fold"
        )
        total = 0.0
        for villain, reach in opponent.items():
            if reach == 0 or set(hero).intersection(villain):
                continue
            if node["terminal"] == "fold":
                share = float(node["fold_winner"] == player)
            else:
                mine, theirs = self.rank(board, hero), self.rank(board, villain)
                share = 1.0 if mine > theirs else 0.5 if mine == theirs else 0.0
            total += reach * ((pot + sum(chips)) * share - pot / 2 - chips[player])
        return total

    def leaf(self, node, player, hero, opponent, sources):
        """Continuation value the capture reported, weighted by the compatible opposing mass.

        No compatible opposing hand reaches this leaf means the continuation contributes
        nothing, whatever it is worth, and that zero is arithmetic rather than a stand-in.
        Any other missing value is a refusal: see `MissingReportedValue`.
        """
        sources.add("reported_chance_node_ev")
        history = tuple(node["history_labels"])
        mass = sum(
            reach
            for villain, reach in opponent.items()
            if reach > 0 and not set(hero).intersection(villain)
        )
        if mass == 0:
            return 0.0
        reported = self.reported.get(history)
        value = reported[player].get(hero) if reported is not None else None
        if value is None:
            side = "reference" if self.reference else "project"
            raise MissingReportedValue(
                f"The {side} capture reports no value for player {player} holding "
                f"{' '.join(hero)} at {list(history)}, and the walk cannot cross this "
                "node to compute one"
            )
        return value * mass

    def walk(self, history, player, hero, opponent, maximize, sources):
        node = self.nodes[history]
        if node["kind"] == "terminal":
            return self.terminal(node, player, hero, opponent, sources)
        if node["kind"] == "chance":
            return self.leaf(node, player, hero, opponent, sources)
        actions = self.actions(node)
        if node["player"] == player:
            values = [
                self.walk(history + (a,), player, hero, opponent, maximize, sources)
                for a in actions
            ]
            if maximize:
                return max(values)
            row = self.rows[history][hero]
            return sum(p * v for p, v in zip(row, values, strict=True))
        total = 0.0
        rows = self.rows[history]
        for index, action in enumerate(actions):
            nxt = {
                villain: reach * rows[villain][index]
                for villain, reach in opponent.items()
            }
            total += self.walk(
                history + (action,), player, hero, nxt, maximize, sources
            )
        return total

    def path_weights(self, history):
        """Reconstruct inclusion/path weights; chance is a separate 1/44 factor.

        Project inclusion weights are divided by each board-filtered range's maximum.
        Reference products round to f32 at every operation, as its producer does.
        This uses raw exported policies, without the oracle walk's normalization.
        A positive f64 product that underflows is invalid in the project producer.
        """
        history = tuple(history)
        if history in self._path_cache:
            return self._path_cache[history]
        if not history:
            weights = [dict(side) for side in self.weights]
            for side in weights:
                scale = 1 if self.reference else max(side.values())
                for hand in side:
                    value = side[hand] / scale
                    side[hand] = self._f32(value) if self.reference else value
            result = (weights, 1.0)
        else:
            parent_history, action = history[:-1], history[-1]
            previous, chance = self.path_weights(parent_history)
            weights = [dict(side) for side in previous]
            parent = self.nodes[parent_history]
            if parent["kind"] == "chance":
                runout = action.split(":", 1)[1]
                for side in weights:
                    for hand in side:
                        if runout in hand:
                            side[hand] = 0.0
                # Four public and four private cards are already known to a deal.
                chance /= 52 - len(self.node_board(parent)) - 4
            else:
                player = parent["player"]
                index = self.actions(parent).index(action)
                for hand, reach in weights[player].items():
                    if reach == 0:
                        continue
                    probability = self.raw_rows[parent_history][hand][index]
                    product = reach * probability
                    if self.reference:
                        product = self._f32(product)
                    elif reach > 0 and probability > 0 and product == 0:
                        raise ValueError(
                            f"Positive project reach underflow at {history}"
                        )
                    weights[player][hand] = product
            result = (weights, chance)
        self._path_cache[history] = result
        return result

    def reference_reach_bounds(self, history, policy_rounding):
        """Enclose true f32 path reach despite decimal policy presentation.

        Below one, the pinned display rounds to six decimal places. Half that
        quantum brackets each displayed policy; f32 multiplication adds one ulp
        conservatively on either side. Zero is never inferred from display zero.
        """
        history = tuple(history)
        cache_key = ("bounds", history, policy_rounding)
        if cache_key in self._path_cache:
            return self._path_cache[cache_key]
        if not history:
            bounds = [
                {h: (self._f32(w), self._f32(w)) for h, w in side.items()}
                for side in self.weights
            ]
        else:
            prior = self.reference_reach_bounds(history[:-1], policy_rounding)
            bounds = [dict(side) for side in prior]
            parent = self.nodes[history[:-1]]
            if parent["kind"] == "chance":
                runout = history[-1].split(":", 1)[1]
                for side in bounds:
                    for hand in side:
                        if runout in hand:
                            side[hand] = (0.0, 0.0)
            else:
                player = parent["player"]
                index = self.actions(parent).index(history[-1])
                for hand, (low, high) in bounds[player].items():
                    if high == 0:
                        continue
                    p = self.raw_rows[history[:-1]][hand][index]
                    lo = low * max(0.0, p - policy_rounding)
                    hi = high * min(1.0, p + policy_rounding)
                    bounds[player][hand] = (
                        max(0.0, lo * (1 - 2**-23) - 2**-149),
                        hi * (1 + 2**-23) + (2**-149 if hi else 0.0),
                    )
        self._path_cache[cache_key] = bounds
        return bounds

    @staticmethod
    def _f32(value):
        return struct.unpack("<f", struct.pack("<f", value))[0]

    def compatible_hands(self, player, hero):
        cache_key = (player, hero)
        if cache_key not in self._compatible_cache:
            self._compatible_cache[cache_key] = tuple(
                hand
                for hand in self.hands[player ^ 1]
                if not set(hero).intersection(hand)
            )
        return self._compatible_cache[cache_key]

    def reach_evidence(self, history, player):
        """Own inclusion reach and directly summed compatible mass for live hands."""
        weights, chance = self.path_weights(history)
        node = self.nodes[tuple(history)]
        opponent = {hand: reach * chance for hand, reach in weights[player ^ 1].items()}
        return {
            hand: (
                weights[player][hand],
                math.fsum(
                    opponent[villain] for villain in self.compatible_hands(player, hand)
                ),
            )
            for hand in self.live_hands(player, node)
        }

    def action_values(self, history, hero):
        """Counterfactual own history; no EV when compatible opposing reach is zero."""
        history = tuple(history)
        hero = key(hero)
        node = self.nodes[history]
        if node["kind"] != "decision":
            raise ValueError("Action values are defined at decision nodes only")
        player = node["player"]
        opponent = dict(self.weights[player ^ 1])
        own = 1.0
        for depth, action in enumerate(history):
            parent = self.nodes[history[:depth]]
            if parent["kind"] == "chance":
                runout = action.split(":", 1)[1]
                if runout in hero:
                    raise ValueError(
                        "Hero holds the dealt runout card; the history is dead"
                    )
                opponent = {
                    villain: reach
                    for villain, reach in opponent.items()
                    if runout not in villain
                }
                continue
            index = self.actions(parent).index(action)
            if parent["player"] == player:
                own *= self.rows[history[:depth]][hero][index]
            else:
                rows = self.rows[history[:depth]]
                opponent = {
                    villain: reach * rows[villain][index]
                    for villain, reach in opponent.items()
                }
        mass = sum(
            reach
            for villain, reach in opponent.items()
            if not set(hero).intersection(villain)
        )
        sources: set[str] = set()
        values = None
        if mass > 0:
            values = [
                self.walk(history + (a,), player, hero, opponent, False, sources) / mass
                for a in self.actions(node)
            ]
        return {
            "counterfactual_action_ev": values,
            "own_history_reach": own,
            "compatible_opponent_mass": mass,
            "continuation_sources": sorted(sources),
        }

    def metrics(self):
        return {
            "policy_basis": "normalized_exported_policy",
            "scope": (
                "Per-row counterfactual values only. This oracle does not compute a turn "
                "exploitability; see the module docstring."
            ),
            "compatible_root_mass": self.mass,
            "decision_nodes": len(self.rows),
            "reported_value_leaves": len(self.reported),
            "private_hand_counts": [len(hands) for hands in self.hands],
            "max_probability_normalization_adjustment": self.max_normalization_adjustment,
        }
