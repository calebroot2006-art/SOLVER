"""Independent review arithmetic; no project solver or model client is exercised."""

from fractions import Fraction as F
from itertools import permutations, product
import json


def kuhn_value(cards, policies, history=""):
    winner = 1 if cards[0] > cards[1] else -1
    terminal = {"cc": winner, "bc": 2 * winner, "cbc": 2 * winner,
                "bf": 1, "cbf": -1}
    if history in terminal:
        return F(terminal[history])
    player = 0 if history in ("", "cb") else 1
    second = history in ("cb", "b")
    actions = ("f", "c") if history in ("b", "cb") else ("c", "b")
    policy = policies[player]
    probs = (F(1, 2), F(1, 2)) if policy is None else (
        (F(1), F(0)) if policy[2 * cards[player] + int(second)] == 0
        else (F(0), F(1))
    )
    return sum(p * kuhn_value(cards, policies, history + action)
               for p, action in zip(probs, actions))


def value(policies):
    return sum(kuhn_value(cards, policies) for cards in permutations(range(3), 2)) / 6


def run():
    policies = list(product((0, 1), repeat=6))
    br0 = max(value((policy, None)) for policy in policies)
    br1 = max(-value((None, policy)) for policy in policies)
    nash_conv = br0 + br1
    average = nash_conv / 2
    pct = 100 * average / 2  # Two one-chip antes form the starting pot.
    assert (br0, br1, nash_conv) == (F(1, 2), F(5, 12), F(11, 12))
    assert average == F(11, 24) and pct == F(275, 12)

    # Each physical Leduc private-card pair permits four of six board cards.
    chance_masses = [sum(F(1, 4) * int(board != a) * int(board != b)
                        for board in range(6))
                     for a, b in permutations(range(6), 2)]
    assert len(chance_masses) == 30 and set(chance_masses) == {F(1)}

    accumulated = [F(0), F(0)]
    contribution_only = [F(0), F(0)]
    for t, strategy in enumerate(((1, 0), (0, 1), (0, 1)), 1):
        discount = F(t, t + 1) ** 2
        accumulated = [(old + new) * discount
                       for old, new in zip(accumulated, strategy)]
        contribution_only = [old + new * discount
                             for old, new in zip(contribution_only, strategy)]
    correct = accumulated[0] / sum(accumulated)
    wrong = contribution_only[0] / sum(contribution_only)
    assert correct == F(1, 14) and correct != wrong

    # Synthetic minimum validator from the proposal, not application code.
    facts = {"bet": {"ev": 0.4, "frequency": 0.62},
             "check": {"ev": 0.4, "frequency": 0.38}}
    response = {"explanation": "Betting is mandatory; checking always loses.",
                "cited_action": "bet", "cited_ev": 0.4,
                "cited_frequency": 0.62}
    assert isinstance(response["explanation"], str)
    quoted = facts[response["cited_action"]]
    accepted_by_number_match = (response["cited_ev"] == quoted["ev"]
                               and response["cited_frequency"] == quoted["frequency"])
    assert accepted_by_number_match
    assert facts["bet"]["ev"] - facts["check"]["ev"] == 0

    return {
        "scope": "Exact arithmetic and a synthetic coach counterexample only; no product code",
        "kuhn_uniform": {"profile_ev_p0": str(value((None, None))),
                         "br0": str(br0), "br1": str(br1),
                         "nash_conv": str(nash_conv), "average": str(average),
                         "starting_pot_chips": 2, "pct_of_pot": str(pct)},
        "leduc_chance": {"legal_private_pairs": 30, "mass_for_every_pair": "1"},
        "dcfr_averaging_example": {"full_accumulator": str(correct),
                                   "new_contribution_only": str(wrong)},
        "coach_counterexample": {"facts": facts, "response": response,
                                 "passes_cited_number_match": accepted_by_number_match,
                                 "actual_ev_loss_for_check": 0},
    }


if __name__ == "__main__":
    print(json.dumps(run(), indent=2))
