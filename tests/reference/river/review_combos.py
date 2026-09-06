"""Record independent action values and single-row effects for every policy mismatch."""

import argparse
import hashlib
import json
from pathlib import Path
import tomllib

from capture import MAX_OUTPUT_BYTES, read_json, require
from compare import compare
from oracle import Oracle, key


def evidence(oracle, history, cards, alternative):
    values = oracle.action_values(history, cards)
    policy = oracle.rows[tuple(history)][key(cards)]
    ev = values["counterfactual_action_ev"]
    reach = values["own_history_reach"]
    mass = values["compatible_opponent_mass"]
    probability = reach * mass / oracle.mass
    result = {
        **values,
        "strategy": policy,
        "root_information_set_probability": probability,
        "conditional_action_gaps": None,
        "conditional_best_action_gain": None,
        "root_best_action_gain_chips": 0.0,
        "root_alternative_row_gain_chips": 0.0,
    }
    if ev is not None:
        current = sum(p * v for p, v in zip(policy, ev, strict=True))
        gain = max(ev) - current
        result.update(
            conditional_action_gaps=[max(ev) - v for v in ev],
            conditional_best_action_gain=gain,
            root_best_action_gain_chips=probability * gain,
            root_alternative_row_gain_chips=probability
            * (sum(p * v for p, v in zip(alternative, ev, strict=True)) - current),
        )
    return result


def review(project, reference, initial_project, initial_reference):
    final = compare(project, reference)
    initial = compare(initial_project, initial_reference)
    reports = []
    for case, baseline, pc, rc, pi, ri in zip(
        final["cases"],
        initial["cases"],
        project["cases"],
        reference["cases"],
        initial_project["cases"],
        initial_reference["cases"],
        strict=True,
    ):
        require(
            all(c["input"] == pc["input"] for c in (rc, pi, ri)),
            "Review cases differ",
        )
        po, ro, pio, rio = Oracle(pc), Oracle(rc, True), Oracle(pi), Oracle(ri, True)
        rows = {
            (tuple(row["history"]), tuple(row["cards"])): row
            for comparison in (baseline, case)
            for row in comparison["differences"]
        }
        details = []
        for (history, cards), row in sorted(rows.items()):
            p, r = po.rows[history][cards], ro.rows[history][cards]
            pb, rb = pio.rows[history][cards], rio.rows[history][cards]
            delta = [abs(a - b) for a, b in zip(p, r, strict=True)]
            pe, re = evidence(po, history, cards, r), evidence(ro, history, cards, p)
            description = (
                "Frequency difference is within two percentage points after refinement."
                if max(delta) <= 0.02
                else (
                    "Reference own history reach is exactly zero; its row is outside its profile path. Project reach, counterfactual action gaps and single-row effects are recorded separately."
                    if re["own_history_reach"] == 0
                    else (
                        "Compatible opposing reach is zero in at least one profile; that profile has no defined conditional action EV here."
                        if pe["compatible_opponent_mass"] == 0
                        or re["compatible_opponent_mass"] == 0
                        else "Both profiles reach this information set. Their action gaps and changes from the initial capture require numerical review; low root residual alone does not explain the row."
                    )
                )
            )
            details.append(
                {
                    "history": list(history),
                    "cards": list(cards),
                    "player": row["player"],
                    "actions": row["actions"],
                    "initial_max_frequency_difference": max(
                        abs(a - b) for a, b in zip(pb, rb, strict=True)
                    ),
                    "final_frequency_differences": delta,
                    "project_initial": evidence(pio, history, cards, rb),
                    "reference_initial": evidence(rio, history, cards, pb),
                    "project_refined": pe,
                    "reference_refined": re,
                    "interpretation": description,
                }
            )
        reports.append(
            {
                "id": case["id"],
                "initial_iterations": [pi["iterations"], ri["iterations"]],
                "refined_iterations": [pc["iterations"], rc["iterations"]],
                "initial_metrics": [pio.metrics(), rio.metrics()],
                "refined_metrics": [po.metrics(), ro.metrics()],
                "reviewed_union_rows": len(details),
                "remaining_frequency_rows": sum(
                    max(d["final_frequency_differences"]) > 0.02 for d in details
                ),
                "rows": details,
            }
        )
    return {
        "schema_version": 1,
        "scope": "Every row differing by more than two percentage points in either initial or refined captures. Metrics evaluate normalized exported policies. Each row effect changes only that own information set, holding all other rows fixed; effects are not additive or bounds for other decisions.",
        "acceptance": "requires_independent_review",
        "cases": reports,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in (
        "project",
        "reference",
        "initial_project",
        "initial_reference",
        "output",
    ):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    inputs = {}
    hashes = {}
    for name in ("project", "reference", "initial_project", "initial_reference"):
        path = getattr(args, name)
        require(path.stat().st_size <= MAX_OUTPUT_BYTES, "Oversized review input")
        inputs[name] = (
            tomllib.loads(path.read_text(encoding="utf-8"))
            if "project" in name
            else read_json(path, MAX_OUTPUT_BYTES)
        )
        hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    report = review(**inputs)
    report["input_sha256"] = hashes
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")
    print(
        json.dumps(
            [
                {
                    k: v
                    for k, v in c.items()
                    if k not in ("rows", "initial_metrics", "refined_metrics")
                }
                for c in report["cases"]
            ],
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
