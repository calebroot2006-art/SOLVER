"""The project's DCFR reference, defined once.

OpenSpiel's scalar `CFRSolver` traversal plus whole-accumulator discounting after
each alternating iteration. `capture.py`, `sensitivity.py`, and `verify_snapshots.py`
all import this module, so every reference applies the same arithmetic.
`test_dcfr_reference.py` checks it against OpenSpiel's own `DCFRSolver`, which
discounts each player's regrets right after that player's update and weights the
average by `t**gamma`. The definitions agree in exact arithmetic; independently
accumulated floating-point trajectories can diverge after many iterations.
"""

import hashlib
from pathlib import Path

from open_spiel.python.algorithms import cfr

ALPHA = 1.5
BETA = 0.0
GAMMA = 2.0


def source_sha256():
    """Identify the actual local implementation used by a capture or verifier."""
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def make_solver(game, variant):
    """The unmodified OpenSpiel solver for a variant. DCFR starts from CFRSolver."""
    if variant == "cfr_plus":
        return cfr.CFRPlusSolver(game)
    if variant in ("cfr", "dcfr"):
        return cfr.CFRSolver(game)
    raise ValueError(f"unknown variant {variant!r}")


def apply_dcfr_discount(solver, iteration):
    """Discount every accumulator after one-based `iteration` has completed.

    Positive regrets scale by t^alpha / (t^alpha + 1), negative regrets by
    t^beta / (t^beta + 1) (one half for beta = 0), and the whole cumulative
    strategy by (t / (t + 1))^gamma. Zero regrets are unchanged either way.
    """
    if isinstance(iteration, bool) or not isinstance(iteration, int) or iteration < 1:
        raise ValueError("iteration must be a positive integer")
    positive = iteration**ALPHA / (iteration**ALPHA + 1)
    negative = iteration**BETA / (iteration**BETA + 1)
    strategy = (iteration / (iteration + 1)) ** GAMMA
    for node in solver._info_state_nodes.values():
        for action, regret in node.cumulative_regret.items():
            node.cumulative_regret[action] *= positive if regret > 0 else negative
        for action in node.cumulative_policy:
            node.cumulative_policy[action] *= strategy
