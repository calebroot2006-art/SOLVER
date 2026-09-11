"""Audit CI 34557551003 captures with the current gate, from the checkout root."""

import gc
import hashlib
import json
import sys
import time
from pathlib import Path

import tomllib

sys.path.insert(0, str(Path("tests/reference/turn")))
from compare import joint_report
from review_rule import load_rules


def read(path):
    raw = path.read_bytes()
    value = tomllib.loads(raw.decode()) if path.suffix == ".toml" else json.loads(raw)
    return value, hashlib.sha256(raw).hexdigest()


def main():
    folder = Path("target/correction-evidence")
    reference, reference_sha = read(folder / "turn-reference/cases.json")
    record, record_sha = read(Path("tests/reference/turn/per-combo-review.json"))
    rules = load_rules()
    sources = {
        p.as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in Path("tests/reference/turn").glob("*.py")
    }
    for platform in ["linux", "windows"]:
        start = time.perf_counter()
        project, project_sha = read(folder / f"turn-{platform}/cases.toml")
        parsed = time.perf_counter()
        result = joint_report(
            project,
            reference,
            record,
            "69dc3fffa2ea9507b4146fa640d2c12415d058a6",
            rules,
        )
        rows = [
            row
            for case in result["cases"]
            for row in case["differences"]
            if row.get("category") == "real_gap"
        ]
        summary = {
            "platform": platform,
            "ci_run": 34557551003,
            "project_sha256": project_sha,
            "reference_sha256": reference_sha,
            "record_sha256": record_sha,
            "accepted": result["accepted"],
            "failures": result["gate_failures"],
            "missing": len(result["rows_missing_review"]),
            "stale": len(result["stale_review_rows"]),
            "parse_seconds": parsed - start,
            "gate_seconds": time.perf_counter() - parsed,
            "current_oracle_rows": len(rows),
            "current_oracle_max_abs_difference": max(
                abs(a - b)
                for row in rows
                for a, b in zip(
                    row["current_oracle_action_ev"],
                    row["project_action_ev"],
                    strict=True,
                )
            ),
            "called_all_in_oracle": result["called_all_in_oracle"],
            "counts": [case["counts"] for case in result["review"]["cases"]],
            "source_sha256": sources,
        }
        Path(
            f"docs/astra/step6-corrections/correction-{platform}-gate-summary.json"
        ).write_text(json.dumps(summary, indent=2, allow_nan=False) + "\n")
        print(
            json.dumps(
                {
                    k: v
                    for k, v in summary.items()
                    if k not in {"source_sha256", "called_all_in_oracle", "counts"}
                }
            ),
            flush=True,
        )
        assert summary["accepted"] and not summary["failures"]
        assert summary["missing"] == summary["stale"] == 0
        assert summary["current_oracle_rows"] == 817
        del rows, result, project
        gc.collect()


if __name__ == "__main__":
    main()
