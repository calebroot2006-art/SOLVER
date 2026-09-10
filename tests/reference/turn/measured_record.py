"""Write the small measured record for one turn gate run.

The captures themselves are 65 MB each and the comparison reports 18 MB, so what goes
into `measured/<sha>/` is the reading rather than the instrument: convergence per case
and operating system, the root expected values, the review's counts and its real-gap
rows, and the artifact and run identifiers that say where the raw files can still be
fetched from while the run's retention lasts.

    python tests/reference/turn/measured_record.py \\
      --run 34474380677 --commit <40 hex> \\
      --project ubuntu-latest=target/turn-project-ubuntu-latest/cases.toml \\
      --project windows-latest=target/turn-project-windows-latest/cases.toml \\
      --reference target/turn-reference/cases.json \\
      --report ubuntu-latest=target/turn-comparison/ubuntu-latest.json \\
      --report windows-latest=target/turn-comparison/windows-latest.json \\
      --rss ubuntu-latest=target/turn-project-ubuntu-latest/linux-time.txt \\
      --rss windows-latest=target/turn-project-windows-latest/windows-peak-rss.txt \\
      --artifacts artifacts.json --jobs jobs.json \\
      --output tests/reference/turn/measured/<short sha>

`artifacts.json` and `jobs.json` are whatever
`python docs/astra/development-takeover/ci_status.py artifacts <run>` and `jobs <run>`
printed. Both are optional; without them the record simply carries no artifact ids.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tomllib
from pathlib import Path

from capture import sha256

SCHEMA_VERSION = 1
CASE_FIELDS = (
    "iterations",
    "stop_reason",
    "reached_target",
    "exploitability_pct_of_pot",
    "exploitability_chips",
    "root_centered_expected_values",
    "best_response_values",
    "compatible_weight",
    "working_set_bound_bytes",
    "reserved_bytes",
    "elapsed_seconds",
    "exported_nodes",
    "workers",
)
TIMING_FIELDS = (
    "mean_iteration_seconds",
    "best_response_measurement_seconds",
    "average_strategy_snapshot_seconds",
    "cancel_latency_seconds",
    "cancel_latency_excluding_measurement_seconds",
)
ROW_FIELDS = (
    "history",
    "cards",
    "player",
    "street",
    "runout",
    "reach",
    "max_switch_loss_chips",
    "reach_weighted_loss_chips",
    "oracle_max_abs_difference_chips",
)


def pairs(values):
    """`name=path` arguments, in order."""
    result = {}
    for value in values or ():
        name, _, path = value.partition("=")
        if not name or not path:
            raise SystemExit(f"Expected name=path, got {value!r}")
        result[name] = Path(path)
    return result


def digest(path):
    """The hash and size of a small input, through the capture's own helper."""
    return {"sha256": sha256(path), "bytes": path.stat().st_size}


def read_once(path):
    """Text and digest of a capture from a single read. These files are 65 MB."""
    raw = path.read_bytes()
    return (
        raw.decode("utf-8"),
        {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)},
    )


def peak_rss_bytes(path):
    """Either `/usr/bin/time -v` output or the Windows poll's single number."""
    text = path.read_text(encoding="utf-8", errors="replace")
    linux = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", text)
    if linux:
        return int(linux.group(1)) * 1024
    windows = re.search(r"peak_working_set_bytes\s*=\s*(\d+)", text)
    if windows:
        return int(windows.group(1))
    raise SystemExit(f"{path} carries no peak resident set size")


def project_side(path, rss):
    text, capture_digest = read_once(path)
    capture = tomllib.loads(text)
    return {
        "capture": capture_digest,
        "project_revision": capture.get("project_revision"),
        "resolved_workers": capture.get("resolved_workers"),
        "progress_interval_seconds": capture.get("progress_interval_seconds"),
        "host": capture.get("host"),
        "peak_resident_set_bytes": None if rss is None else peak_rss_bytes(rss),
        "cases": [
            {"id": case["input"]["id"]}
            | {name: case[name] for name in CASE_FIELDS}
            | {"timings": {name: case["timings"][name] for name in TIMING_FIELDS}}
            for case in capture["cases"]
        ],
    }


def reference_side(path):
    text, capture_digest = read_once(path)
    capture = json.loads(text)
    return {
        "capture": capture_digest,
        "provenance": capture.get("provenance"),
        "cases": [
            {
                "id": case["input"]["id"],
                "iterations": case["iterations"],
                "stop_reason": case["stop_reason"],
                "exploitability_pct_of_pot": case["exploitability_pct_of_pot"],
                "reference_memory_estimate_bytes": case.get(
                    "reference_memory_estimate_bytes"
                ),
            }
            for case in capture["cases"]
        ],
    }


def comparison(path):
    report = json.loads(path.read_text(encoding="utf-8"))
    return {
        "accepted": report["accepted"],
        "gate_failures": report["gate_failures"],
        "rows_missing_review": len(report["rows_missing_review"]),
        "stale_review_rows": len(report["stale_review_rows"]),
        "tree_reconciliation": report["tree_reconciliation"][
            "project_wager_labels_restated"
        ],
        "cases": [
            {
                "id": case["id"],
                "matched_nodes": case["matched_nodes"],
                "matched_policy_rows": case["matched_policy_rows"],
                "frequency_rows_requiring_review": case[
                    "frequency_rows_requiring_review"
                ],
                "max_frequency_difference": case["max_frequency_difference"],
                "root_ev_absolute_difference": case["root_ev_absolute_difference"],
                "called_all_in_run_outs": case["called_all_in_run_outs"],
            }
            for case in report["cases"]
        ],
        "review": {
            "rule": report["review"]["rule"],
            "cases": [
                {key: case[key] for key in case if key != "rows"}
                for case in report["review"]["cases"]
            ],
        },
    }


def real_gap_rows(path):
    """The C rows, trimmed to what identifies them and what they cost."""
    review = json.loads(path.read_text(encoding="utf-8"))
    return {
        "record": digest(path),
        "rule": review["rule"],
        # Every summary field the review carries, so an aggregate added to the rule
        # reaches the record without an edit here, and the rows trimmed to what
        # identifies them, what they cost, and how far the recomputation sat from the
        # capture.
        "cases": [
            {key: value for key, value in case.items() if key != "rows"}
            | {
                "rows": [
                    {name: row[name] for name in ROW_FIELDS} for row in case["rows"]
                ]
            }
            for case in review["cases"]
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=int, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--project", action="append", required=True)
    parser.add_argument("--report", action="append", required=True)
    parser.add_argument("--rss", action="append")
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument(
        "--review",
        type=Path,
        default=Path(__file__).with_name("per-combo-review.json"),
    )
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--jobs", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    projects = pairs(args.project)
    reports = pairs(args.report)
    rss = pairs(args.rss)
    record = {
        "schema_version": SCHEMA_VERSION,
        "street": "turn",
        "source_commit": args.commit,
        "workflow_run": args.run,
        "generated_by": "tests/reference/turn/measured_record.py",
        "project": {
            name: project_side(path, rss.get(name)) for name, path in projects.items()
        },
        "reference": reference_side(args.reference),
        "comparison": {name: comparison(path) for name, path in reports.items()},
        "review": real_gap_rows(args.review),
    }
    if args.artifacts is not None:
        listing = json.loads(args.artifacts.read_text(encoding="utf-8"))
        record["artifacts"] = [
            {key: item[key] for key in ("id", "name", "size_in_bytes", "expires_at")}
            for item in listing["artifacts"]
            if "turn" in item["name"]
        ]
    if args.jobs is not None:
        jobs = json.loads(args.jobs.read_text(encoding="utf-8"))
        record["jobs"] = [
            {key: job[key] for key in ("id", "name", "conclusion")} for job in jobs
        ]
    args.output.mkdir(parents=True, exist_ok=True)
    target = args.output / "record.json"
    with target.open("w", encoding="utf-8") as stream:
        json.dump(record, stream, indent=2, allow_nan=False)
        stream.write("\n")
    print(f"Wrote {target} ({target.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
