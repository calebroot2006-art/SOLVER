"""Generate independent golden inputs for the Rust exact reach accumulator.

Run from the repository root with Python 3.12 or later:
    python tests/reference/exact_mass_vectors.py
    python tests/reference/exact_mass_vectors.py --check

Only the Python standard library is used. Expected values use Fraction.from_float
and Python's Fraction-to-float conversion; there is no port of the Rust integer
accumulator or its rounding algorithm. The Rust unit test consumes the CSV and
executes the actual implementation. Input tokens are binary64 hex bits followed
by '*' and a repetition count. Subtrahends are always a subset of the addends.
"""

from __future__ import annotations

import argparse
from collections import Counter
from fractions import Fraction
import math
from pathlib import Path
import random
import struct
import sys


SEED = 0x14CE23019872
CASE_COUNT = 1000
MAX_INPUTS = 1327
MAX_BYTES = 200 * 1024
DESTINATION = (
    Path(__file__).resolve().parents[2]
    / "crates/postflop/src/terminal/exact_mass_vectors.csv"
)


def from_bits(bits: int) -> float:
    return struct.unpack("<d", struct.pack("<Q", bits))[0]


def to_bits(value: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def expected(addends: list[tuple[int, int]], removed: list[tuple[int, int]]) -> str:
    added = Counter()
    taken = Counter()
    exact = Fraction(0)
    for bits, count in addends:
        value = from_bits(bits)
        assert math.isfinite(value) and value >= 0 and count > 0
        added[bits] += count
        exact += Fraction.from_float(value) * count
    for bits, count in removed:
        assert count > 0
        taken[bits] += count
        exact -= Fraction.from_float(from_bits(bits)) * count
    assert sum(added.values()) <= MAX_INPUTS
    assert all(count <= added[bits] for bits, count in taken.items())
    assert exact >= 0
    try:
        value = float(exact)
    except OverflowError:
        return "overflow"
    assert math.isfinite(value)
    return f"{to_bits(value):016x}"


def encode(terms: list[tuple[int, int]]) -> str:
    return " ".join(f"{bits:016x}*{count}" for bits, count in terms)


def generate() -> bytes:
    assert sys.float_info.radix == 2 and sys.float_info.mant_dig == 53
    assert sys.float_info.max_exp == 1024
    rng = random.Random(SEED)
    cases: list[tuple[str, list[tuple[int, int]], list[tuple[int, int]]]] = []

    def append(name: str, adds: list[tuple[int, int]], removes=None) -> None:
        cases.append((name, adds, [] if removes is None else removes))

    def values(*numbers: float) -> list[tuple[int, int]]:
        return [(to_bits(number), 1) for number in numbers]

    one = to_bits(1.0)
    maximum = to_bits(sys.float_info.max)
    half = 2.0**-53
    append("zero", [(0, 1)])
    append("negative_zero", [(1 << 63, 1)])
    append("min_subnormal", [(1, 1)])
    append("subnormal_carry", [((1 << 52) - 1, 1), (1, 1)])
    append("even_midpoint", values(1.0, half))
    append("odd_midpoint", [(one + 1, 1), (to_bits(half), 1)])
    append("midpoint_sticky", values(1.0, half, from_bits(1)))
    append("below_midpoint", values(1.0, math.nextafter(half, 0.0)))
    append("round_significand_carry", values(math.nextafter(2.0, 0.0), half))
    append("max_finite", [(maximum, 1)])
    append("overflow_below_midpoint", values(sys.float_info.max, 2.0**969))
    append("overflow_midpoint", values(sys.float_info.max, 2.0**970))
    append("overflow_sticky", values(sys.float_info.max, 2.0**970, from_bits(1)))
    append("capacity_cancellation", [(maximum, 1326), (1, 1)], [(maximum, 1326)])
    append("capacity_overflow", [(maximum, 1327)])
    append("capacity_max_residual", [(maximum, 1327)], [(maximum, 1326)])
    append("all_cancel", values(1e300, 1e100, from_bits(1)), values(1e300, 1e100, from_bits(1)))

    # The first three inputs in 800 cases cover every finite exponent field,
    # including subnormals. Extra random inputs and removal order vary carries
    # and cancellation without relying on production bit-decomposition code.
    exponents = set()
    for case in range(800):
        adds = []
        for index in range(rng.randrange(3, 7)):
            exponent = (3 * case + index) % 2047
            exponents.add(exponent)
            adds.append(((exponent << 52) | rng.getrandbits(52), 1))
        removes = [term for term in adds if rng.getrandbits(1)]
        rng.shuffle(adds)
        rng.shuffle(removes)
        append(f"span_{case:03d}", adds, removes)
    assert exponents == set(range(2047))

    while len(cases) < CASE_COUNT:
        case = len(cases)
        mode = case % 4
        if mode == 0:
            exponent = rng.randrange(2, 2047)
            base = (exponent << 52) | rng.getrandbits(52)
            midpoint = math.ldexp(1.0, exponent - 1076)
            terms = [(base, 1), (to_bits(midpoint), 1)]
            if rng.getrandbits(1):
                terms.append((1, 1))
            append(f"midpoint_{case}", terms)
        elif mode == 1:
            count = rng.randrange(1, 1327)
            small = rng.randrange(1, 1 << 52)
            append(f"cancel_{case}", [(maximum, count), (small, 1)], [(maximum, count)])
        elif mode == 2:
            exponent = rng.randrange(2043, 2047)
            large = (exponent << 52) | rng.getrandbits(52)
            count = rng.randrange(2, 1328)
            removed = rng.randrange(count + 1)
            append(f"overflow_{case}", [(large, count)], [(large, removed)] if removed else [])
        else:
            # Two exact power-of-two terms cross a limb boundary when one is
            # removed; the arbitrary subnormal remains after cancellation.
            first = to_bits(math.ldexp(1.0, rng.randrange(-1022, 1024)))
            second = to_bits(math.ldexp(1.0, rng.randrange(-1022, 1024)))
            small = rng.randrange(1, 1 << 52)
            append(f"borrow_{case}", [(first, 1), (small, 1), (second, 1)], [(second, 1), (first, 1)])

    rows = ["case,addends,subtrahends,expected"]
    for name, adds, removes in cases:
        rows.append(f"{name},{encode(adds)},{encode(removes)},{expected(adds, removes)}")
    result = ("\n".join(rows) + "\n").encode("ascii")
    assert len(cases) == CASE_COUNT and len(result) < MAX_BYTES
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the tracked CSV without writing")
    args = parser.parse_args()
    result = generate()
    if args.check:
        if DESTINATION.read_bytes() != result:
            raise SystemExit("Exact mass fixture differs; regenerate and review its diff.")
        print(f"Verified {CASE_COUNT} independent exact-mass cases ({len(result)} bytes).")
    else:
        DESTINATION.write_bytes(result)
        print(f"Wrote {CASE_COUNT} independent exact-mass cases ({len(result)} bytes).")


if __name__ == "__main__":
    main()
