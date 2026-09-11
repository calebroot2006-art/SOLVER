"""Reproduce acceptance defects without modifying shipped tooling or thresholds."""
import copy
import json
from pathlib import Path
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / "tests/reference/turn"))
import _fixture
from compare import joint_report
from review_rule import load_rules
from review_combos import review
from oracle import Oracle

rules = load_rules()
reference = _fixture.reference_capture()
empty = {"schema_version": 2, "street": "turn", "rule": rules, "cases": []}
results = []


def gate(name, project, record=empty):
    report = joint_report(project, reference, record, "0" * 40, rules)
    summary = {"mutation": name, "accepted": report["accepted"], "stale_review_rows": len(report["stale_review_rows"]), "gate_failures": report["gate_failures"]}
    results.append(summary)
    return summary


gate("baseline", _fixture.project_capture(reference))
project = _fixture.project_capture(reference)
project["cases"][0]["root_centered_expected_values"] = [1000., -1000.]
gate("root_values", project)

for mutation in ("missing_ev", "zero_reach"):
    project = _fixture.project_capture(reference, root_check_frequency=0.40)
    for hand in project["cases"][0]["nodes"][0]["hands"]:
        if mutation == "missing_ev":
            hand["ev_available"] = False
            hand["action_expected_values"] = []
        else:
            hand["own_reach"] = 0.
    gate(mutation, project)

base = _fixture.project_capture(reference)
case = base["cases"][0]
hand = case["nodes"][0]["hands"][0]
hero = hand["cards"]
hand["strategy"] = [0.68, 0.32]
hand["action_expected_values"] = Oracle(case).action_values((), hero)["counterfactual_action_ev"]
record = review(base, reference, rules)
gate("oracle_baseline", base, record)["oracle_values"] = hand["action_expected_values"]

for mutation in ("future_policy", "chance_value"):
    project = copy.deepcopy(base)
    case = project["cases"][0]
    if mutation == "future_policy":
        node = next(n for n in case["nodes"] if n["history_labels"] == ["check"])
        for row in node["hands"]:
            row["strategy"] = [0.51, 0.49]
    else:
        node = next(n for n in case["nodes"] if n["history_labels"] == ["check", "check"])
        row = next(h for h in node["hands"] if h["player"] == 0 and h["cards"] == hero)
        row["expected_value"] += 100.
    gate(mutation, project, record)["current_oracle_values"] = Oracle(case).action_values((), hero)["counterfactual_action_ev"]

output = json.dumps(results, indent=2, allow_nan=False) + "\n"
print(output)
(ROOT / "docs/reviews/2026-09-10-step6-review/acceptance-mutation-results.json").write_text(output, encoding="utf-8")
# These passing assertions confirm that the known-bad inputs are still accepted.
assert all(result["accepted"] and not result["gate_failures"] and result["stale_review_rows"] == 0 for result in results)
