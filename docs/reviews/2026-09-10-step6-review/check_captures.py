"""Read-only audit of downloaded captures. Run from the review checkout root."""
from collections import Counter
import copy
import gc
import hashlib
import json
from pathlib import Path
import sys
import tomllib

ROOT = Path.cwd()
EVIDENCE = ROOT / "target/review-evidence"
sys.path.insert(0, str(ROOT / "tests/reference/turn"))
from compare import joint_report
from review_rule import load_rules


def read(path):
    raw = path.read_bytes()
    value = tomllib.loads(raw.decode()) if path.suffix == ".toml" else json.loads(raw)
    return value, hashlib.sha256(raw).hexdigest()


def ignored(path):
    return path == ("project_revision",) or path == ("progress_interval_seconds",) or (
        len(path) >= 3 and path[0] == "cases" and isinstance(path[1], int)
        and path[2] in {"timings", "elapsed_seconds", "working_set_bound_bytes", "reserved_bytes"}
    ) or (
        len(path) == 5 and path[0] == "cases" and isinstance(path[1], int)
        and path[2] == "checkpoints" and isinstance(path[3], int) and path[4] == "elapsed_seconds"
    )


def compare_values(a, b):
    def leaves(value):
        if isinstance(value, dict):
            return sum(leaves(v) for v in value.values())
        if isinstance(value, list):
            return sum(leaves(v) for v in value)
        return 1
    counts = {"before_leaves": leaves(a), "after_leaves": leaves(b), "leaves": 0, "identical": 0, "allowed_differences": 0, "unexpected_differences": 0}
    differences = []
    def visit(x, y, path=()):
        if type(x) is not type(y):
            raise AssertionError(("type mismatch", path, type(x).__name__, type(y).__name__))
        if isinstance(x, dict):
            if x.keys() != y.keys():
                if not ignored(path):
                    raise AssertionError(("key mismatch", path, x.keys() ^ y.keys()))
                counts["allowed_differences"] += 1
                differences.append({"path": list(path), "removed_keys": sorted(x.keys() - y.keys()), "added_keys": sorted(y.keys() - x.keys()), "allowed": True})
            for key in x:
                if key in y:
                    visit(x[key], y[key], path + (key,))
        elif isinstance(x, list):
            if len(x) != len(y):
                raise AssertionError(("length mismatch", path, len(x), len(y)))
            for i, (left, right) in enumerate(zip(x, y)):
                visit(left, right, path + (i,))
        else:
            counts["leaves"] += 1
            if x == y:
                counts["identical"] += 1
            else:
                allowed = ignored(path)
                counts["allowed_differences" if allowed else "unexpected_differences"] += 1
                differences.append({"path": list(path), "before": x, "after": y, "allowed": allowed})
    visit(a, b)
    return counts | {"differences": differences}


def write_results(output):
    (EVIDENCE / "astra-full-capture-check-results.json").write_text(json.dumps(output, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    compact = copy.deepcopy(output)
    comparisons = [case["comparison"] for case in compact["turn"].values()]
    comparisons += [case["comparison"] for captures in compact["river"].values() for case in captures.values()]
    for comparison in comparisons:
        differences = comparison.pop("differences")
        comparison["difference_counts_by_field"] = dict(Counter(".".join("*" if isinstance(part, int) else part for part in entry["path"]) for entry in differences))
        comparison["non_timing_differences"] = [entry for entry in differences if not entry["allowed"] or entry["path"][-1] not in {"elapsed_seconds"} and "timings" not in entry["path"]]
        comparison["timing_schema_changes"] = [entry for entry in differences if "added_keys" in entry]
    destination = ROOT / "docs/reviews/2026-09-10-step6-review/capture-check-results.json"
    destination.write_text(json.dumps(compact, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def main():
    reference, reference_sha = read(EVIDENCE / "step6-turn-reference/cases.json")
    record, record_sha = read(ROOT / "tests/reference/turn/per-combo-review.json")
    rules = load_rules()
    output = {"reference_sha256": reference_sha, "review_sha256": record_sha, "turn": {}, "river": {}}
    for os_name in ("linux", "windows"):
        before, before_sha = read(EVIDENCE / f"step5b-turn-{os_name}/cases.toml")
        after, after_sha = read(EVIDENCE / f"step6-turn-{os_name}/cases.toml")
        comparison = compare_values(before, after)
        del before
        gc.collect()
        report = joint_report(after, reference, record, "e338d7a853f10bf819bf31728e23ac4f1fac802b", rules)
        gate = {k: v for k, v in report.items() if k not in {"cases", "review", "rows_missing_review", "stale_review_rows"}}
        gate["rows_missing_review"] = len(report["rows_missing_review"])
        gate["stale_review_rows"] = len(report["stale_review_rows"])
        gate["cases"] = [{k: v for k, v in case.items() if k != "differences"} for case in report["cases"]]
        gate["review"] = {"rule": report["review"]["rule"], "cases": [{k: v for k, v in case.items() if k != "rows"} for case in report["review"]["cases"]]}
        output["turn"][os_name] = {"before_sha256": before_sha, "after_sha256": after_sha, "comparison": comparison, "existing_gate": gate}
        print(json.dumps({"turn": os_name, **{k: v for k, v in comparison.items() if k != "differences"}, "gate_accepted": gate["accepted"], "stale_rows": gate["stale_review_rows"]}), flush=True)
        del after, report
        gc.collect()
    for os_name in ("linux", "windows"):
        output["river"][os_name] = {}
        for name in ("cases", "refined-cases"):
            before, before_sha = read(ROOT / f"tests/reference/river/measured/2930550/project-{os_name}-{name}.toml")
            after, after_sha = read(EVIDENCE / f"step6-river-{os_name}/{name}.toml")
            comparison = compare_values(before, after)
            output["river"][os_name][name] = {"before_sha256": before_sha, "after_sha256": after_sha, "comparison": comparison}
            print(json.dumps({"river": os_name, "capture": name, **{k: v for k, v in comparison.items() if k != "differences"}}), flush=True)
    write_results(output)
    assert all(v["comparison"]["unexpected_differences"] == 0 and v["existing_gate"]["accepted"] for v in output["turn"].values())
    assert all(v["comparison"]["unexpected_differences"] == 0 for os_capture in output["river"].values() for v in os_capture.values())


if __name__ == "__main__":
    main()
