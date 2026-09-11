---
type: plan
status: in-progress
date: 2026-09-10
---

# Phase 5 format foundation

Base: `69dc3ff`, isolated branch `spots/astra-phase5-format`. The amended phase 5
plan permits schema, bounded types, codecs and synthetic tests. Source capture,
SQLite and production library acceptance retain their separate gates.

## Progress

The wire schema is specified in `docs/phase-5/spot-format.md`. Independent review
identified reach scaling, resumed iteration caps, streaming budget overlap and
error-maxima inconsistencies; Astra amended all four before implementation.
The numerical kernel at `71aec4a` is independently reviewed. Astra read the full
source and tests and ran all nine spots tests successfully. It allocates no heap,
retains strict source-error bounds and preserves failed outputs. The builder and
codecs remain outstanding; this does not complete phase 5 step 1 or capture.

## Decisions and ownership

Astra owns this plan and `docs/phase-5/spot-format.md`. The schema records
untrusted source claims; structural validation does not certify a capture.
Canonical game inputs remain authoritative, with no provisional game hash.
The actual tree configuration has typed street/player menus and no donk option.
Unknown versions and unsupported nonidentity runout mappings refuse.

A separate quantization executor may own `crates/spots/src/quantize.rs`, a module
export in `src/lib.rs`, and `docs/astra/phase5-format/quantization.md` in its own
worktree. It adds no dependencies, capture adapter or codecs. Astra reviews its
entire diff and tests the numerical edge cases before integrating it.

The kernel consumes borrowed input and caller-owned output. It allocates no heap
storage, uses bounded stack scratch, and preserves outputs on failure. Future
callers must charge input, output and scratch to their aggregate budget.

Probability rows have 1..255 finite entries in [0,1]. Accept only summation noise
within `8 * f64::EPSILON * action_count` of one. Use largest remainder with
original action order breaking ties. Refuse unless the integer sum is 65535 and
each decoded value differs from its original input by at most 1/65535. No
materially unnormalized row may be silently repaired.

Optional EV entries preserve missing values. Values must be finite. A node has
at most 1326 * 255 entries. Choose the smallest positive finite f32 scale at least
`max_abs / 32767`, rounded upward when needed; all-zero or all-missing uses one.
Refuse scale overflow or nonzero cast underflow. Quantize to [-32767,32767],
nearest with ties away from zero, with no clamp. Verify each decoded value's
absolute error is at most half the stored scale before writing any output.

Reach is finite and in [0,1]. Store f32 plus the absolute error against the input.
Reject a positive input that casts to zero. Missing EVs and zero reach are
different states. These errors describe representation, not solver accuracy.

## Verification

Meaningful tests cover exact row sum, stable ties, 255 actions, unnormalized and
nonfinite rows, extreme f32 scales, signed EV endpoints, missing/zero values,
half-step ties, underflow, output preservation and deterministic generated rows.
Run spots tests, Clippy with warnings denied, formatting and prose checks.
Schema and allocation-path review remain separate from these pure kernel tests.

## Remaining work

Implement the specified bounded builder and ResourceLimits
with leases that survive until payload deallocation, then the codecs. Test
capacity/growth/error accounting and hostile lengths before calling step 1 done.
Keep the final 25-flop and representative-session coverage gates intact.

## Builder implementation contract

After schema review, one isolated executor may implement `crates/spots/src/format.rs`
and `src/resource.rs`, tests and its README. Use only existing approved workspace
crates as needed (cards, tree, serde and serde_json are already available); this
step requires no codec or new external dependency. A path-dependency lockfile
change must contain only the dependency edges actually added to spots.

Implement all specified header/record types and a private-storage SpotBuilder
whose finished ValidatedSpot exposes borrowed accessors. Typed caller input may
be borrowed from DTOs; do not imply caller-created input was allocated by this
builder. Input ownership and its charge remain the caller's responsibility.
No public whole-Spot Deserialize or unrestricted owning clone. Codecs will use
the same bounded insertion API later.

ResourceLimits has explicit values for every schema resource and no default.
A reusable caller-owned memory budget covers concurrently retained results,
builder scratch and explicit external reservations. Reserve before builder
allocations, count capacities and growth overlap, and free payloads before
releasing their leases. Keep retained and peak/live byte limits distinct.
Count every node/combo/action/string cumulatively; failed insertion must not
leave a partial record or consume counts. Finite-value, version and collection
validation precede copying. Invalid input yields named errors without panics.
Use fallible reservation for growable collections; bound all validation indexes.

Range parsing/canonical output and compatibility work are themselves charged
scratch. Validate without constructing an expanded betting tree. Source claims
remain untrusted; no fake capture binding or game digest. Validate the source
metric operation order from the schema and existing checked conversion, allowing
resumed iteration counts above a lowered cap. Preserve bit-exact valid scalar
values apart from the schema's explicitly canonicalized threshold zero.

Meaningful tests cover resource boundary/refusal, cumulative counts, arithmetic
overflow, spare capacity and growth/error/drop order; duplicate IDs/full keys/
combos; missing/blocked/off-path EV semantics; malformed mappings; canonical
tiny ranges and scaled root mass; metadata staleness and resumed cap behavior.
Use a deterministic allocator or equivalent drop observation for reservation
lifetime, not only an after-return used-byte assertion. Run spots tests, Clippy,
formatting and prose checks; save locally for Astra's full independent review.
