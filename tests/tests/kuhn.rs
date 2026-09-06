//! Kuhn reference curves, fixed-budget accuracy and equilibrium structure.

mod common;

use postflop::Variant;
use toygames::kuhn;

fn run(variant: Variant) {
    let game = kuhn::game();
    let fixture: common::Fixture = toml::from_str(include_str!("../fixtures/kuhn.toml")).unwrap();
    let (strategy, residual, value) = common::check_curve(&game, variant, &fixture);
    let (reference_value, reference_residual) = fixture.final_reference();
    assert!((value - reference_value).abs() <= residual.nash_conv + reference_residual + 1e-9);
    if !matches!(variant, Variant::Vanilla) {
        assert!((value + 1.0 / 18.0).abs() < 1e-4);
    }
    if matches!(variant, Variant::Discounted { .. }) {
        let probability = |history, hand: usize| {
            strategy.row(game.node_for_history(history).unwrap()).unwrap()[hand * 2 + 1]
        };
        for (actual, expected) in [
            (probability("c", 2), 1.0), // Player 1 value bets king after a check.
            (probability("r", 2), 1.0), // Calls king.
            (probability("r", 0), 0.0), // Folds jack to a bet.
            (probability("c", 0), 1.0 / 3.0),
            (probability("r", 1), 1.0 / 3.0),
            (probability("c", 1), 0.0),
            (probability("", 1), 0.0),
            (probability("", 2), 3.0 * probability("", 0)),
        ] { assert!((actual - expected).abs() < 0.02, "{actual} != equilibrium {expected}"); }
    }
}

#[test]
fn kuhn_vanilla_matches_captured_curve_and_fixed_budget() { run(Variant::Vanilla); }
#[test]
fn kuhn_plus_matches_captured_curve_and_fixed_budget() { run(Variant::Plus); }
#[test]
fn kuhn_dcfr_matches_scalar_reference_and_equilibrium_structure() {
    run(Variant::Discounted { alpha: 1.5, beta: 0.0, gamma: 2.0 });
}
