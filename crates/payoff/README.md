# Payoff

`ChipEv` computes net chips as `pot_share * total_contributions - own_contribution`.
The shared `Real` alias is `f64`. This crate has no dependencies.

Use `ChipEv::try_utilities` when constructing or validating a terminal. It rejects
length mismatches, non-finite or negative inputs, invalid shares, and pot overflow
before changing the output. The planned `Payoff::utilities` hook has no error return;
its ChipEv implementation fills the output with NaN on invalid input. The solver
detects that and returns a contextual error.

`stacks_before` is validated but does not change chip EV. This interface currently
certifies chip payoffs only. Rake, side-pot allocation, ICM, and bounty rules need
their own reviewed inputs and metrics; a single pot-share vector is not a complete
tournament model.

Run `cargo test -p payoff --locked` from the workspace. Unit tests cover folds,
chops, 101 pot splits, malformed lengths, and NaN propagation. Compiler execution
on Caleb's Windows machine is blocked by Smart App Control. Root integration runs
Rust checks through the approved GitHub Actions workflow.