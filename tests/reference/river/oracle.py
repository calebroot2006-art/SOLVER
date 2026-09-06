"""Independent scalar river evaluation over explicit physical private deals."""

from functools import lru_cache
from itertools import combinations
import math


RANKS = "23456789TJQKA"
SUITS = "cdhs"


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


@lru_cache(maxsize=8192)
def seven(cards):
    return max(five(hand) for hand in combinations(cards, 5))


def expand(groups, board):
    deck = [r + s for r in RANKS for s in SUITS]
    groups = set(groups.split(","))
    if not groups or any(
        len(g) != 2
        or any(r not in RANKS for r in g)
        or RANKS.index(g[0]) < RANKS.index(g[1])
        for g in groups
    ):
        raise ValueError("Scalar oracle supports only explicit unweighted rank classes")
    result = []
    for low, high in combinations(deck, 2):
        if high[0] + low[0] in groups and not set((low, high)).intersection(board):
            result.append(key((low, high)))
    return result


class Oracle:
    """One recursion per own hand; opponent hands are summed before maximizing."""

    def __init__(self, case, reference=False):
        self.input = case["input"]
        self.hands = [expand(r, self.input["board"]) for r in self.input["ranges"]]
        self.nodes = {tuple(n["history_labels"]): n for n in case["nodes"]}
        if len(self.nodes) != len(case["nodes"]):
            raise ValueError("Repeated scalar oracle history")
        self.rows = {}
        self.max_normalization_adjustment = 0.0
        for history, node in self.nodes.items():
            if node["kind"] != "decision":
                continue
            p = node["player"]
            if reference:
                count = len(case["private_cards"][p])
                rows = {
                    key(h["cards"]): [
                        node["strategy"][a * count + i]
                        for a in range(len(node["actions"]))
                    ]
                    for i, h in enumerate(case["private_cards"][p])
                }
            else:
                rows = {key(h["cards"]): h["strategy"] for h in node["hands"]}
                if len(rows) != len(node["hands"]):
                    raise ValueError("Repeated scalar oracle private hand")
            if set(rows) != set(self.hands[p]):
                raise ValueError("Scalar oracle physical hand set differs")
            self.rows[history] = {}
            for hand in self.hands[p]:
                row = rows[hand]
                if len(row) != len(node["actions"]) or any(
                    not math.isfinite(v) or not 0 <= v <= 1 for v in row
                ):
                    raise ValueError("Invalid reported probability row")
                total = sum(row)
                if total <= 0 or abs(total - 1) > 0.00002:
                    raise ValueError("Invalid reported row normalization")
                normalized = [v / total for v in row]
                self.max_normalization_adjustment = max(
                    self.max_normalization_adjustment,
                    *(abs(a - b) for a, b in zip(row, normalized, strict=True))
                )
                self.rows[history][hand] = normalized
        self.ranks = {
            h: seven(tuple(self.input["board"]) + h)
            for hands in self.hands
            for h in hands
        }
        self.mass = sum(
            not set(a).intersection(b) for a in self.hands[0] for b in self.hands[1]
        )
        if not self.mass:
            raise ValueError("Empty compatible private deals")

    @staticmethod
    def actions(node):
        return [a if isinstance(a, str) else a["label"] for a in node["actions"]]

    def terminal(self, node, player, hero, opponent):
        pot = self.input["starting_pot"]
        chips = node["contributions"]
        total = 0.0
        for villain, reach in zip(self.hands[1 - player], opponent, strict=True):
            if reach == 0 or set(hero).intersection(villain):
                continue
            if node["terminal"] == "fold":
                share = float(node["fold_winner"] == player)
            else:
                share = (
                    1.0
                    if self.ranks[hero] > self.ranks[villain]
                    else 0.5 if self.ranks[hero] == self.ranks[villain] else 0.0
                )
            total += reach * ((pot + sum(chips)) * share - pot / 2 - chips[player])
        return total

    def walk(self, history, player, hero, opponent, maximize):
        node = self.nodes[history]
        if node["kind"] == "terminal":
            return self.terminal(node, player, hero, opponent)
        actions = self.actions(node)
        if node["player"] == player:
            values = [
                self.walk(history + (a,), player, hero, opponent, maximize)
                for a in actions
            ]
            return (
                max(values)
                if maximize
                else sum(
                    p * v for p, v in zip(self.rows[history][hero], values, strict=True)
                )
            )
        total = 0.0
        for index, action in enumerate(actions):
            next_opponent = [
                reach * self.rows[history][villain][index]
                for villain, reach in zip(self.hands[1 - player], opponent, strict=True)
            ]
            total += self.walk(
                history + (action,), player, hero, next_opponent, maximize
            )
        return total

    def value(self, player, maximize=False):
        opponent = [1.0] * len(self.hands[1 - player])
        return (
            sum(
                self.walk((), player, hero, opponent, maximize)
                for hero in self.hands[player]
            )
            / self.mass
        )

    def metrics(self):
        ev = [self.value(p) for p in (0, 1)]
        br = [self.value(p, True) for p in (0, 1)]
        return {
            "policy_basis": "normalized_exported_policy",
            "expected_values": ev,
            "best_response_values": br,
            "pct_of_pot": 50 * max(0, sum(br)) / self.input["starting_pot"],
            "max_probability_normalization_adjustment": self.max_normalization_adjustment,
        }

    def action_values(self, history, hero):
        """Counterfactual own history; no EV when compatible opposing reach is zero."""
        history = tuple(history)
        hero = key(hero)
        player = self.nodes[history]["player"]
        opponent = [1.0] * len(self.hands[1 - player])
        own = 1.0
        for depth, action in enumerate(history):
            parent = history[:depth]
            node = self.nodes[parent]
            index = self.actions(node).index(action)
            if node["player"] == player:
                own *= self.rows[parent][hero][index]
            else:
                opponent = [
                    reach * self.rows[parent][villain][index]
                    for villain, reach in zip(
                        self.hands[1 - player], opponent, strict=True
                    )
                ]
        mass = sum(
            reach
            for villain, reach in zip(self.hands[1 - player], opponent, strict=True)
            if not set(hero).intersection(villain)
        )
        values = (
            [
                self.walk(history + (a,), player, hero, opponent, False) / mass
                for a in self.actions(self.nodes[history])
            ]
            if mass > 0
            else None
        )
        return {
            "counterfactual_action_ev": values,
            "own_history_reach": own,
            "compatible_opponent_mass": mass,
        }
