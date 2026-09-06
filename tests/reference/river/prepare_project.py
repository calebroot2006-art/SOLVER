"""Validate the shared cases and convert their primitive fields to project TOML."""

import argparse
import json
from pathlib import Path

from capture import MAX_INPUT_BYTES, read_json, validate_inputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    payload = read_json(args.inputs, MAX_INPUT_BYTES)
    validate_inputs(payload)
    lines = ["schema_version = 1", ""]
    for case in payload["cases"]:
        lines.append("[[cases]]")
        for key, value in case.items():
            lines.append(f"{key} = {json.dumps(value, allow_nan=False)}")
        lines.append("")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as stream:
        stream.write("\n".join(lines))


if __name__ == "__main__":
    main()
