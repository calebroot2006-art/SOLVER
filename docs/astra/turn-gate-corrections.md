# Turn acceptance corrections, 2026-09-10

B1/B2/B3 are corrected on `solver/astra-turn-gate-corrections`, based on `2f81339`.
No Rust, Decision 14 candidate, or committed capture/review record changed.

- Root summaries now require dimensions, finiteness, zero-sum consistency, the
  captured root policy/action-EV mixture, display-origin agreement, and the
  project's own best-response interval. Cross-solver agreement uses the sum of
  measured NashConv gaps plus the documented arithmetic allowance.
- Every C row is walked again from the current capture. Stored oracle values
  remain review evidence; they cannot substitute for the current walk.
- Before classification, project reach and compatible mass are reconstructed
  from ranges, policies, blockers and chance. EV availability follows the
  producer's decision/chance distinction. Reference checks enclose the pinned
  wrapper's decimal rounding, truncated empty-range flags and f32 arithmetic.

The numerical allowances and source links are in
[the turn README](../../tests/reference/turn/README.md). The reference display can
hide a tiny positive policy as zero, or withhold EV after rounding its compatible
joint weight to zero. Exact equality against displayed reference reach would
reject thousands of legitimate rows. The implementation checks explicit intervals;
it does not loosen the project check or the candidate B threshold.

The synthetic fixture now reports consistent root summaries and path reach. Its
one-chip action gaps remain deliberate inputs to isolated classification tests.
Nineteen new regressions use the shipped rules and cover the five accepted bad
mutations, malformed roots, BR interval violations, false mass, missing reference
EV, genuine zero-action reach, runout/private-card blockers, range scaling, f64 underflow and f32 presentation.
The original C-row controls used synthetic all-in values. The all-in closure
below replaces them with physical payoffs while preserving the mutation amounts
and the one-chip classification gap.

Validation:

- `python -B -m unittest discover -s tests/reference/turn -p 'test_*.py'`:
  163 passed. Black, Ruff, the input-contract check and the prose checker passed. The sandbox refused writes inside newly created temporary test
  folders; the approved run used `target/python-tmp` and passed.
- Both downloaded `e338d7a` OS captures accepted with zero gate failures, missing
  rows or stale rows, using the unchanged committed review and shipped rules.
  A/B/C counts stayed 10,998/32,193/351, 17,029/35,561/189 and 9,913/32,227/277.
  The final complete gate took 37.17 seconds for Linux and 40.36 for Windows, excluding
  15.40/16.91 seconds of TOML parsing. Every OS pass recomputed all 817 C
  rows; maximum error was `6.856737400084967e-13` chips. Summaries are retained in this checkout's
  ignored `target/linux-gate-summary.json` and `target/windows-gate-summary.json`.

The 117 oracle rows using reported turn chance values remain dependent on those
leaf values. Called-turn-all-in numerical coverage was completed in the separate
follow-up below. A corrected gate is not an independent complete turn solve, and this branch
is neither merged nor pushed by the executor. Astra still owns final acceptance.

## Follow-up: tiny compatible mass

Astra's review of `4aec013` found that its absolute mass allowance could suppress
material normalized reach. The direct evidence-unit reproduction uses board
`Ac Ad Kh Qh` and both ranges `AA,KK:0.000000000000001`. Their dominant `AhAs`
combos block each other. Root compatible mass is `6e-15`; `AhAs` has opponent
mass `3e-15` and normalized reach `0.5`. A forged zero passed the former evidence
check and changed a C classification to B. The new tests also cover forged
`1e-100` and `1e-22` masses.

`opposing_mass` multiplies each opponent reach by chance, then `evaluate_fold`
uses `ExactMass`/`Bucket::compatible` to sum the compatible weights exactly and
round once to f64. The comparator now mirrors that multiplication order and
uses relative tolerance `1e-10` with no absolute floor for own reach, opponent
mass or the root normalizer. Root compatible mass is summed with `math.fsum`;
positive pair-product underflow is refused. Classification and review generation
normalize with the reconstructed root mass, even if the parsed capture is changed
later. No candidate threshold changed.

Three added regressions cover the demonstrated bypass, root-mass scale and the
normalizer used by classification. The complete Python suite now passes 166 tests.

Both complete OS gates passed again after this correction, with zero failures,
missing rows or stale rows. Each recomputed all 817 C rows; maximum error remained
`6.856737400084967e-13` chips and A/B/C counts stayed unchanged. Gate runtimes were
39.84 seconds for Linux and 38.74 for Windows, excluding parsing. Black, Ruff,
input validation and the prose checker also passed.

## Follow-up: called-turn-all-in numerical coverage

The gate independently enumerates 44 rivers per compatible private pair at every
called turn all-in, then reconstructs each capture's conditional hand value using
its own opponent path policy. It checks both the project's chance-parent export
and its supported turn-showdown terminal fallback. Own action reach does not
suppress a project conditional EV. Reference availability is checked against
rounded path/normalized-mass intervals before absent values are accepted.

The project check retains `1e-9` chips. Reference intervals combine the actual
rounded-policy uncertainty with pinned uncompressed f32 finalization, blocker-mass,
normalization and display arithmetic; the derivation and primary source links are
in the README. No captured EV supplies an expected value. Tables cache only board
and physical hands, with three bounded dense tables and the existing bounded rank
cache. Selected river children receive structural checks; their captures contain
no terminal EV rows, so this makes no claim about unavailable terminal values.

Eighteen new tests include known quads wins, Broadway ties, a 41-win/3-tie blocker
example and exact extrema against every weight-box corner. They cover both exporter
formats, mutated project/reference parent EVs, independent opposing policies, zero
own reach, absent EVs and malformed river sets/children. The shared fixture's sets
beat QQ/JJ on all 44 rivers: both called all-ins now report `+45/-45`. Changing the
IP response to OOP's jam to fold `0.7875`, call `0.2125` makes root action values
`[12.5, 13.5]`. The C control uses `[0.6, 0.4]` on one hand and retains the one-chip
gap. The original +100 chance mutation now gives `[62.5, 13.5]`; the original
1-point downstream policy mutation gives `[12.25, 13.5]`. Both remain rejected.

Both real step 6 OS gates pass with zero failures, missing rows or stale rows and
unchanged A/B/C counts. Per OS, coverage is six histories, 577,025 unique compatible
pairs, 25,389,100 unique pair-river evaluations and 5,546 checked project parent
EVs. Worst project error is `1.7053025658242404e-13` chips. The reference exposes
313 values and withholds 5,233 under its verified producer semantics. Worst
nominal error is `0.004964802468379048` chips; no value exceeds its independent
interval. The widest reference interval is about 65.87 chips on a sparse dry-board
hand: rounded-away opponent policies cannot establish a tight conditional EV.
That limitation is reported, rather than converted into an empirical tolerance.
All 817 current C rows still agree within `6.856737400084967e-13` chips.

The final complete gates took 37.04 seconds for Linux and 28.86 for Windows,
excluding 10.01/10.30 seconds parsing. The first pass builds all pair tables; the
second reuses them. Compared with the previous 39.84/38.74-second passes, these
wall times show the additional check is practical, not a controlled speedup.
The largest reference arithmetic allowance is `0.000232` chips; the wide intervals
come from policy rounding (`65.8666` chips at worst), not arithmetic tolerance.
All 184 Python tests, Black, Ruff, input validation and the prose checker pass.
The ignored gate summaries retain the executed source hashes and full counts.
The raw-mode test tag was then corrected to the supported `raw_f32` spelling; all
18 all-in tests passed again after that test-only edit.

### Arithmetic derivation and remaining raw-display limit

The pinned source is `b-inary/postflop-solver` revision
`9d1509fe5077d019825f833eed04b16d342dfda1`. These are the relevant paths and lines:

- [game/base.rs, 405-418](https://github.com/b-inary/postflop-solver/blob/9d1509fe5077d019825f833eed04b16d342dfda1/src/game/base.rs#L405):
  root compatible mass accumulates products of initial f32 weights in f64.
- [utility.rs, 351-402](https://github.com/b-inary/postflop-solver/blob/9d1509fe5077d019825f833eed04b16d342dfda1/src/utility.rs#L351):
  chance reciprocal, f32 reach scaling, f64 accumulation and final f32 cast.
- [game/evaluation.rs, 85-135](https://github.com/b-inary/postflop-solver/blob/9d1509fe5077d019825f833eed04b16d342dfda1/src/game/evaluation.rs#L85):
  separate winning/losing f64 blocker sums, two f32 payoff casts and their f32 sum.
- [game/interpreter.rs, 403-462](https://github.com/b-inary/postflop-solver/blob/9d1509fe5077d019825f833eed04b16d342dfda1/src/game/interpreter.rs#L403):
  total/card-bin f64 sums and inclusion/exclusion, f32 mass cast and own-reach product.
- [game/interpreter.rs, 659-735](https://github.com/b-inary/postflop-solver/blob/9d1509fe5077d019825f833eed04b16d342dfda1/src/game/interpreter.rs#L659):
  normalizer cast, own/normalized division, two scale products, integer-origin casts
  and two origin/display additions. The pot's multiplication by one half is exact.
- [wasm wrapper lib.rs, 43-56](https://github.com/b-inary/wasm-postflop/blob/97360db7644329b1c23a7adf06e9aa59406e4d4b/rust/solver-src/lib.rs#L43):
  the decimal quantum. Its [results path, 297 onward](https://github.com/b-inary/wasm-postflop/blob/97360db7644329b1c23a7adf06e9aa59406e4d4b/rust/solver-src/lib.rs#L297)
  supplies the truncated-range/normalized-weight availability semantics.

Let `N` be opposing hand count, `S` its upper total reach, `M` compatible reach,
`H = starting_pot/2 + stack`, `Z` root compatible mass, `u = 2^-24`, and
`eta = 2^-149`. The 16 normal f32 operations listed above give
`gamma16 = 16*u/(1-16*u)`, applied to absolute positive/negative payoff mass.
This avoids relying on relative error of a near-zero signed EV.

For f64 blocker arithmetic, `D = 4*N*2^-52*S` bounds three sums plus their
inclusion/exclusion. For `N >= 2`, the first-order coefficient is at most
`3*(N-1)+12 <= 8*N` in f64 unit-roundoff units; the finite gamma denominator
fits the remaining slack. The one-hand sums/subtractions are exact. Across at
most 48 public children, both showdown passes contribute less than
`2*(48/44)*D` after chance scaling. Normalization adds one `D`; payoff arithmetic
and up to 48 f64 chance additions add less than seven more `D`. Thus `16*D`
covers their sum and subsequent f32 propagation. The sum uses all opposing
hands because subtracting blocker bins can lose compatible mass.

Each chance-weight product has absolute rounding error at most `eta/2`. Its
payoff contribution over at most 48 children is at most `24*N*H*eta` before
division by compatible mass. The implemented `64*N*H*eta` also encloses subsequent
propagation, whose relative factor is below two. Each river has two payoff casts
and one addition, contributing at most `3*eta/2` in CFV units. Forty-eight rivers
plus the final chance cast give `72.5*eta`; the implemented `100*Z*eta` encloses
that contribution after conversion from CFV units and propagation. `Z` uses
an upper f64 accumulation bound over reconstructed f32 initial-weight products.
The own/normalized product's absolute rounding gets its separate `H*eta/Jmin`
term. Display addition and the wrapper's half decimal quantum are also explicit.
These constants come from operation counts; they were not fitted to the captures.

The weight-box threshold argument gives mathematical extrema. Their f64
products, prefix sums and ratio require an outward allowance: the implementation
uses `gamma(4*N+8)*max_abs_pair_payoff`, then `nextafter` in each direction.
The operation bound includes weight differences, products, both prefix sums and
division; nonnegative accumulated weights keep the denominator from cancellation.

The relative normalization part requires normal f32 normalizer, compatible
mass, normalized mass and scale, and finite upper CFV/scale bounds. The gate now
refuses raw subnormal normalization
when those lower bounds cannot be proved; it never accepts an infinite interval.
The current display captures imply normalized mass at least `0.5e-6` and satisfy
the domain. A raw tiny-joint regression proves the named refusal; another proves
that a rounded empty range cannot forge an available EV. Extending the proof to
all raw subnormal normalization remains outside this checked result.
