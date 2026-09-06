"""Verify paired 20,000-iteration WASM captures; print a factual JSON report.

Usage: python verify_presentation.py --default DEFAULT.json --raw RAW.json
The display rules are independently expressed from the pinned wrapper contract.
No solver code, external dependencies, or reference outputs are included here.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import re
import struct

import capture

require = capture.require
DISPLAY_CUTOFF = struct.unpack("f", struct.pack("f", 0.0005))[0]
DISPLAY_FIELDS = {
    "reach_display_cutoff",
    "values_rounded_by_upstream",
    "values_below_1_decimal_places",
}
TREE_FIELDS = (
    "history",
    "history_labels",
    "kind",
    "player",
    "actions",
    "terminal",
    "fold_winner",
    "contributions",
    "reported_contributions",
)
SHARED_PROVENANCE = (
    "wasm_postflop_revision",
    "engine_revision",
    "toolchain",
    "rustc_verbose",
    "wasm_bindgen_cli",
    "python",
    "capture_python_sha256",
    "capture_javascript_sha256",
    "input_file_sha256",
    "resolved_reference_lock_sha256",
    "resolved_dependencies",
    "wasm_bindings_sha256",
    "upstream_manifest_hashes",
    "adjusted_manifest_hashes",
    "execution",
    "hosted_website_build_reproduction",
    "wrapper_source_path",
)
COUNT_FIELDS = (
    "nodes",
    "checkpoints",
    "strategy_cells",
    "reach_cells",
    "normalized_cells",
    "equity_cells",
    "ev_cells",
    "action_ev_cells",
    "additional_raw_ev_cells",
    "additional_raw_action_ev_cells",
)


def finite(value: object) -> float:
    require(
        type(value) in (int, float) and math.isfinite(value), "Nonfinite numeric cell"
    )
    return value


def display_round(value: float) -> float:
    """Signed threshold bands, with Rust's nearest/ties-away f64 rounding."""
    finite(value)
    band = sum(value >= 10**exponent for exponent in range(5))
    scale = 10 ** (6 - band)
    magnitude = abs(value * scale)
    require(math.isfinite(magnitude), "Display rounding overflow")
    integral = math.floor(magnitude)
    rounded = integral + (magnitude - integral >= 0.5)
    return math.copysign(rounded / scale, value)


def equal(actual: object, expected: object, label: str) -> None:
    require(actual == expected, f"Presentation mismatch: {label}")


def verify_provenance(default: dict, raw: dict) -> None:
    for data, is_raw in ((default, False), (raw, True)):
        equal(
            data.get("presentation_mode"),
            "raw_f32" if is_raw else "upstream_display",
            "mode",
        )
        capture.validate_presentation(data, is_raw)
        provenance = data.get("provenance", {})
        expected = {
            "wasm_postflop_revision": capture.WASM_REVISION,
            "engine_revision": capture.ENGINE_REVISION,
            "toolchain": capture.TOOLCHAIN,
            "wasm_bindgen_cli": "wasm-bindgen " + capture.BINDGEN_VERSION,
            "wrapper_source_path": "rust/solver-src/lib.rs",
            "original_wrapper_sha256": capture.WRAPPER_SHA256,
            "instrumented_wrapper_sha256": (
                capture.RAW_WRAPPER_SHA256 if is_raw else capture.WRAPPER_SHA256
            ),
            "instrumentation_id": capture.RAW_DISPLAY_ID if is_raw else None,
            "presentation_replacements": {
                "round": int(is_raw),
                "trunc": 2 * int(is_raw),
            },
            "execution": "single-thread upstream WASM in separate Node process",
            "hosted_website_build_reproduction": False,
        }
        for key, value in expected.items():
            require(key in provenance, f"Missing provenance: {key}")
            equal(provenance[key], value, key)
        for key in (
            "wasm_sha256",
            "wasm_bindings_sha256",
            "capture_python_sha256",
            "capture_javascript_sha256",
            "input_file_sha256",
            "resolved_reference_lock_sha256",
        ):
            require(
                isinstance(provenance.get(key), str)
                and re.fullmatch(r"[0-9a-f]{64}", provenance[key]),
                f"Invalid provenance hash: {key}",
            )
    for key in SHARED_PROVENANCE:
        require(
            key in default["provenance"] and key in raw["provenance"],
            f"Missing provenance: {key}",
        )
        equal(default["provenance"][key], raw["provenance"][key], key)
    adjustments = default["provenance"].get("build_adjustments")
    require(
        isinstance(adjustments, list) and len(adjustments) == 3,
        "Unexpected default build adjustments",
    )
    equal(
        raw["provenance"].get("build_adjustments"),
        adjustments + ["Wrapper presentation only: " + capture.RAW_DISPLAY_ID],
        "raw build adjustments",
    )
    require(
        default["provenance"]["wasm_sha256"] != raw["provenance"]["wasm_sha256"],
        "Presentation modes claim identical WASM binaries",
    )
    for data in (default, raw):
        require(
            data["interface"].get("arithmetic_precision") == "f32",
            "Expected f32 arithmetic",
        )
    equal(
        {k: v for k, v in default["interface"].items() if k not in DISPLAY_FIELDS},
        {k: v for k, v in raw["interface"].items() if k not in DISPLAY_FIELDS},
        "interface",
    )
    for key in ("node", "v8", "platform", "architecture"):
        require(
            key in default["runtime"] and key in raw["runtime"],
            f"Missing runtime: {key}",
        )
        equal(default["runtime"][key], raw["runtime"][key], key)


def compare_node(default: dict, raw: dict, counts: list[int]) -> Counter:
    matched = Counter(nodes=1)
    for key in TREE_FIELDS:
        equal(default[key], raw[key], key)
    for values in (default, raw):
        for field in ("reach_weights", "ev_available", "equity", "expected_values"):
            require(
                len(values[field]) == 2
                and all(
                    len(row) == n for row, n in zip(values[field], counts, strict=True)
                ),
                f"Invalid dimensions: {field}",
            )
        require(len(values["normalized_weights"]) == 2, "Invalid normalized dimensions")
    flags = []
    for is_raw in (False, True):
        rows = [
            [finite(w) if is_raw or w >= DISPLAY_CUTOFF else 0 for w in row]
            for row in raw["reach_weights"]
        ]
        require(all(w >= 0 for row in rows for w in row), "Negative reach")
        flags.append(sum((1 << p) for p, row in enumerate(rows) if not any(row)))
        if not is_raw:
            for p, row in enumerate(rows):
                for h, value in enumerate(row):
                    equal(default["reach_weights"][p][h], display_round(value), "reach")
                    matched["reach_cells"] += 1
    equal(default["wasm_empty_range_flag"], flags[0], "default empty flag")
    equal(raw["wasm_empty_range_flag"], flags[1], "raw empty flag")
    for data, flag in ((default, flags[0]), (raw, flags[1])):
        if flag:
            equal(data["normalized_weights"], [None, None], "empty normalized weights")
        else:
            require(
                all(
                    isinstance(row, list) and len(row) == n
                    for row, n in zip(data["normalized_weights"], counts, strict=True)
                ),
                "Invalid normalized dimensions",
            )
    for p, count in enumerate(counts):
        for h in range(count):
            raw_weight = 0 if flags[1] else finite(raw["normalized_weights"][p][h])
            require(raw_weight >= 0, "Negative normalized weight")
            displayed_weight = display_round(raw_weight)
            if not flags[0]:
                equal(
                    default["normalized_weights"][p][h],
                    displayed_weight,
                    "normalized weight",
                )
                matched["normalized_cells"] += 1
            available = [
                flags[0] == 0 and displayed_weight > 0,
                flags[1] == 0 and raw_weight > 0,
            ]
            for data, expected in zip((default, raw), available, strict=True):
                require(
                    type(data["ev_available"][p][h]) is bool,
                    "Invalid availability type",
                )
                equal(data["ev_available"][p][h], expected, "EV availability")
                for field in ("equity", "expected_values"):
                    value = data[field][p][h]
                    if expected:
                        finite(value)
                    else:
                        equal(value, None, "unavailable " + field)
            if available[0]:
                for field, label in (
                    ("equity", "equity_cells"),
                    ("expected_values", "ev_cells"),
                ):
                    equal(default[field][p][h], display_round(raw[field][p][h]), field)
                    matched[label] += 1
            elif available[1]:
                matched["additional_raw_ev_cells"] += 1
    player = raw["player"]
    size = 0 if player is None else len(raw["actions"]) * counts[player]
    for field in ("strategy", "action_expected_values"):
        require(
            len(default[field]) == len(raw[field]) == size, "Invalid action dimensions"
        )
    for index in range(size):
        equal(
            default["strategy"][index],
            display_round(raw["strategy"][index]),
            "strategy",
        )
        matched["strategy_cells"] += 1
        hand = index % counts[player]
        for data in (default, raw):
            if data["ev_available"][player][hand]:
                finite(data["action_expected_values"][index])
            else:
                equal(
                    data["action_expected_values"][index], None, "unavailable action EV"
                )
        if default["ev_available"][player][hand]:
            equal(
                default["action_expected_values"][index],
                display_round(raw["action_expected_values"][index]),
                "action EV",
            )
            matched["action_ev_cells"] += 1
        elif raw["ev_available"][player][hand]:
            matched["additional_raw_action_ev_cells"] += 1
    return matched


def check_root_aggregates(case: dict) -> None:
    root = case["nodes"][0]
    values = []
    for player in (0, 1):
        numerator = denominator = 0.0
        for value, weight in zip(
            root["expected_values"][player],
            root["normalized_weights"][player],
            strict=True,
        ):
            if value is not None and weight > 0:
                numerator += value * weight
                denominator += weight
        require(denominator > 0, "Root has no compatible mass")
        values.append(numerator / denominator)
    equal(case["root_expected_values"], values, "root weighted EV")
    equal(
        case["root_centered_expected_values"],
        [v - case["input"]["starting_pot"] / 2 for v in values],
        "root centered EV",
    )


def verify(default: dict, raw: dict) -> dict:
    verify_provenance(default, raw)
    payload = {
        "schema_version": 1,
        "cases": [case["input"] for case in default["cases"]],
    }
    capture.validate_inputs(payload)
    for data, is_raw in ((default, False), (raw, True)):
        capture.validate_output(data, payload, finish_budget=True, raw_display=is_raw)
    totals = Counter({key: 0 for key in COUNT_FIELDS})
    cases = []
    for displayed, unrounded in zip(default["cases"], raw["cases"], strict=True):
        require(
            displayed["iterations"] == unrounded["iterations"] == 20000,
            "Expected paired 20,000 iterations",
        )
        equal(displayed["private_cards"], unrounded["private_cards"], "private cards")
        for key in ("exploitability_chips", "exploitability_pct_of_pot"):
            equal(displayed[key], unrounded[key], key)
        checkpoints = []
        for case in (displayed, unrounded):
            check_root_aggregates(case)
            checkpoints.append(
                [
                    (c["iterations"], finite(c["exploitability_chips"]))
                    for c in case["checkpoints"]
                ]
            )
            expected_checks = [0] + list(
                range(case["input"]["check_every"], 20001, case["input"]["check_every"])
            )
            if expected_checks[-1] != 20000:
                expected_checks.append(20000)
            equal(
                [c[0] for c in checkpoints[-1]], expected_checks, "checkpoint schedule"
            )
            equal(
                checkpoints[-1][-1][1], case["exploitability_chips"], "final checkpoint"
            )
        equal(checkpoints[0], checkpoints[1], "residual checkpoints")
        equal(len(displayed["nodes"]), len(unrounded["nodes"]), "node count")
        counts = [len(cards) for cards in displayed["private_cards"]]
        matched = Counter({key: 0 for key in COUNT_FIELDS})
        matched["checkpoints"] = len(checkpoints[0])
        for d_node, r_node in zip(displayed["nodes"], unrounded["nodes"], strict=True):
            matched.update(compare_node(d_node, r_node, counts))
        totals.update(matched)
        cases.append(
            {
                "id": displayed["input"]["id"],
                "iterations": 20000,
                "matched": dict(matched),
            }
        )
    return {
        "verification_version": 1,
        "status": "matched",
        "comparison": "exact_after_upstream_display",
        "instrumentation_id": capture.RAW_DISPLAY_ID,
        "cases": cases,
        "totals": dict(totals),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--default", type=Path, required=True)
    parser.add_argument("--raw", type=Path, required=True)
    args = parser.parse_args()
    result = verify(
        capture.read_json(args.default, capture.MAX_OUTPUT_BYTES),
        capture.read_json(args.raw, capture.MAX_OUTPUT_BYTES),
    )
    result["capture_sha256"] = {
        "default": capture.sha256(args.default),
        "raw": capture.sha256(args.raw),
    }
    print(json.dumps(result, allow_nan=False, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
