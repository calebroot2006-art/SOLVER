//! Leduc reference curves and accuracy gates with both profiles' residuals.

mod common;

use postflop::{Game, Variant};
use toygames::leduc;

fn run(variant: Variant) {
    let game = leduc::game();
    let fixture: common::Fixture = toml::from_str(include_str!("../fixtures/leduc.toml")).unwrap();
    let (strategy, residual, value) = common::check_curve(&game, variant, &fixture);
    assert_eq!(strategy.node_rows().len(), game.num_nodes());
    let (reference_value, reference_residual) = fixture.final_reference();
    assert!(
        (value - reference_value).abs() <= residual.nash_conv + reference_residual + 1e-9,
        "value difference must fit both measured equilibrium residuals"
    );
}

#[test]
fn leduc_vanilla_matches_every_captured_checkpoint() {
    run(Variant::Vanilla);
}
#[test]
fn leduc_plus_matches_captured_curve_and_fixed_budget() {
    run(Variant::Plus);
}
#[test]
fn leduc_dcfr_matches_scalar_reference_and_fixed_budget() {
    run(Variant::Discounted {
        alpha: 1.5,
        beta: 0.0,
        gamma: 2.0,
    });
}
