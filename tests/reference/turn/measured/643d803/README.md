# The turn gate, measured

`record.json` is the reading, not the instrument. Project commit
`643d803e35263820219be90a5eb61f945b92fd5e`,
[CI run 34474380677](https://github.com/calebroot2006-art/SOLVER/actions/runs/34474380677),
both operating systems, against the pinned WASM reference from the same run.

The two project captures are 65,101,178 and 65,101,159 bytes and the two comparison
reports are 18 MB each. None of that is committed. The record carries the convergence
per case and host, the root expected values, the peak resident set, the three timings,
the review's counts, and every row the review rule would not excuse. It also carries the
artifact ids, so the raw files can still be fetched while the run's retention lasts. The
river's `measured/` folders keep whole captures because a river capture is 440 KB.

## What it says

Every case reached its own 0.25%-of-pot target on both hosts, at the same iteration and
the same exploitability to every digit `record.json` prints:

| Case | Iterations | Project | Reference | Root EV difference |
| --- | --- | --- | --- | --- |
| `turn_100bb_dry_rainbow` | 150 | 0.20821% of pot | 0.15869% | 0.00337 chips |
| `turn_100bb_paired` | 200 | 0.16611% | 0.17830% | 0.00091 chips |
| `turn_100bb_flush_possible` | 150 | 0.20755% | 0.18510% | 0.00067 chips |

The pot is 11 chips. Peak resident set was 347,312,128 bytes on Linux and 298,860,544
on Windows, against an estimate of 377,933,341 at four workers. Both runners reported
four CPUs and about 16 GB. That estimate is the one this commit computed: the node-value
report became a row of its own afterwards, which adds 85,376 bytes to every bound, so the
same tree priced today reports 378,018,717.

Of the rows that differ by more than two percentage points, the review rule
(`review_rules.json`, thresholds unconfirmed) calls 10,998 / 17,029 / 9,913 indifferent,
32,193 / 35,561 / 32,227 unreached, and 351 / 189 / 277 a real gap. The real gaps cost
4.10e-05, 1.35e-05 and 1.32e-05 of the pot, against a budget of 5.0e-03. The unreached
rows' own bounds sum to 9.62e-05, 4.96e-05 and 1.04e-04. Both hosts produce the same
counts and the same sums.

## Regenerating it

Download the run's three artifacts, run the gate once per operating system, then write
the record. `<sha>` is the commit the captures name, and the folder is its first seven
characters.

```bash
python docs/astra/development-takeover/ci_status.py artifacts <run> > artifacts.json
python docs/astra/development-takeover/ci_status.py jobs <run> > jobs.json
# download turn-project-ubuntu-latest, turn-project-windows-latest and
# turn-wasm-reference by the ids in artifacts.json, with
# python docs/astra/development-takeover/ci_status.py download <id> <file.zip>

for os in ubuntu-latest windows-latest; do
  python tests/reference/turn/compare.py \
    --project "target/turn-project-$os/cases.toml" \
    --reference target/turn-reference/cases.json \
    --review tests/reference/turn/per-combo-review.json \
    --expected-revision <sha> \
    --output "target/turn-comparison/$os.json"
done

python tests/reference/turn/measured_record.py \
  --run <run> --commit <sha> \
  --project ubuntu-latest=target/turn-project-ubuntu-latest/cases.toml \
  --project windows-latest=target/turn-project-windows-latest/cases.toml \
  --reference target/turn-reference/cases.json \
  --report ubuntu-latest=target/turn-comparison/ubuntu-latest.json \
  --report windows-latest=target/turn-comparison/windows-latest.json \
  --rss ubuntu-latest=target/turn-project-ubuntu-latest/linux-time.txt \
  --rss windows-latest=target/turn-project-windows-latest/windows-peak-rss.txt \
  --artifacts artifacts.json --jobs jobs.json \
  --output tests/reference/turn/measured/<short sha>
```

The record is a fact about one run, so a later run gets a new folder rather than an edit
to this one.
