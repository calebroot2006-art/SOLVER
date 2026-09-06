"""Build and run a pinned external WASM reference; never import our solver."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib

WASM_REVISION = "97360db7644329b1c23a7adf06e9aa59406e4d4b"
ENGINE_REVISION = "9d1509fe5077d019825f833eed04b16d342dfda1"
TOOLCHAIN = "nightly-2023-10-01"
BINDGEN_VERSION = "0.2.87"
NODE_VERSION = "v24.19.0"
MAX_INPUT_BYTES = 64 * 1024
MAX_OUTPUT_BYTES = 64 * 1024 * 1024
MAX_LOG_BYTES = 16 * 1024 * 1024
CASE_KEYS = {
    "id",
    "board",
    "ranges",
    "chips_per_bb",
    "starting_pot",
    "effective_stack",
    "bets",
    "raises",
    "min_bet",
    "max_raises",
    "add_all_in_threshold",
    "force_all_in_threshold",
    "target_pct_of_pot",
    "max_iterations",
    "check_every",
}
ROOT = Path(__file__).resolve().parent


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def reject_constant(value: str) -> None:
    raise ValueError(f"Nonfinite JSON constant: {value}")


def finite_float(value: str) -> float:
    number = float(value)
    require(math.isfinite(number), "JSON number overflows floating point")
    return number


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        require(key not in result, f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path, limit: int) -> dict:
    require(
        path.is_file() and path.stat().st_size <= limit, f"Invalid file size: {path}"
    )
    with path.open("rb") as stream:
        content = stream.read(limit + 1)
    require(len(content) <= limit, "File grew beyond its size limit")
    value = json.loads(
        content,
        parse_constant=reject_constant,
        parse_float=finite_float,
        object_pairs_hook=unique_object,
    )
    require(type(value) is dict, "JSON root must be an object")
    return value


def validate_inputs(payload: dict) -> None:
    require(set(payload) == {"schema_version", "cases"}, "Unknown input root fields")
    require(
        type(payload["schema_version"]) is int and payload["schema_version"] == 1,
        "Unsupported schema",
    )
    cases = payload["cases"]
    require(type(cases) is list and 1 <= len(cases) <= 8, "Expected 1 to 8 cases")
    ids = set()
    for case in cases:
        require(type(case) is dict and set(case) == CASE_KEYS, "Invalid case fields")
        name = case["id"]
        require(
            type(name) is str
            and re.fullmatch(r"[a-z][a-z0-9_]{0,63}", name) is not None,
            "Invalid case id",
        )
        require(name not in ids, "Duplicate case id")
        ids.add(name)
        board = case["board"]
        require(type(board) is list and len(board) == 5, "Expected five board cards")
        require(
            all(type(c) is str and re.fullmatch(r"[2-9TJQKA][cdhs]", c) for c in board),
            "Invalid board card",
        )
        require(len(set(board)) == 5, "Repeated board card")
        ranges = case["ranges"]
        require(type(ranges) is list and len(ranges) == 2, "Expected two ranges")
        for text in ranges:
            require(type(text) is str and 1 <= len(text) <= 1024, "Invalid range size")
            items = text.split(",")
            require(
                1 <= len(items) <= 32 and len(items) == len(set(items)),
                "Invalid range groups",
            )
            for item in items:
                require(
                    re.fullmatch(r"[2-9TJQKA]{2}", item) is not None,
                    "Only unweighted two-rank groups are supported by this capture",
                )
                ranks = "23456789TJQKA"
                require(
                    ranks.index(item[0]) >= ranks.index(item[1]), "Reversed range group"
                )
        # Restrict this runner to the small trees agreed in reference-contract.md.
        for field, allowed in (("bets", {"", "50%"}), ("raises", {"", "100%"})):
            require(
                type(case[field]) is list and len(case[field]) == 2, f"Invalid {field}"
            )
            require(
                all(type(x) is str and x in allowed for x in case[field]),
                f"Unsupported {field}",
            )
        for field, value in (
            ("chips_per_bb", 1),
            ("starting_pot", 10),
            ("min_bet", 1),
            ("max_raises", 32),
            ("add_all_in_threshold", 0),
            ("force_all_in_threshold", 0),
        ):
            require(
                type(case[field]) is int and case[field] == value,
                f"Unsupported {field}",
            )
        require(
            type(case["effective_stack"]) is int
            and case["effective_stack"] in {20, 100, 200},
            "Unsupported effective stack",
        )
        require(
            type(case["max_iterations"]) is int
            and 1 <= case["max_iterations"] <= 20000,
            "Iteration limit must be 1 to 20000",
        )
        require(
            type(case["check_every"]) is int and 1 <= case["check_every"] <= 100,
            "Check interval must be 1 to 100",
        )
        target = case["target_pct_of_pot"]
        require(
            type(target) in {int, float}
            and math.isfinite(target)
            and 0.001 <= target <= 0.5,
            "Target must be 0.001 to 0.5 percent of pot",
        )


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_command(
    args: list[str], cwd: Path, env: dict, timeout: int, commands: list
) -> str:
    """Bound subprocess time/log output and kill the entire process group on failure."""
    log = cwd / f"command-{len(commands):03d}.log"
    started = time.monotonic()
    print(f"Reference step {len(commands) + 1}: {args[0]} {args[1:3]!r}", flush=True)
    with log.open("wb") as output:
        process = subprocess.Popen(
            args,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            while process.poll() is None:
                if (
                    time.monotonic() - started > timeout
                    or log.stat().st_size > MAX_LOG_BYTES
                ):
                    raise TimeoutError(
                        f"Reference command exceeded time/log budget: {args[0]}"
                    )
                time.sleep(0.1)
            require(log.stat().st_size <= MAX_LOG_BYTES, "Reference log exceeds budget")
            if process.returncode:
                tail = log.read_bytes()[-6000:].decode("utf-8", errors="replace")
                raise RuntimeError(
                    f"Command failed ({process.returncode}): {args!r}\n{tail}"
                )
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    commands.append({"argv": args, "seconds": time.monotonic() - started})
    return log.read_text(encoding="utf-8", errors="replace").strip()


def replace_once(path: Path, before: str, after: str) -> None:
    text = path.read_text(encoding="utf-8")
    require(text.count(before) == 1, f"Unexpected upstream manifest: {path.name}")
    path.write_text(text.replace(before, after), encoding="utf-8")


def validate_output(output: dict, payload: dict) -> None:
    require(
        output.get("schema_version") == 1 and output.get("capture_version") == 1,
        "Unexpected reference schema",
    )
    require(
        output.get("runtime", {}).get("node") == NODE_VERSION, "Unexpected Node runtime"
    )
    cases = output.get("cases")
    require(
        type(cases) is list and len(cases) == len(payload["cases"]), "Missing cases"
    )
    for result, expected in zip(cases, payload["cases"], strict=True):
        require(result.get("input") == expected, "Reference changed its inputs")
        require(
            type(result.get("iterations")) is int
            and 0 <= result["iterations"] <= expected["max_iterations"],
            "Invalid measured iteration count",
        )
        residual = result.get("exploitability_chips")
        require(
            type(residual) in {float, int} and math.isfinite(residual),
            "Invalid residual",
        )
        require(
            abs(
                result["exploitability_pct_of_pot"]
                - 100 * residual / expected["starting_pot"]
            )
            < 1e-12,
            "Residual units differ",
        )
        target = expected["target_pct_of_pot"] * expected["starting_pot"] / 100
        reason = "target" if residual <= target else "iteration_cap"
        require(result.get("stop_reason") == reason, "Incorrect stop reason")
        require(
            reason != "iteration_cap"
            or result["iterations"] == expected["max_iterations"],
            "Premature iteration cap",
        )
        nodes = result.get("nodes")
        require(type(nodes) is list and 1 <= len(nodes) <= 10000, "Invalid node count")
        private = result.get("private_cards")
        require(type(private) is list and len(private) == 2, "Missing private cards")
        counts = []
        for player, entries in enumerate(private):
            require(
                type(entries) is list and 1 <= len(entries) <= 1326,
                "Invalid private hand count",
            )
            seen = set()
            for entry in entries:
                ids = entry.get("ids")
                require(
                    type(ids) is list
                    and len(ids) == 2
                    and all(type(i) is int for i in ids)
                    and 0 <= ids[0] < ids[1] < 52,
                    "Invalid private card IDs",
                )
                cards = ["23456789TJQKA"[i // 4] + "cdhs"[i % 4] for i in ids]
                require(
                    entry.get("cards") == cards, "Private card labels differ from IDs"
                )
                require(tuple(ids) not in seen, "Duplicate private hand")
                seen.add(tuple(ids))
            # Independent expansion checks that no legal input hand disappeared.
            expected_hands = set()
            for low in range(52):
                for high in range(low + 1, 52):
                    group = "23456789TJQKA"[high // 4] + "23456789TJQKA"[low // 4]
                    cards = {
                        "23456789TJQKA"[i // 4] + "cdhs"[i % 4] for i in (low, high)
                    }
                    if group in expected["ranges"][player].split(
                        ","
                    ) and not cards.intersection(expected["board"]):
                        expected_hands.add((low, high))
            live = {
                pair
                for pair in seen
                if not {
                    "23456789TJQKA"[i // 4] + "cdhs"[i % 4] for i in pair
                }.intersection(expected["board"])
            }
            require(
                live == expected_hands, "Reference physical range differs from input"
            )
            counts.append(len(entries))
        histories = {}
        for node in nodes:
            history = node.get("history")
            require(
                type(history) is list
                and len(history) <= 64
                and all(type(i) is int and 0 <= i < 4 for i in history),
                "Invalid history",
            )
            key = tuple(history)
            require(key not in histories, "Repeated history")
            histories[key] = node
            labels = node.get("history_labels")
            require(
                type(labels) is list and len(labels) == len(history),
                "Mismatched history labels",
            )
            wagers = sum(
                label.startswith(("bet:", "raise:", "allin:")) for label in labels
            )
            require(
                max(0, wagers - 1) <= expected["max_raises"],
                "Reference exceeded project raise cap",
            )
            actions = node.get("actions")
            require(type(actions) is list and len(actions) <= 3, "Invalid action count")
            require(
                node.get("kind") in {"decision", "terminal"}, "Unexpected node kind"
            )
            require(
                (node["kind"] == "terminal") == (len(actions) == 0), "Invalid terminal"
            )
            player = node.get("player")
            if actions:
                require(
                    type(player) is int and player in {0, 1}, "Invalid acting player"
                )
                require(player == len(history) % 2, "Incorrect acting player")
            else:
                require(player is None, "Terminal has an acting player")
            size = len(actions) * counts[player] if actions else 0
            for field in ("strategy", "action_expected_values"):
                require(
                    type(node.get(field)) is list and len(node[field]) == size,
                    f"Invalid {field} dimensions",
                )
            for field in ("reach_weights", "equity", "expected_values", "ev_available"):
                rows = node.get(field)
                require(
                    type(rows) is list
                    and len(rows) == 2
                    and all(
                        type(row) is list and len(row) == count
                        for row, count in zip(rows, counts, strict=True)
                    ),
                    f"Invalid {field} dimensions",
                )
            for p in (0, 1):
                for h in range(counts[p]):
                    available = node["ev_available"][p][h]
                    require(type(available) is bool, "Invalid EV availability")
                    require(
                        (node["expected_values"][p][h] is not None) == available,
                        "Unavailable EV was filled",
                    )
                    if actions and p == player:
                        row = [
                            node["strategy"][a * counts[p] + h]
                            for a in range(len(actions))
                        ]
                        require(
                            all(
                                type(v) in {int, float}
                                and math.isfinite(v)
                                and 0 <= v <= 1
                                for v in row
                            )
                            and abs(sum(row) - 1) <= 0.00002,
                            "Invalid strategy row",
                        )
                        require(
                            all(
                                (
                                    node["action_expected_values"][a * counts[p] + h]
                                    is not None
                                )
                                == available
                                for a in range(len(actions))
                            ),
                            "Unavailable action EV was filled",
                        )
        require(() in histories, "Missing root")
        for history, node in histories.items():
            if history:
                parent = histories.get(history[:-1])
                require(
                    parent is not None and history[-1] < len(parent["actions"]),
                    "Orphan node",
                )
                require(
                    node["history_labels"]
                    == parent["history_labels"]
                    + [parent["actions"][history[-1]]["label"]],
                    "History label mismatch",
                )
            for index in range(len(node["actions"])):
                require(history + (index,) in histories, "Reference omitted a branch")


def orchestrate(inputs: Path, destination: Path, temp_root: Path) -> None:
    require(sys.platform == "linux", "Reference compilation runs only on Linux CI")
    payload = read_json(inputs, MAX_INPUT_BYTES)
    validate_inputs(payload)
    require(not destination.exists(), "Refusing to overwrite an existing capture")
    workspace = Path(os.environ.get("GITHUB_WORKSPACE", ROOT.parents[2])).resolve()
    temp_root = temp_root.resolve(strict=True)
    require(
        not temp_root.is_relative_to(workspace),
        "External temp root is inside the repository",
    )
    commands: list[dict] = []
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="river-wasm-", dir=temp_root) as directory:
        external = Path(directory)
        env = dict(os.environ)
        env.update(
            {
                "CARGO_HOME": str(external / "cargo"),
                "RUSTUP_HOME": str(external / "rustup"),
                "CARGO_TARGET_DIR": str(external / "target"),
                "CARGO_BUILD_JOBS": "2",
                "GIT_TERMINAL_PROMPT": "0",
                "CARGO_TERM_COLOR": "never",
            }
        )

        def command(args, cwd=external, timeout=600):
            return run_command(args, cwd, env, timeout, commands)

        node = command(["node", "--version"])
        require(node == NODE_VERSION, f"Expected Node {NODE_VERSION}, got {node}")
        for name, revision in (
            ("wasm-postflop", WASM_REVISION),
            ("postflop-solver", ENGINE_REVISION),
        ):
            checkout = external / name
            checkout.mkdir()
            command(["git", "init", "--quiet"], checkout)
            command(
                [
                    "git",
                    "remote",
                    "add",
                    "origin",
                    f"https://github.com/b-inary/{name}.git",
                ],
                checkout,
            )
            command(
                ["git", "fetch", "--quiet", "--depth", "1", "origin", revision],
                checkout,
            )
            command(["git", "checkout", "--quiet", "--detach", "FETCH_HEAD"], checkout)
            require(
                command(["git", "rev-parse", "HEAD"], checkout) == revision,
                "Wrong reference revision",
            )
            require((checkout / "LICENSE").is_file(), "Upstream license missing")
        reference = external / "wasm-postflop" / "rust" / "solver-st"
        engine_manifest = external / "postflop-solver" / "Cargo.toml"
        manifest = reference / "Cargo.toml"
        before_hashes = {
            "wasm_manifest": sha256(manifest),
            "engine_manifest": sha256(engine_manifest),
        }
        replace_once(
            manifest,
            'postflop-solver = { git = "https://github.com/b-inary/postflop-solver",',
            'postflop-solver = { path = "../../../postflop-solver",',
        )
        replace_once(manifest, 'wasm-bindgen = "0.2.87"', 'wasm-bindgen = "=0.2.87"')
        replace_once(engine_manifest, 'once_cell = "1.18.0"', 'once_cell = "=1.18.0"')
        replace_once(engine_manifest, 'regex = "1.9.6"', 'regex = "=1.9.6"')
        command(
            [
                "rustup",
                "toolchain",
                "install",
                TOOLCHAIN,
                "--profile",
                "minimal",
                "--component",
                "rust-src",
                "--target",
                "wasm32-unknown-unknown",
            ],
            timeout=900,
        )
        rustc = command(["rustup", "run", TOOLCHAIN, "rustc", "-Vv"])
        cargo = ["rustup", "run", TOOLCHAIN, "cargo"]
        command(
            cargo
            + [
                "install",
                "wasm-bindgen-cli",
                "--version",
                BINDGEN_VERSION,
                "--locked",
                "--root",
                str(external / "tools"),
            ],
            timeout=1800,
        )
        command(cargo + ["generate-lockfile"], reference)
        command(
            cargo
            + ["build", "--locked", "--release", "--target", "wasm32-unknown-unknown"],
            reference,
            timeout=900,
        )
        bindgen = external / "tools" / "bin" / "wasm-bindgen"
        version = command([str(bindgen), "--version"])
        require(
            version == f"wasm-bindgen {BINDGEN_VERSION}",
            "Unexpected wasm-bindgen version",
        )
        package = external / "package"
        command(
            [
                str(bindgen),
                str(
                    external
                    / "target"
                    / "wasm32-unknown-unknown"
                    / "release"
                    / "solver.wasm"
                ),
                "--target",
                "nodejs",
                "--out-dir",
                str(package),
                "--out-name",
                "solver",
            ]
        )
        validated_inputs = external / "inputs.json"
        validated_inputs.write_text(
            json.dumps(payload, allow_nan=False), encoding="utf-8"
        )
        driver = external / "capture.mjs"
        shutil.copyfile(ROOT / "capture.mjs", driver)
        raw_output = external / "capture.json"
        command(
            [
                "node",
                "--max-old-space-size=1024",
                str(driver),
                str(package / "solver.js"),
                str(validated_inputs),
                str(raw_output),
            ],
            timeout=1200,
        )
        output = read_json(raw_output, MAX_OUTPUT_BYTES)
        validate_output(output, payload)
        lock = tomllib.loads((reference / "Cargo.lock").read_text(encoding="utf-8"))
        output["provenance"] = {
            "wasm_postflop_revision": WASM_REVISION,
            "engine_revision": ENGINE_REVISION,
            "toolchain": TOOLCHAIN,
            "rustc_verbose": rustc,
            "wasm_bindgen_cli": version,
            "python": sys.version,
            "capture_python_sha256": sha256(Path(__file__)),
            "capture_javascript_sha256": sha256(driver),
            "input_file_sha256": sha256(inputs),
            "resolved_reference_lock_sha256": sha256(reference / "Cargo.lock"),
            "resolved_dependencies": [
                {
                    key: package[key]
                    for key in ("name", "version", "source", "checksum")
                    if key in package
                }
                for package in lock["package"]
            ],
            "wasm_sha256": sha256(package / "solver_bg.wasm"),
            "wasm_bindings_sha256": sha256(package / "solver.js"),
            "upstream_manifest_hashes": before_hashes,
            "adjusted_manifest_hashes": {
                "wasm_manifest": sha256(manifest),
                "engine_manifest": sha256(engine_manifest),
            },
            "build_adjustments": [
                "Engine dependency replaced with verified local pinned checkout",
                "wasm-bindgen =0.2.87, once_cell =1.18.0, regex =1.9.6",
                "wasm-bindgen nodejs bindings; no wasm-opt; unmodified solver source",
            ],
            "execution": "single-thread upstream WASM in separate Node process",
            "hosted_website_build_reproduction": False,
            "elapsed_seconds": time.monotonic() - started,
            "commands": commands,
        }
        serialized = json.dumps(output, allow_nan=False, separators=(",", ":")) + "\n"
        require(
            len(serialized.encode("utf-8")) <= MAX_OUTPUT_BYTES,
            "Final capture exceeds 64 MiB",
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        staging = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=destination.parent,
                prefix=".river-capture-",
                delete=False,
            ) as stream:
                staging = Path(stream.name)
                stream.write(serialized)
                stream.flush()
                os.fsync(stream.fileno())
            # Same-directory link publishes atomically and refuses an existing destination.
            os.link(staging, destination)
        finally:
            if staging is not None:
                staging.unlink(missing_ok=True)
        print(
            f"Saved {len(output['cases'])} measured WASM cases to {destination}",
            flush=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, default=ROOT / "cases.json")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--temp-root", type=Path, default=os.environ.get("RUNNER_TEMP"))
    parser.add_argument("--validate-only", action="store_true")
    args = parser.parse_args()
    if args.validate_only:
        validate_inputs(read_json(args.inputs, MAX_INPUT_BYTES))
        print("Input contract passed")
        return
    require(
        args.output is not None and args.temp_root is not None,
        "Capture requires --output and --temp-root (or RUNNER_TEMP)",
    )
    orchestrate(args.inputs.resolve(), args.output.resolve(), args.temp_root)


if __name__ == "__main__":
    main()
