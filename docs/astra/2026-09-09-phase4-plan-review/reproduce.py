"""Small plan-review counterexamples; this does not run or certify the solver."""

import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/reference/turn"))

import _fixture
from compare import summarize_reference

reference = summarize_reference(_fixture.reference_capture(stop_reason="iteration_cap"))
positive = 1 / (1 + 2 ** (-1.5))
negative = 1 / (1 + 2**0)
board = ["Ah", "Kh", "Qh"]
hero = ["2s", "3s"]
opponent = ["Ac", "Ad"]
orbit = ["2s", "2c", "2d"]
dead = set(board + hero + opponent)
legal = [card for card in orbit if card not in dead]
representative_count = int(orbit[0] not in dead) * len(orbit)
tiny = struct.unpack("f", struct.pack("f", 1e-30))[0]
underflow = struct.unpack("f", struct.pack("f", 1e-50))[0]

assert positive != negative
assert representative_count == 0 and len(legal) == 2
assert tiny > 0 and underflow == 0
assert reference["mode"] == "reference_only"
assert reference["project_capture"] == "absent"
assert not reference["cases"][0]["reached_target"]

sources = [
    "docs/phase-4/PLAN.md",
    "docs/ROADMAP.md",
    "docs/PRODUCT.md",
    "crates/postflop/src/cfr.rs",
    "crates/postflop/src/solver.rs",
    "tests/reference/turn/compare.py",
    ".github/workflows/ci.yml",
]
result = {
    "reviewed_revision": "b593f40",
    "command": "python docs/astra/2026-09-09-phase4-plan-review/reproduce.py",
    "scope": "Arithmetic and comparator fixtures; no full solve acceptance",
    "f32_1e_minus_30": tiny,
    "f32_1e_minus_50": underflow,
    "dcfr_iteration_2": {
        "positive_factor": positive,
        "negative_factor": negative,
        "discounted_plus_minus_one": [positive, -negative],
    },
    "orbit_blockers": {
        "board": board,
        "hero": hero,
        "opponent": opponent,
        "orbit": orbit,
        "legal_members": legal,
        "representative_2s_mask_times_orbit_size": representative_count,
        "correct_legal_member_count": len(legal),
    },
    "reference_only_cap": {
        "mode": reference["mode"],
        "project_capture": reference["project_capture"],
        "checks": reference["structural_and_convergence_checks"],
        "reached_target": reference["cases"][0]["reached_target"],
    },
    "source_sha256": {
        path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in sources
    },
}
Path(__file__).with_name("evidence.json").write_text(
    json.dumps(result, indent=2) + "\n", encoding="utf-8"
)
print(json.dumps(result, indent=2))
