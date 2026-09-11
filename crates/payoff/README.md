# Payoff

`ChipEv` computes net chips as `pot_share * total_contributions - own_contribution`.
The shared `Real` alias is `f64`. This crate has no dependencies.

Use `ChipEv::try_utilities` when constructing or validating a terminal. It rejects
length mismatches, non-finite or negative inputs, invalid shares, and pot overflow
before changing the output. The `Payoff::utilities` hook has no error return;
its ChipEv implementation fills the output with NaN on invalid input. The solver
detects that and returns a contextual error.

`stacks_before` is validated but does not change chip EV. This interface certifies
chip payoffs only. Rake, side-pot allocation and bounty rules need their own
reviewed inputs and metrics; a single pot-share vector is not a complete
tournament model.

`PayoutStructure::new` validates an explicit vector of finishing-place payouts,
from first place onward. It requires at least one place, finite nonnegative
amounts and non-increasing order. Tied payouts, trailing zeros, and an all-zero
structure are valid and preserved. There is no seat-count cap or default payout
schedule. The `amounts` accessor returns a read-only slice. Validation covers
each amount and its order; it does not sum the prize pool or compute equity.

`Icm::new` requires a validated payout structure, available through `payouts`.
ICM remains unsupported in phase 6. `Icm::try_utilities` always returns
`PayoffError("ICM not implemented in phase 6")` without changing output, including
when request dimensions or values are invalid. Its `Payoff::utilities`
implementation fills every supplied output entry with NaN. Empty output stays
empty; neither API supplies a finite placeholder equity.

Run `cargo test -p payoff --locked` from the workspace. Unit tests cover folds,
chops, 101 pot splits, malformed lengths, NaN propagation, payout validation,
and checked/unchecked ICM refusal for valid-looking, ragged and empty requests.
The payoff tests, Clippy and formatting checks ran locally on Caleb's Windows
machine on 2026-09-10 without changing security settings. This validates the
phase 6 stub; tournament equity calculation and full phase 6 acceptance remain
outstanding.
