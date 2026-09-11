# Pure quantization kernel

Executor branch: `spots/astra-phase5-quantization`, base `0e098e4`.
Scope: `crates/spots/src/quantize.rs`, its module export and this note.
This implements only the numerical kernel specified in `PLAN.md`. It adds no
capture adapter, codecs, dependencies or persisted identity.

## API and allocation ownership

`quantize_probabilities(&[f64], &mut [u16]) -> Result<f64, QuantizeError>`
returns measured maximum absolute error. Input/output lengths must agree and
have 1..255 entries. Its fixed `ProbabilityScratch` contains 255 u16 codes and
255 f64 remainders. `PROBABILITY_SCRATCH_BYTES` exposes the scratch object's
size, including padding, for future budget accounting; ordinary scalar frame
state is additional. The scratch object is 2,552 bytes on this Windows target.
Largest-remainder selection takes at most 255 scans of 255 entries.

`quantize_evs(&[Option<f64>], &mut [Option<i16>]) ->
Result<EvQuantization, QuantizeError>` returns stored f32 scale and measured
maximum absolute error. Blocks have at most 338,130 entries (1326 * 255).
Separate passes validate values, select scale and verify every decoded error
before writing output. EV scratch is scalar; there is no node-sized temporary.

`quantize_reach(f64) -> Result<QuantizedReach, QuantizeError>` returns the f32
value and absolute error by value. It owns no buffers. Zero reach remains a
present number; it does not represent a missing EV.

Production code allocates no heap storage. Both slice APIs leave output
unchanged on every error, and perform no fallible operation after the first
write. Callers own and must charge input/output storage plus scratch. Errors
are a Copy enum with static Display messages, without owned strings.

## Exact numerical choices

Probabilities must be finite and in [0,1]. The only accepted normalization
allowance is `8 * f64::EPSILON * action_count`. Inputs are not renormalized.
The kernel floors `input * 65535`, assigns remaining units by descending
fractional remainder and breaks equal remainders by original action order.
Before copying scratch to output, it verifies the exact integer sum of 65535
and each decoded error against the original input, bounded by `1 / 65535`.
The returned error is the largest of those measured differences.

Present EV values must be finite. A nonzero block uses the smallest positive
finite f32 scale covering `max_abs / 32767`. After casting the quotient, the
kernel compares `scale * 32767` to max_abs and advances one f32 value if needed.
That product is exact in f64, since f32's significand and the 15-bit multiplier
fit within f64 precision. Scale overflow or a nonzero scale casting to zero
returns a named error. Division underflow from a nonzero max_abs also refuses.

Codes round to nearest with ties away from zero and must stay in
[-32767,32767]; -32768 is never emitted. There is no clamp. Every decoded EV
must differ from its source by at most half the stored scale, with no epsilon
added to that bound. Missing values stay missing. Individual small EVs may
round to zero within this error bound; scale-cast underflow is the refused case.

API choice for Astra's review: empty EV blocks are accepted as all-missing.
Empty, all-missing and all-zero blocks use scale one and error zero. Signed EV
zero becomes integer zero; optional presence is preserved. Probability rows
remain nonempty as specified. Reach preserves signed zero through the cast.
Reach must be finite and in [0,1]; positive-to-zero f32 conversion refuses.
Every reported error describes representation, not solver accuracy.

## Verification

Nine spots tests pass, including the pre-existing crate-name test. Numerical
coverage includes known largest-remainder answers and stable ties, 255 actions,
1,020 deterministic generated probability rows, exact row totals and original
input error measurements. Summation tests accept the stated boundary and refuse
the next representable test increment outside it. Failed calls retain sentinel
outputs, including failures discovered after a valid first EV.

EV tests cover all 338,130 permitted entries, missing/zero/empty blocks, signed
endpoints, half-step ties and their adjacent f64 values across six f32 scales.
They cover minimum subnormal, minimum normal and maximum finite scales; scale
rounding between adjacent f32 values; values just beyond maximum scale; and
nonzero-to-zero underflow. Reach tests check measured conversion error, signed
zero, subnormal results, invalid values and positive underflow.

Commands: `cargo test -p spots --locked --offline`; `cargo clippy -p spots
--all-targets --locked --offline -- -D warnings`; `cargo fmt --all -- --check`;
`git diff --check`; and the repository prose checker for this note. Checks run
locally on Windows. No capture or persisted-format acceptance is claimed;
Astra's independent numerical and allocation review precedes integration.
