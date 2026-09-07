"""Choose the 49-flop cross-check subset: enumerate, bucket by texture, sample by seed.

Pure standard library. Run it to write `flops.json`, or with `--check` to prove the committed
file is what this script produces.

The subset has to be defensible, not merely random. Two flops that differ only by a
relabelling of suits solve to the same strategy, so the population is the 1,755 suit-canonical
flops rather than all 22,100 three-card combinations. Those are then bucketed by the texture
features that change how a spot plays, every non-empty bucket gets at least one flop so no
texture is missing, and the rest are handed out in proportion to bucket size so the sample
still looks like the real distribution.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import sys
from itertools import combinations, permutations
from pathlib import Path

SCHEMA_VERSION = 1
SAMPLE_SIZE = 49
SEED = 20260906
CANONICAL_FLOP_COUNT = 1755
RANKS = "23456789TJQKA"
SUITS = "cdhs"
ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "flops.json"

BUCKET_DIMENSIONS = {
    "pairing": {
        "trips": "one distinct rank",
        "paired": "two distinct ranks",
        "unpaired": "three distinct ranks",
    },
    "suits": {
        "monotone": "one suit",
        "two_tone": "two suits",
        "rainbow": "three suits",
    },
    "connectedness": {
        "connected": "highest rank minus lowest distinct rank is at most 2",
        "one_gap": "that span is 3 or 4",
        "disconnected": "that span is 5 or more",
    },
    "high_card": {
        "high": "highest rank is ten or better",
        "middle": "highest rank is seven, eight or nine",
        "low": "highest rank is six or lower",
    },
}


def card_label(card: tuple[int, int]) -> str:
    return RANKS[card[0]] + SUITS[card[1]]


def flop_label(flop: tuple[tuple[int, int], ...]) -> str:
    return "".join(card_label(card) for card in flop)


def canonical(flop) -> tuple[tuple[int, int], ...]:
    """The smallest relabelling of the suits, with cards in descending order.

    Two flops are suit-isomorphic exactly when this is equal, because it minimises over the
    whole suit permutation group and the ordering inside a flop is fixed first.
    """
    best = None
    for permutation in permutations(range(4)):
        relabelled = tuple(
            sorted(((rank, permutation[suit]) for rank, suit in flop), reverse=True)
        )
        if best is None or relabelled < best:
            best = relabelled
    return best


def canonical_flops() -> list[tuple[tuple[int, int], ...]]:
    deck = [(rank, suit) for rank in range(13) for suit in range(4)]
    return sorted({canonical(flop) for flop in combinations(deck, 3)}, reverse=True)


def bucket_of(flop) -> tuple[str, str, str, str]:
    distinct = sorted({rank for rank, _ in flop}, reverse=True)
    pairing = {1: "trips", 2: "paired", 3: "unpaired"}[len(distinct)]
    suits = {1: "monotone", 2: "two_tone", 3: "rainbow"}[
        len({suit for _, suit in flop})
    ]
    span = distinct[0] - distinct[-1]
    connectedness = (
        "connected" if span <= 2 else "one_gap" if span <= 4 else "disconnected"
    )
    top = distinct[0]
    high_card = (
        "high"
        if top >= RANKS.index("T")
        else "middle" if top >= RANKS.index("7") else "low"
    )
    return (pairing, suits, connectedness, high_card)


def allocate(
    sizes: dict[tuple[str, ...], int], sample_size: int
) -> dict[tuple[str, ...], int]:
    """One per non-empty bucket, then the remainder in proportion, largest remainder first."""
    if len(sizes) > sample_size:
        raise ValueError(
            f"{len(sizes)} non-empty buckets cannot each get one of {sample_size}"
        )
    total = sum(sizes.values())
    counts = {key: 1 for key in sizes}
    remaining = sample_size - len(sizes)
    shares = {key: remaining * size / total for key, size in sizes.items()}
    for key, share in shares.items():
        counts[key] += int(share)
    handed_out = sum(counts.values())
    # Ties break on bucket size, then on the bucket key, so the result never depends on
    # dictionary order or on floating-point noise deciding two equal remainders.
    order = sorted(
        sizes,
        key=lambda key: (-(shares[key] - int(shares[key])), -sizes[key], key),
    )
    for key in order:
        if handed_out >= sample_size:
            break
        if counts[key] < sizes[key]:
            counts[key] += 1
            handed_out += 1
    if handed_out != sample_size:
        raise ValueError(f"Allocated {handed_out} flops, expected {sample_size}")
    for key, count in counts.items():
        if count > sizes[key]:
            raise ValueError(f"Bucket {key} was allocated more flops than it holds")
    return counts


def select(sample_size: int = SAMPLE_SIZE, seed: int = SEED) -> dict:
    population = canonical_flops()
    if len(population) != CANONICAL_FLOP_COUNT:
        raise ValueError(
            f"Expected {CANONICAL_FLOP_COUNT} canonical flops, got {len(population)}"
        )
    members: dict[tuple[str, ...], list] = {}
    for flop in population:
        members.setdefault(bucket_of(flop), []).append(flop)
    sizes = {key: len(value) for key, value in members.items()}
    counts = allocate(sizes, sample_size)
    rng = random.Random(seed)
    buckets = []
    chosen = []
    for key in sorted(members):
        ordered = sorted(members[key], reverse=True)
        picked = sorted(rng.sample(ordered, counts[key]), reverse=True)
        chosen.extend(picked)
        buckets.append(
            {
                "pairing": key[0],
                "suits": key[1],
                "connectedness": key[2],
                "high_card": key[3],
                "population": sizes[key],
                "sampled": counts[key],
                "flops": [flop_label(flop) for flop in picked],
            }
        )
    chosen.sort(reverse=True)
    return {
        "schema_version": SCHEMA_VERSION,
        "seed": seed,
        "sample_size": sample_size,
        "canonical_flop_count": len(population),
        "method": (
            "Suit-canonical flops, bucketed on four texture features. Every non-empty bucket "
            "gets one flop; the remaining places go in proportion to bucket size, largest "
            "fractional remainder first. Within a bucket the flops are drawn with "
            "random.Random(seed) over the descending-sorted members, in sorted bucket order."
        ),
        "bucket_dimensions": BUCKET_DIMENSIONS,
        "bucket_count": len(buckets),
        "buckets": buckets,
        "flops": [flop_label(flop) for flop in chosen],
        "select_flops_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail instead of writing if the file on disk differs from a fresh selection",
    )
    args = parser.parse_args()
    serialized = json.dumps(select(), indent=2, allow_nan=False) + "\n"
    if args.check:
        if not args.output.is_file():
            print(f"{args.output} does not exist", file=sys.stderr)
            return 1
        if args.output.read_text(encoding="utf-8") != serialized:
            print(f"{args.output} differs from a fresh selection", file=sys.stderr)
            return 1
        print(f"{args.output} matches a fresh selection")
        return 0
    args.output.write_text(serialized, encoding="utf-8")
    print(f"Wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
