---
type: schema
status: reviewed-for-implementation
version: 1
date: 2026-09-10
---

# Solved-spot format version 1

This specifies phase 5 step 1 against solver revision `69dc3ff`. Implementation
is in progress. Independent schema review closed four findings: range scaling,
resumed caps, streaming overlap and error maxima. A valid file describes stored
values and source claims; parsing
it does not establish that the claimed game, policy and solve attempt match.
The future capture adapter needs a separate binding proof.

## Scope and scalar representation

Version 1 stores heads-up, no-rake, chip-EV postflop decisions with identity
runout mappings. It supports flop, turn and river starts. Player 0 acts out of
position; player 1 acts in position. Position labels describe the scenario and
cannot alter these roles. A partial policy has no measured exploitability of
its own. Missing coverage and missing EVs remain ungraded.

All binary integers are little endian. Fixed-size numbers have no padding.
Booleans use exactly 0 or 1. Text is UTF-8. A card is `rank * 4 + suit`, with
ranks 2..A indexed 0..12 and suits clubs, diamonds, hearts, spades indexed 0..3.
For card IDs `a < b`, combo ID is `b * (b - 1) / 2 + a`, in 0..1326.
Validate through the cards crate, including duplicate cards.

JSON uses integers for u8/u16/u32/i16 values. All u64 values are canonical decimal
strings, without a sign or leading zeros except `"0"`. F32 and F64 values are
their IEEE bit patterns as exactly 8 or 16 lowercase hexadecimal digits, with
no prefix. Reject nonfinite decoded floats. This avoids decimal parser and
JavaScript integer rounding. Binary float fields contain those same bits.
Typed float wrappers validate field-specific constraints before use.

Original canonical ranges are strings from `Range::to_canonical_string`, in
canonical combo order with exact f64 weights. Reparse and require identical
canonical output; tiny positive weights must survive. Board-blocked entries in
an original range are legal. They cannot be stored as live node combos.

Object fields below are required unless explicitly optional. Unknown or repeated
keys, unknown tags, trailing data and unsupported versions refuse. Optional
fields are present with JSON null; omission is not another encoding. Arrays keep
their order. No layout depends on a Rust enum discriminant or struct size.

## Header

The binary JSON header and JSON export use the same `SpotHeaderV1` object.

| Field | Type and meaning |
|---|---|
| `version` | u16, exactly 1 |
| `spot_id`, `scenario_id` | Nonempty descriptive strings; no path semantics |
| `positions`, `range_labels` | Two nonempty strings each, ordered by player |
| `game` | `GameSpecV1` below, authoritative canonical inputs |
| `coverage` | `{"kind":"start_street"}` or `{"kind":"action_depth","depth":u16}` |
| `payoff_model` | Exactly `"heads_up_chip_ev_no_rake"` |
| `ev_units` | Exactly `"net_chips_from_fixed_root_origin"` |
| `accuracy_units` | Exactly `"nashconv_chips_and_half_as_root_pot_percent"` |
| `uncovered_policy` | Exactly `"ungraded"` |
| `source` | `SourceRecord` below |
| `provenance` | `ProvenanceRecord` below |
| `quantization` | Algorithm `"largest_remainder_u16_scaled_i16_v1"`; F64 `max_probability_error`, `max_ev_error`, `max_reach_error` |

`GameSpecV1` contains `board` (3..5 ordered card IDs), two canonical `ranges`,
positive finite F64 `chips_per_big_blind`, and `tree`. Board length agrees with
the tree's start street. `tree` mirrors the actual `PostflopTreeConfig`:

| Field | Constraint |
|---|---|
| `starting_pot`, `effective_stack`, `min_bet` | u64 chips, at most 1,000,000,000; pot and min bet positive |
| `start_street` | `"flop"`, `"turn"` or `"river"` |
| `sizes` | Three streets by two players, each with ordered `bets` and `raises` arrays |
| `max_raises` | u8, at most 32 |
| `add_all_in_threshold`, `force_all_in_threshold` | Finite nonnegative F64 |
| `max_nodes` | u32, 1..1,000,000 compact public nodes |

A size is `{"kind":"pot","value":F64}`, `{"kind":"previous_bet","value":F64}`,
`{"kind":"additive","chips":u64}`, or `{"kind":"all_in"}`. Pot fractions are
positive. Previous-bet multipliers exceed one and occur only in raise menus.
Additive chips are in 1..1,000,000,000. Each menu has at most 64 entries and
contains no duplicates, matching the checked tree API after deduplication.
Preserve first-occurrence order. All six street/player menus are stored, including
earlier streets. There is no donk-option field in the current tree API.

Nonnegative threshold zero uses positive zero, since its sign changes no rule.
Other finite input bits are preserved. Canonical inputs have no provisional
digest. A future versioned GameId encoding/hash needs its own reviewed contract.

`SourceRecord` fields:

| Field | Type and meaning |
|---|---|
| `producer_run_id` | Nonempty string distinguishing the producer process/run; a claim |
| `attempt` | Null, or `{"solver":u64,"generation":u32}` with both positive; process-local claim |
| `policy_iteration` | u64 completed iteration represented by the source policy |
| `measurement` | Null or the measurement object below |
| `root_values` | Null or `{"iteration":u64,"values":[F64,F64]}` at the policy iteration |
| `stop_reason` | `"target_reached"`, `"iteration_cap"` or `"cancelled"` |
| `target_pct_of_pot` | Finite nonnegative F64 |
| `max_iterations`, `check_every` | Positive u64; a resumed solver may already exceed a newly lowered cap |
| `variant` | `{"kind":"vanilla"}`, `{"kind":"plus"}`, or `{"kind":"discounted","alpha":F64,"beta":F64,"gamma":F64}` |
| `precision` | `"f64"`, `"f32"` or `"i16"`, describing claimed source storage |
| `elapsed_nanoseconds` | u64 monotonic duration for this attempt |
| `root_compatible_mass` | Positive finite F64 denominator for the scaled ranges defined below |

Discount exponents are finite and nonnegative. The source precision tag does not
certify that a producer implements it. `ProvenanceRecord` contains nonempty
`solver_revision`, `solver_version`, `producer_version`, `generated_at_utc`, and
`host` strings. Generation time uses `YYYY-MM-DDTHH:MM:SSZ`, validated as a real
UTC calendar date. It is display metadata. Canonical game inputs provide the
configuration identity; an arbitrary string cannot replace them.

A measurement has u64 `iteration`, two F64 `br_values`, and nonnegative F64
`nash_conv`, `average`, `pct_of_pot`. Its iteration is at most the policy's.
Derive staleness; do not serialize another potentially contradictory flag.
Target/cap reports require a fresh measurement. Target reports require percentage
at or below the stated target; cap reports require policy iteration at least
the cap and percentage above target. Cancellation permits absent, stale or fresh
measurement. Failure is not a solved entry.

Metric consistency follows the existing f64 reporting operations: NashConv is
the nonnegative part of the summed BR values, average is NashConv divided by
two. Percentage is `(nash_conv / starting_pot) * 50.0` in that operation order.
Preserve the original bits;
the implementation must use the same checked conversion and existing numerical
allowance as the source API. Do not introduce a format-specific solver tolerance.
Root values and accuracy remain claims after structural validation.

## Decision records and lookup

Each `NodeRecord` contains u32 `node_id` and `compact_id`; `history`; `board`;
`street`; u8 `player`; ordered `actions`; two u64 `contributions`; positive F32
`ev_scale`; F64 `max_probability_error` and `max_ev_error`; `mapping`; and `combos`.
Each node has 1..255 actions and 1..1326 combos. Only decision nodes are stored.
Node IDs identify expanded nodes, while compact
IDs may repeat on different runouts. Contributions are total commitments since
the solve root, each at most the effective stack.

An action is fold, check, call, bet, raise or all-in, with JSON kind strings
`fold`, `check`, `call`, `bet`, `raise`, `all_in`. The final three have u64
`to` amounts, total commitments since the root. Their amounts are positive
and at most the effective stack. Ordered action lists are unique and follow
the tree API order: fold, check, call, increasing bets, raises and all-ins.

A history contains `{"kind":"action","player":u8,"action":Action}` or
`{"kind":"deal","card":u8}` events. It starts at the root, excludes the node's
own choice, and has at most 128 events. Deals append the next board card in
order. The node board begins with the entire root board, contains no duplicates,
and agrees with the dealt suffix and street. No deal occurs after river.
Full legal-history/tree correspondence belongs to bound capture validation;
structural validation alone cannot establish legal actor order or completeness.

`StartStreet` covers decision nodes on the root street. `ActionDepth(N)` uses
1..128 and covers decisions with fewer than N preceding player-action events;
deals do not count. All stored records must fall within the declared cut.
Capture must establish completeness of the declared cut against its source tree.
Imported structural data carries no completeness certificate.

Lookup requires canonical game equality and a key of history, board, actor,
combo and exact ordered actions. A missing node/combo, unsupported mapping or
outside-cut key returns an explicit uncovered reason. Never select the nearest
node ID or treat a missing record as an empty strategy. Approximate translation
and applicability rules require separate grading work.

`mapping` is `{"representative_node":u32,"suit_permutation":[u8,u8,u8,u8]}`.
The permutation maps original suits to representative suits and must contain
0..3 exactly once. Version 1 accepts identity only, pointing to the same node.
Nonidentity mappings refuse even when bijective; enabling them needs accepted
solver symmetry and action/range/history correspondence.

Each combo has u16 `combo_id`, ordered `probabilities` (u16), ordered `action_evs`
(i16 or null), F32 `own_reach`, F64 `reach_error`, and F64 `opponent_mass`.
Combo IDs are unique and increasing, lie in the acting original range with
positive weight, and do not overlap any board card. Vector lengths equal the
action count. Node IDs and full lookup keys are unique; records sort by node ID.

Probability rows sum to exactly 65535. EV integers exclude -32768; decoded EV
is integer times the stored scale. Reach is in [0,1] and its error is finite
and nonnegative. Opponent mass is finite and nonnegative: compatible opposing range/action/chance
mass, stored losslessly in f64. This additional field prevents own reach from
being mistaken for joint reach. Both zero-own-reach and zero-opponent-mass
decisions have all action EVs missing; a positive reachable row has all present.
Store off-path policy rows if the source has them, preserving missing EVs.

Quantization uses the exact algorithm in `docs/astra/phase5-format/PLAN.md`.
The source row must be finite and normalized within its specified summation
allowance. Measure error against the original row, with per-action probability
error at most 1/65535 and EV error at most half the stored scale. Nonzero f32
scale or reach underflow refuses. Store measured reach error per combo; the node's
reach maximum is derived from its combo errors, without another wire field.
Header maxima equal maxima over contained nodes. Empty spots have zero maxima.
Probability errors are nonnegative and at most 1/65535; node EV errors are
nonnegative and at most half that node's scale. Reach errors are at most 2^-25,
the maximum nearest-f32 error on [0,1]. On import, bounds are claims checked for internal
consistency; the unavailable original source values cannot be reconstructed.

Consumers treat the true own reach as lying within stored reach plus/minus its
recorded error, intersected with [0,1]. If applicability changes within that
interval, it remains unresolved. Tiny positive opponent mass is never rounded
to zero. Any joint-reach calculation uses scaled compatible root mass and checked
arithmetic; the format does not define Decision 14 thresholds.

Reach conventions match `postflop/src/ranges.rs` and the strategy query API.
First remove root-board-blocked combos from each original range, then divide
each side by its own surviving maximum. Both maxima must be positive. Own reach
is this scaled inclusion weight times that player's preceding action factors
and later-board masks, without chance factors. Opponent mass sums compatible
opposing scaled weights times their action factors, later-board masks and the
path's chance factors. Chance factors are 1/45 on flop-to-turn and 1/44 on
turn-to-river, conditional on both private hands.

The root denominator is the sum of scaled OOP weight times scaled IP weight
over compatible pairs on the original board. `root_compatible_mass` preserves
the solver's evaluated f64 denominator as a source claim. Bound capture checks
it against `PostflopGame::compatible_weight()` and the canonical inputs using
the existing range arithmetic. Positive compatible products that underflow
refuse; empty compatible mass refuses. Structural import must validate this
range compatibility within a charged, bounded workspace.

Normalized joint reach for a combo is own reach times compatible opponent mass
divided by that denominator. Using unscaled original weights in the denominator
would change applicability: two uniformly half-weighted ranges give a fourfold
error. Tests must include that case and a blocked original maximum.

## Binary framing

The file contains, in order:

| Bytes | Field |
|---|---|
| 4 | ASCII `GTOS` |
| 2 | Version, exactly 1 |
| 2 | Flags, exactly zero |
| 4 | JSON header byte length |
| 4 | Node count |
| 8 | Total encoded node-section byte length, including each record length |
| header length | UTF-8 header JSON, version agrees with prefix |
| section length | Node records, each prefixed by a u32 payload byte length |
| 4 | CRC32 of every preceding byte, including prefix and lengths |

CRC32 uses the IEEE reflected polynomial 0xEDB88320, initial value 0xFFFFFFFF,
and final XOR 0xFFFFFFFF. `123456789` gives 0xCBF43926. It detects corruption;
it provides no source authentication. No compression or trailing bytes in v1.

A node payload begins with u32 node ID, u32 compact ID, u8 street
(0 flop, 1 turn, 2 river), u8 player, u8 board count and that many card bytes.
Next are u16 history count and events, u8 action count and actions, then two
u64 contributions. These are followed by f32 EV scale, two f64 error maxima,
u32 representative node and four suit bytes. Last are u16 combo count and records.

Each action is a u8 tag (0 fold, 1 check, 2 call, 3 bet, 4 raise, 5 all-in),
followed only for tags 3..5 by its u64 total. A history action is tag 0, player
byte, then that action encoding. A deal is tag 1 and a card byte.
Each combo is u16 ID, f32 own reach, f64 reach error, f64 opponent mass, then
`action_count` pairs of u16 probability and i16 EV. Binary EV -32768 means
missing, never a numeric EV; JSON represents the same missing value as null.

JSON export is `{"header":SpotHeaderV1,"nodes":[NodeRecord,...]}`. It contains
the same quantized values and ordering as binary. Re-encoding a decoded spot
must preserve every scalar bit and missing value. Canonical exporters use the
field order above and compact JSON without insignificant whitespace. Importers
may accept different object-key order and whitespace within all byte limits.

## Resource limits and ownership

Hard byte ceilings are 4 GiB encoded bytes, 1 MiB header and 16 MiB per encoded
node. Count ceilings are 4,000,000 nodes, 1326 combos and 255 actions per node,
128 history events and 64 entries per menu. Strings permit 131,072 bytes per
range and 4096 bytes otherwise. JSON nesting is at most 32. Limits count UTF-8 bytes after unescaping
as well as total input bytes. They are ceilings, never allocation permissions.

Every importer/builder/exporter requires explicit caller-provided ResourceLimits:
live bytes, retained bytes, encoded bytes, header/node bytes, nesting, nodes,
combos, action entries and string bytes. Counters use checked u64 arithmetic,
then checked usize conversion before allocation. No multi-gigabyte default.
Aggregate action entries count menu entries, node actions, history actions and
each combo's probability/EV pair; aggregates never reset between nodes.

A caller-owned budget issues leases before allocation. Charge actual Vec/String
capacity and element/header storage, indexes and parser/encoder scratch.
Growth reserves old/new allocation overlap. If an allocator returns more
capacity than requested, keep the old reservation and refuse publication until
the extra capacity is charged. Payloads drop before leases on success and error,
including function-argument ownership and partially constructed collections.
No unrestricted clone may allocate an uncharged copy.

The builder accepts borrowed records, validates bounded counts, reserves before
copying, and retains private storage. Its immutable finished type means
structurally validated data, not trusted provenance. Binary and JSON use the
same insertion/validation logic. Whole-object derived Deserialize or a complete
serde_json::Value followed by checks does not satisfy preallocation limits.
Bounded visitors must also cover the binary JSON header and duplicate-key checks.

Before reserving collections, verify counts against resource limits, remaining
input, and checked minimum encoded lengths. Bound validation indexes; sorted
indexes avoid an unpriced hash map. Borrowed input still counts toward the
caller's total working set. Both complete and streaming capture account for
retained source snapshots, input and query overlap within the application budget.
Streaming reduces the additional capture allocation; it does not bypass that
aggregate limit.

## Acceptance and open implementation gates

Tests need canonical tiny ranges, exact binary/JSON round trips, float boundary
bits, missing values, staleness cases, later-runout blockers, duplicate lookup
keys, invalid mappings and malformed length/count/nesting cases. Allocation
probes must include spare capacity, growth overlap and refusal cleanup order.
Mutation tests must refuse corrupt files within the caller's budget.

The source binding API, GameId encoding/hash, full capture, codecs and bounded
builder are not implemented by this document. It does not approve SQLite,
production scenario/coverage choices, a runner install, numerical thresholds,
or the final library. The required 25-flop and session-coverage gates remain.
