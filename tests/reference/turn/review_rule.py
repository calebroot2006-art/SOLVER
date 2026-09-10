"""The rule that decides what a turn policy difference means, and what it costs.

Two solves that both stop at 0.25% of pot disagree about the mix on 138,738 of the
233,567 compared rows. Writing 138,738 reasoning sentences is not review; it is a file.
So the reasoning is generated per row by rule, from the two captures, and the reviewer's
work is the rule and the rows it cannot excuse.

Each differing row falls into exactly one of three categories.

* **A, indifferent.** Adopting the other side's mix costs at most
  `indifference_pot_fraction` of the pot, measured on each side's own action EVs, on both
  sides. Two mixes that are worth the same to the player choosing between them are not a
  disagreement about the game; they are a disagreement about a coin.
* **B, unreached.** One of the two captures reports no EV here, or the row's reach is
  below `reach_floor`. A row nobody reaches contributes at most its reach times the
  largest gap on offer, and that bound is recorded rather than assumed to be small.
* **C, a real gap.** Anything else: the mixes differ and the difference is worth real
  chips at a history somebody reaches. Every C row is listed individually with its
  reach-weighted loss, and their sum is what the gate holds to a budget.

The thresholds live in `review_rules.json` beside this file, never in the code, and they
are a Decision 14 candidate that Caleb has not confirmed.

Two conventions worth stating, because both make the rule stricter rather than kinder:

* The switching cost is an absolute value. A row where adopting the other side's mix
  *gains* EV under your own action values is not indifferent either: it says your own
  average is that far from its own best response here, which is exactly the kind of row a
  reviewer should see.
* Reach is the root probability of this information set: the hand's own reach times the
  compatible opposing mass, over the case's compatible weight. It is the project's
  measurement of it. The reference reports a display reach on a different footing, so
  mixing the two would compare two different numbers.
"""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

RULES_FILENAME = "review_rules.json"
THRESHOLDS = (
    "indifference_pot_fraction",
    "reach_floor",
    "real_gap_budget_pot_fraction",
)
CATEGORIES = ("indifferent", "unreached", "real_gap")
# Why a row landed where it did. The two unreached reasons are worth counting apart:
# one is a capture that reports nothing, the other is a history nobody visits.
REASONS = ("indifferent", "ev_absent", "below_reach_floor", "real_gap")


class RuleError(ValueError):
    """The rule cannot be applied to what it was given."""


def default_rules_path():
    return Path(__file__).resolve().parent / RULES_FILENAME


def load_rules(path=None):
    """Read and check the thresholds, and hash the file they came from.

    The hash goes into the committed review so a record cannot outlive the rule that
    produced it: change a threshold and the gate says the record is stale rather than
    quietly judging old rows by new numbers.
    """
    path = Path(path) if path is not None else default_rules_path()
    raw = path.read_bytes()
    if len(raw) > 64 * 1024:
        raise RuleError(f"{path} is too large to be a rules file")
    try:
        parsed = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuleError(f"{path} is not readable JSON: {error}") from error
    if type(parsed) is not dict or parsed.get("schema_version") != 1:
        raise RuleError(f"{path} is not a version 1 rules file")
    thresholds = {}
    for name in THRESHOLDS:
        value = parsed.get(name)
        if type(value) not in (int, float) or type(value) is bool:
            raise RuleError(f"{path} does not give a number for {name}")
        value = float(value)
        if not math.isfinite(value) or not 0 < value <= 1:
            raise RuleError(f"{name} must be a fraction above zero, not {value}")
        thresholds[name] = value
    unknown = set(parsed) - set(THRESHOLDS) - {"schema_version", "status"}
    if unknown:
        raise RuleError(f"{path} carries fields the rule does not read: {sorted(unknown)}")
    return {
        "source": path.name,
        "sha256": hashlib.sha256(raw).hexdigest(),
        "status": parsed.get("status", ""),
        **thresholds,
    }


def same_rule(recorded, current):
    """Whether a committed record was generated under exactly the current rule."""
    if type(recorded) is not dict:
        return False
    if recorded.get("sha256") != current["sha256"]:
        return False
    return all(recorded.get(name) == current[name] for name in THRESHOLDS)


def mix_value(strategy, action_ev):
    return sum(p * v for p, v in zip(strategy, action_ev, strict=True))


def switch_loss(own_strategy, other_strategy, own_action_ev):
    """What adopting the other side's mix is worth here, on this side's own EVs.

    Absolute: a switch that gains is as much a disagreement as one that loses.
    """
    if own_action_ev is None:
        return None
    return abs(
        mix_value(own_strategy, own_action_ev)
        - mix_value(other_strategy, own_action_ev)
    )


def action_gap(action_ev):
    """The most any single action here can be worth over the worst one."""
    if not action_ev:
        return None
    return max(action_ev) - min(action_ev)


def row_reach(row, compatible_weight):
    """Root probability of this information set, from the project's own measurement.

    The hand's reach on its own path times the compatible opposing mass, over the
    case's total compatible weight: the chance that this hand, some compatible
    opposing hand, and this history all happen.
    """
    if not compatible_weight > 0:
        raise RuleError("A case reports no compatible weight to normalize reach by")
    reach = row["project_own_reach"] * row["project_opponent_mass"] / compatible_weight
    if not math.isfinite(reach) or reach < 0:
        raise RuleError(f"Unusable reach at {row['history']} {row['cards']}")
    return reach


def classify(row, pot, compatible_weight, rules):
    """One row's category, its reasoning, and the chips behind both.

    The row is a `compare.py` difference entry, so every number here was measured by
    one of the two captures; nothing is recomputed from a policy.
    """
    if not pot > 0:
        raise RuleError("A case reports no starting pot")
    reach = row_reach(row, compatible_weight)
    project_ev = row["project_action_ev"]
    reference_ev = row["reference_action_ev"]
    project_loss = switch_loss(
        row["project_strategy"], row["reference_strategy"], project_ev
    )
    reference_loss = switch_loss(
        row["reference_strategy"], row["project_strategy"], reference_ev
    )
    gaps = [gap for gap in (action_gap(project_ev), action_gap(reference_ev)) if gap]
    max_gap = max(gaps) if gaps else None
    verdict = {
        "reach": reach,
        "project_switch_loss_chips": project_loss,
        "reference_switch_loss_chips": reference_loss,
        "max_switch_loss_chips": None,
        "max_action_gap_chips": max_gap,
        "bound_chips": None,
        "reach_weighted_loss_chips": None,
    }
    indifference = rules["indifference_pot_fraction"] * pot
    if project_loss is None or reference_loss is None:
        absent = [
            side
            for side, loss in (("project", project_loss), ("reference", reference_loss))
            if loss is None
        ]
        missing = " and the ".join(absent)
        verdict["bound_chips"] = None if max_gap is None else reach * max_gap
        bound = (
            "no action EV at all, so the row bounds nothing"
            if max_gap is None
            else f"at most {verdict['bound_chips']:.3e} chips, its reach {reach:.3e} "
            f"times the largest gap on offer, {max_gap:.4f} chips"
        )
        return verdict | {
            "category": "unreached",
            "reason": "ev_absent",
            "review_reasoning": (
                f"The {missing} capture reports no action EV here, which a capture does "
                f"where its own reach underflows. The row is worth {bound}."
            ),
        }
    loss = max(project_loss, reference_loss)
    verdict["max_switch_loss_chips"] = loss
    if loss <= indifference:
        return verdict | {
            "category": "indifferent",
            "reason": "indifferent",
            "review_reasoning": (
                f"Adopting the other mix costs {project_loss:.5f} chips on the project's "
                f"own action EVs and {reference_loss:.5f} on the reference's, both within "
                f"{indifference:.5f} chips, {100 * rules['indifference_pot_fraction']:.4g}% "
                "of the pot. The two mixes are worth the same to the player choosing "
                "between them."
            ),
        }
    if reach < rules["reach_floor"]:
        verdict["bound_chips"] = reach * loss
        return verdict | {
            "category": "unreached",
            "reason": "below_reach_floor",
            "review_reasoning": (
                f"Reach {reach:.3e} is below the floor {rules['reach_floor']:.0e}, so the "
                f"row is worth at most {verdict['bound_chips']:.3e} chips: its reach times "
                f"the {loss:.5f} chips the switch costs."
            ),
        }
    verdict["reach_weighted_loss_chips"] = reach * loss
    return verdict | {
        "category": "real_gap",
        "reason": "real_gap",
        "review_reasoning": (
            f"Switching mixes costs {loss:.5f} chips, above the "
            f"{indifference:.5f} the rule allows, at a history reached with probability "
            f"{reach:.3e}: {verdict['reach_weighted_loss_chips']:.3e} chips of the pot's "
            "expected value, counted against the gate's budget."
        ),
    }


def empty_counts():
    return (
        {name: 0 for name in CATEGORIES}
        | {f"reason_{name}": 0 for name in REASONS}
        | {"rows": 0, "unbounded_unreached_rows": 0}
    )


def summarize(case_id, pot, verdicts, rules):
    """Per-case counts, the C total, and whether it is inside the budget."""
    counts = empty_counts()
    real_gap = 0.0
    unreached_bound = 0.0
    for verdict in verdicts:
        counts["rows"] += 1
        counts[verdict["category"]] += 1
        counts[f"reason_{verdict['reason']}"] += 1
        if verdict["category"] == "real_gap":
            real_gap += verdict["reach_weighted_loss_chips"]
        elif verdict["category"] == "unreached":
            if verdict["bound_chips"] is None:
                counts["unbounded_unreached_rows"] += 1
            else:
                unreached_bound += verdict["bound_chips"]
    budget = rules["real_gap_budget_pot_fraction"] * pot
    return {
        "id": case_id,
        "starting_pot": pot,
        "counts": counts,
        "real_gap_reach_weighted_loss_chips": real_gap,
        "real_gap_pot_fraction": real_gap / pot,
        "real_gap_budget_chips": budget,
        "within_budget": real_gap <= budget,
        # Not a gate condition: the excused rows' own bound, reported so that the
        # size of what B waves through is on the record rather than assumed.
        "unreached_bound_chips": unreached_bound,
        "unreached_bound_pot_fraction": unreached_bound / pot,
    }
