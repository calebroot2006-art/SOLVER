"""The canonical enumeration is exactly 1,755 flops, and the sampler is reproducible."""

from __future__ import annotations

import json
import random
import unittest
from itertools import combinations, permutations
from pathlib import Path

import select_flops

ROOT = Path(__file__).resolve().parent


class CanonicalEnumerationTests(unittest.TestCase):
    def test_there_are_exactly_1755_suit_canonical_flops(self):
        self.assertEqual(len(select_flops.canonical_flops()), 1755)

    def test_canonicalisation_is_idempotent(self):
        for flop in select_flops.canonical_flops()[:200]:
            self.assertEqual(select_flops.canonical(flop), flop)

    def test_relabelling_the_suits_does_not_change_the_canonical_form(self):
        rng = random.Random(11)
        deck = [(rank, suit) for rank in range(13) for suit in range(4)]
        for _ in range(200):
            flop = tuple(rng.sample(deck, 3))
            expected = select_flops.canonical(flop)
            for permutation in permutations(range(4)):
                relabelled = tuple((rank, permutation[suit]) for rank, suit in flop)
                self.assertEqual(select_flops.canonical(relabelled), expected)

    def test_every_three_card_combination_maps_into_the_canonical_set(self):
        deck = [(rank, suit) for rank in range(13) for suit in range(4)]
        canonical = set(select_flops.canonical_flops())
        count = 0
        for flop in combinations(deck, 3):
            self.assertIn(select_flops.canonical(flop), canonical)
            count += 1
        self.assertEqual(count, 22100)


class AllocationTests(unittest.TestCase):
    def test_every_bucket_gets_at_least_one_and_the_total_is_exact(self):
        sizes = {("a",): 600, ("b",): 200, ("c",): 3, ("d",): 3}
        counts = select_flops.allocate(sizes, 49)
        self.assertEqual(sum(counts.values()), 49)
        self.assertTrue(all(count >= 1 for count in counts.values()))
        self.assertTrue(all(counts[key] <= sizes[key] for key in sizes))
        self.assertGreater(counts[("a",)], counts[("b",)])

    def test_a_bucket_is_never_over_drawn(self):
        sizes = {("a",): 2, ("b",): 2, ("c",): 1}
        counts = select_flops.allocate(sizes, 5)
        self.assertEqual(counts, {("a",): 2, ("b",): 2, ("c",): 1})

    def test_more_buckets_than_places_is_refused(self):
        sizes = {(str(index),): 10 for index in range(50)}
        with self.assertRaises(ValueError):
            select_flops.allocate(sizes, 49)


class SamplerTests(unittest.TestCase):
    def test_the_sampler_is_deterministic(self):
        self.assertEqual(select_flops.select()["flops"], select_flops.select()["flops"])

    def test_a_different_seed_gives_a_different_sample(self):
        self.assertNotEqual(
            select_flops.select()["flops"], select_flops.select(seed=1)["flops"]
        )

    def test_the_sample_is_49_distinct_canonical_flops(self):
        result = select_flops.select()
        labels = result["flops"]
        self.assertEqual(len(labels), 49)
        self.assertEqual(len(set(labels)), 49)
        canonical = {
            select_flops.flop_label(flop) for flop in select_flops.canonical_flops()
        }
        self.assertTrue(set(labels).issubset(canonical))
        self.assertEqual(sum(bucket["sampled"] for bucket in result["buckets"]), 49)
        self.assertEqual(
            sum(bucket["population"] for bucket in result["buckets"]),
            result["canonical_flop_count"],
        )

    def test_every_non_empty_texture_bucket_appears(self):
        result = select_flops.select()
        self.assertEqual(result["bucket_count"], 43)
        for bucket in result["buckets"]:
            self.assertGreaterEqual(bucket["sampled"], 1)
            self.assertEqual(len(bucket["flops"]), bucket["sampled"])

    def test_the_committed_file_matches_a_fresh_selection(self):
        committed = json.loads((ROOT / "flops.json").read_text(encoding="utf-8"))
        self.assertEqual(committed, json.loads(json.dumps(select_flops.select())))


if __name__ == "__main__":
    unittest.main()
