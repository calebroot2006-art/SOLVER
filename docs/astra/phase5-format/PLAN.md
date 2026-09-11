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

The read-only schema proposal is reviewed. Astra is specifying the wire model
and resource accounting before the builder and codecs. The independent numerical
kernel below can proceed without a capture adapter or persisted identity.

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

Specify every header and record field and stable tag, coverage semantics,
binary framing and caller-provided ResourceLimits. Implement the bounded builder
with leases that survive until payload deallocation, then the codecs. Test
capacity/growth/error accounting and hostile lengths before calling step 1 done.
Keep the final 25-flop and representative-session coverage gates intact.
