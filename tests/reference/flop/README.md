# Flop cross-check subset

The 49 flops the phase 4 flop gate cross-checks against the pinned external solver
(`docs/ROADMAP.md:139-156`). Only the subset lives here so far. Step 8 of
`docs/phase-4/PLAN.md` adds the capture, the comparison and the CI jobs that use it, in the
shape `tests/reference/turn/` already has.

## Files

| File | What it does |
| --- | --- |
| `select_flops.py` | Enumerates, buckets and samples. Writes `flops.json`. |
| `flops.json` | The committed subset, with the seed, the buckets and the script's hash. |
| `test_select_flops.py` | The guards, including the 1,755 count and reproducibility. |

## How the 49 were chosen

Neither pinned upstream repository publishes a subset, and Decision 8 in
`docs/phase-4/PLAN.md` asked for a documented one of our own rather than a borrowed list.

**Population.** Two flops that differ only by a relabelling of suits solve to the same
strategy against suit-symmetric ranges, so the population is the suit-canonical flops, not all
22,100 three-card combinations. There are exactly 1,755, and `select_flops.py` proves it by
minimising over all 24 suit permutations; the test asserts the count and asserts that every
one of the 22,100 combinations lands in the canonical set.

**Buckets.** Four texture features, each one something that changes how the spot plays:

| Feature | Values |
| --- | --- |
| `pairing` | `trips`, `paired`, `unpaired` |
| `suits` | `monotone`, `two_tone`, `rainbow` |
| `connectedness` | `connected` (highest minus lowest distinct rank at most 2), `one_gap` (3 or 4), `disconnected` (5 or more) |
| `high_card` | `high` (top card ten or better), `middle` (seven to nine), `low` (six or lower) |

That is 108 combinations on paper and 43 that actually exist. Some are impossible: a paired
flop cannot be monotone, and a flop whose top card is a six cannot span five ranks.

**Sampling.** Every non-empty bucket gets one flop, so no texture is missing from the gate.
The six places left over are handed out in proportion to bucket size, largest fractional
remainder first, with ties broken on bucket size and then on the bucket key so the result
never depends on dictionary order. Inside a bucket the draw is
`random.Random(20260906).sample` over the descending-sorted members, taken in sorted bucket
order.

## Reproducing and checking it

```bash
python tests/reference/flop/select_flops.py            # rewrite flops.json
python tests/reference/flop/select_flops.py --check    # fail if it differs
python -m unittest discover -s tests/reference/flop -p 'test_*.py'
```

No dependencies beyond the standard library, and it takes under four seconds. The `flop-subset`
CI job runs the check on both Windows and Linux, which is what makes "seeded" mean the same
list everywhere rather than the same list on one machine.

`flops.json` records `select_flops_sha256`, the hash of the script that produced it. Editing
the script changes that field, so `--check` fails until the file is regenerated and the new
list is looked at on purpose.

## What this subset is and is not

It is a texture-stratified sample, not a frequency-weighted one. Because coverage comes first,
the crowded buckets are under-represented relative to how often they come up in play: the
largest bucket holds 600 of the 1,755 canonical flops and gets 3 of the 49 places. That is the
right trade for a correctness cross-check, where a texture nobody tested is the real risk, and
the wrong one for anything that averages over the flop distribution. Weight by
`bucket.population` if you ever need the second thing.

Caleb approves the list before it is used in a gate; changing the seed or the buckets changes
the list, so treat both as decisions rather than parameters.
