# Eleven no-limit Hold'em PHH fixtures

These are the eleven no-limit Hold'em hands from the 83-hand mixed-game PPC
fixture set, corresponding to published hands 1-4 and 62-68. They are data for
the phase 6 replay gate. The engine has not replayed them.

The source is [uoftcprg/phh-dataset](https://github.com/uoftcprg/phh-dataset/tree/e47fbd5816372360bade4de5d712346fe1bb70f6/data/wsop/2023/43/5),
revision `e47fbd5816372360bade4de5d712346fe1bb70f6`, directory
`data/wsop/2023/43/5`. The local path is `wsop/2023/43/5/` below this directory.

Before any fixture was copied, the executor read the
[pinned MIT license](https://github.com/uoftcprg/phh-dataset/blob/e47fbd5816372360bade4de5d712346fe1bb70f6/LICENSE):
Copyright (c) 2024-2025 Universal, Open, Free, and Transparent Computer Poker
Research Group. The full notice is preserved in [LICENSE](LICENSE), unchanged.

License bytes: 1130. License SHA-256:
`fd8a94019f50ff8cd84e674acd790427882a99b32435c85e43b8e1d5b37626d1`.

Each exact source URL and SHA-256 is recorded in [inventory.json](inventory.json),
copied unchanged from `docs/reviews/2026-09-10-step6-review/phh-inventory.json`.
The eleven files matched those previously recorded hashes before they were
written here. The download allowed at most 64 KiB per fixture, 16 KiB for the
license, a 25-second timeout per request and three concurrent requests. It
rejected redirects and checked destination paths before writing. No upstream
Python or poker engine code was copied or executed.

Inventory SHA-256:
`ff16afed7d58f962a2c193cfeb4b0e3affab40df44b0404d5cde404144f48a8c`.
The local `.gitattributes` preserves the source files, license and inventory
without line-ending conversion.

## Verification

From the repository root, using Python 3.11 or newer:

```text
python crates/engine/tests/fixtures/verify_inventory.py
```

The verifier uses the standard library, reads only local files and checks the
pinned license, inventory, source URLs, filenames and hashes before inspecting
the PHH fields. It rejects missing, additional, oversized or changed fixtures.
It also verifies complete known private/board cards without duplicate deals,
five positional players, integer stacks and action amounts, recorded chip
conservation, action counts and the exact show actions in the inventory.
The action checks inspect the recorded syntax; they do not establish legality.

| File | Published hand | Bytes | Actions | Board cards | Show actions |
| --- | ---: | ---: | ---: | ---: | ---: |
| `00-02-07.phh` | 1 | 895 | 24 | 5 | 2 |
| `00-08-38.phh` | 2 | 767 | 13 | 0 | 0 |
| `00-15-36.phh` | 3 | 858 | 21 | 5 | 0 |
| `00-18-39.phh` | 4 | 813 | 18 | 4 | 0 |
| `02-51-10.phh` | 62 | 729 | 10 | 0 | 0 |
| `02-53-09.phh` | 63 | 729 | 10 | 0 | 0 |
| `02-54-12.phh` | 64 | 746 | 11 | 0 | 0 |
| `02-56-12.phh` | 65 | 746 | 11 | 0 | 0 |
| `02-57-27.phh` | 66 | 778 | 14 | 3 | 0 |
| `03-00-32.phh` | 67 | 738 | 11 | 0 | 0 |
| `03-02-41.phh` | 68 | 806 | 16 | 5 | 2 |

The corpus contains 8,605 bytes and 159 action strings. All eleven hands use
`NT`, five players, `ante_trimming_status = false` and a big-blind ante of
1.5 big blinds. Four actions explicitly show cards; two occur before the
remaining board is dealt in hand 68. Private deal records describe complete
hidden state. They do not grant an observer permission to see those cards.

These files supply the eleven supported no-limit Hold'em records from the
published mixed-game corpus. They do not provide 83 Hold'em hands or seat-count
coverage beyond five players. Engine replay, legal actions after each event,
commitments, pot awards, independently reproduced finishing stacks and observer
privacy remain unverified. Separate fixtures and the phase 6 rule decisions are
still needed for heads-up and six-to-nine-player games, short posts, rake,
straddles and dead blinds.
