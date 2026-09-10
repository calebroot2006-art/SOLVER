//! Exact evaluation and information-set best response within the supplied tree.
use crate::allocation::{collect, filled, try_collect};
use crate::error::{reach_product, weighted_product};
use crate::{
    Game, NodeId, NodeKind, Real, SolveError, Strategy,
    error::{finite, normalized_sum},
    traversal::{LegacyTerminal, Parallel, SharedRef, TerminalEvaluator},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

/// Two-player zero-sum accuracy measured in chips per hand and root-pot percent.
/// The certificate concerns only the supplied tree, ranges, and utility model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exploitability {
    /// Each player's maximum chip EV against the other player's fixed policy.
    pub br_value: [Real; 2],
    /// Sum of best-response values, in chips per hand (OpenSpiel NashConv).
    pub nash_conv: Real,
    /// NashConv divided by two, in chips per hand.
    pub average: Real,
    /// 100 * average / fixed starting pot. 0.5 means half of one percent.
    pub pct_of_pot: Real,
}

impl Exploitability {
    /// Converts a nonnegative raw NashConv chip target to root-pot percent.
    pub fn nash_conv_to_pct(nash_conv: Real, starting_pot: Real) -> Result<Real, SolveError> {
        checked_conversion(nash_conv, starting_pot, false)
    }
    /// Converts a nonnegative root-pot percentage to raw NashConv chips.
    pub fn pct_to_nash_conv(pct_of_pot: Real, starting_pot: Real) -> Result<Real, SolveError> {
        checked_conversion(pct_of_pot, starting_pot, true)
    }
}

fn checked_conversion(value: Real, pot: Real, inverse: bool) -> Result<Real, SolveError> {
    if !value.is_finite() || value < 0.0 || !pot.is_finite() || pot <= 0.0 {
        return Err(SolveError::InvalidGame(
            "metric conversion needs a finite nonnegative value and a finite positive pot".into(),
        ));
    }
    let converted = if inverse {
        (value / 50.0) * pot
    } else {
        (value / pot) * 50.0
    };
    if !converted.is_finite() {
        return Err(SolveError::InvalidGame("metric conversion overflow".into()));
    }
    if value > 0.0 && converted == 0.0 {
        return Err(SolveError::InvalidGame(
            "metric conversion underflow".into(),
        ));
    }
    Ok(converted)
}

/// Expected net chips for one player under the supplied strategy profile.
pub fn expected_value(
    game: &dyn Game,
    strategy: &Strategy,
    player: usize,
) -> Result<Real, SolveError> {
    strategy.check_game(game)?;
    evaluate(&mut LegacyTerminal(game), strategy, player, false)
}

/// Maximum net chips when this player responds to the fixed opposing strategy.
/// Maximization is per own private state at a public node, never per opponent hand.
pub fn best_response(
    game: &dyn Game,
    strategy: &Strategy,
    player: usize,
) -> Result<Real, SolveError> {
    strategy.check_game(game)?;
    evaluate(&mut LegacyTerminal(game), strategy, player, true)
}

/// Computes both best responses and the explicitly normalized zero-sum metric.
/// Rejects a profile with non-zero-sum expected payoffs. The `Game` implementer
/// must still ensure zero-sum utilities for every compatible terminal deal.
pub fn exploitability(game: &dyn Game, strategy: &Strategy) -> Result<Exploitability, SolveError> {
    strategy.check_game(game)?;
    exploitability_bound(&mut LegacyTerminal(game), strategy)
}

pub(crate) fn exploitability_bound(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
) -> Result<Exploitability, SolveError> {
    exploitability_with(terminal, strategy, None)
}

/// The same measurement with every runout walked on the context's workers.
///
/// Best response holds no accumulators, so an outcome task needs only its own
/// terminal workspace. The four walks, their order, and the arithmetic inside
/// them are the serial ones, so the certificate is bit for bit the same.
pub(crate) fn exploitability_parallel(
    context: &Parallel<'_>,
    strategy: &Strategy,
) -> Result<Exploitability, SolveError> {
    context.pool.install(|| {
        let mut terminal = SharedRef(context.terminal);
        exploitability_with(&mut terminal, strategy, Some(context))
    })
}

fn exploitability_with(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
    parallel: Option<&Parallel<'_>>,
) -> Result<Exploitability, SolveError> {
    let ev = [
        evaluate_with(terminal, strategy, 0, false, parallel)?,
        evaluate_with(terminal, strategy, 1, false, parallel)?,
    ];
    if normalized_sum(ev[0], ev[1]).abs() > 1e-10 {
        return Err(SolveError::InvalidGame(
            "zero-sum exploitability cannot certify these payoffs".into(),
        ));
    }
    let br_value = [
        evaluate_with(terminal, strategy, 0, true, parallel)?,
        evaluate_with(terminal, strategy, 1, true, parallel)?,
    ];
    let raw = br_value[0] + br_value[1];
    finite(&[raw], 0, strategy.layout.root, 0)?;
    if normalized_sum(br_value[0], br_value[1]) < -1e-10 {
        return Err(SolveError::InvalidGame(
            "negative NashConv violates the zero-sum best-response contract".into(),
        ));
    }
    // A negative value inside the explicit f64 allowance carries no evidence of
    // negative exploitability. Only this final reporting value is clamped.
    let nash_conv = raw.max(0.0);
    let average = nash_conv / 2.0;
    let pct_of_pot = Exploitability::nash_conv_to_pct(nash_conv, strategy.layout.pot)?;
    Ok(Exploitability {
        br_value,
        nash_conv,
        average,
        pct_of_pot,
    })
}

pub(crate) fn evaluate(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
    player: usize,
    maximize: bool,
) -> Result<Real, SolveError> {
    evaluate_with(terminal, strategy, player, maximize, None)
}

fn evaluate_with(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
    player: usize,
    maximize: bool,
    parallel: Option<&Parallel<'_>>,
) -> Result<Real, SolveError> {
    if player > 1 {
        return Err(SolveError::InvalidGame("player must be zero or one".into()));
    }
    let layout = &strategy.layout;
    let values = walk_with(
        terminal,
        strategy,
        layout.root,
        player,
        &layout.weights[1 - player],
        &filled(layout.states[player], 1.0)?,
        maximize,
        parallel,
    )?;
    let mut total = 0.0;
    for (value, weight) in values.iter().zip(&layout.weights[player]) {
        total += weighted_product(
            *value,
            *weight,
            terminal.checks_reach_underflow(),
            0,
            layout.root,
            player,
        )?;
    }
    let value = total / layout.normalizer;
    if terminal.checks_reach_underflow() && total != 0.0 && value == 0.0 {
        return Err(SolveError::Arithmetic {
            iteration: 0,
            node: layout.root,
            player,
            reason: "normalized value underflow",
        });
    }
    finite(&[value], 0, layout.root, player)?;
    Ok(value)
}

pub(crate) fn walk(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
    id: NodeId,
    player: usize,
    opponent: &[Real],
    live: &[Real],
    maximize: bool,
) -> Result<Vec<Real>, SolveError> {
    walk_with(
        terminal, strategy, id, player, opponent, live, maximize, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn walk_with(
    terminal: &mut dyn TerminalEvaluator,
    strategy: &Strategy,
    id: NodeId,
    player: usize,
    opponent: &[Real],
    live: &[Real],
    maximize: bool,
    parallel: Option<&Parallel<'_>>,
) -> Result<Vec<Real>, SolveError> {
    let layout = &strategy.layout;
    let node = &layout.nodes[id as usize];
    let mut out = filled(layout.states[player], 0.0)?;
    match node.kind {
        NodeKind::Terminal => {
            out.fill(Real::NAN);
            terminal.evaluate_terminal(id, player, opponent, &mut out, 0)?;
            finite(&out, 0, id, player)?;
            for (value, mask) in out.iter_mut().zip(live) {
                *value *= mask;
            }
        }
        NodeKind::Chance { num_outcomes } => {
            // The same split the CFR walk makes, without accumulators to divide:
            // a measurement only needs each worker's own terminal workspace.
            if let Some(context) = parallel
                && num_outcomes > 1
                && !node.masks.is_empty()
            {
                let checked = context.terminal.checks_reach_underflow();
                let values: Vec<Result<Vec<Real>, SolveError>> = node
                    .children
                    .par_iter()
                    .enumerate()
                    .map(|(outcome, child)| {
                        let masks = layout.masks(node, outcome);
                        let next_opponent =
                            try_collect(opponent.iter().zip(&masks[1 - player]).map(
                                |(reach, mask)| {
                                    reach_product(
                                        reach * mask,
                                        node.probabilities[outcome],
                                        checked,
                                        0,
                                        id,
                                        1 - player,
                                    )
                                },
                            ))?;
                        let next_live =
                            collect(live.iter().zip(&masks[player]).map(|(a, b)| a * b))?;
                        let mut worker = SharedRef(context.terminal);
                        walk_with(
                            &mut worker,
                            strategy,
                            *child,
                            player,
                            &next_opponent,
                            &next_live,
                            maximize,
                            Some(context),
                        )
                    })
                    .collect();
                // Outcome order, so the sum and the first error reported are the
                // serial ones however the workers were scheduled.
                for outcome in values {
                    for (value, add) in out.iter_mut().zip(outcome?) {
                        *value += add;
                    }
                }
            } else {
                for (outcome, child) in node.children.iter().enumerate() {
                    let masks = layout.masks(node, outcome);
                    let next_opponent = try_collect(opponent.iter().zip(&masks[1 - player]).map(
                        |(reach, mask)| {
                            reach_product(
                                reach * mask,
                                node.probabilities[outcome],
                                terminal.checks_reach_underflow(),
                                0,
                                id,
                                1 - player,
                            )
                        },
                    ))?;
                    let next_live = collect(live.iter().zip(&masks[player]).map(|(a, b)| a * b))?;
                    let values = walk_with(
                        terminal,
                        strategy,
                        *child,
                        player,
                        &next_opponent,
                        &next_live,
                        maximize,
                        parallel,
                    )?;
                    for (value, add) in out.iter_mut().zip(values) {
                        *value += add;
                    }
                }
            }
        }
        NodeKind::Player {
            player: actor,
            num_actions,
        } => {
            let n = num_actions as usize;
            let row = &strategy.rows[id as usize];
            if actor as usize == player {
                if maximize {
                    out.fill(Real::NEG_INFINITY);
                }
                for (action, child) in node.children.iter().enumerate() {
                    let values = walk_with(
                        terminal, strategy, *child, player, opponent, live, maximize, parallel,
                    )?;
                    for (state, (value, add)) in out.iter_mut().zip(values).enumerate() {
                        if maximize {
                            *value = value.max(add);
                        } else {
                            *value += weighted_product(
                                add,
                                row[state * n + action],
                                terminal.checks_reach_underflow(),
                                0,
                                id,
                                player,
                            )?;
                        }
                    }
                }
            } else {
                for (action, child) in node.children.iter().enumerate() {
                    let next_opponent =
                        try_collect(opponent.iter().enumerate().map(|(state, reach)| {
                            reach_product(
                                *reach,
                                row[state * n + action],
                                terminal.checks_reach_underflow(),
                                0,
                                id,
                                1 - player,
                            )
                        }))?;
                    let values = walk_with(
                        terminal,
                        strategy,
                        *child,
                        player,
                        &next_opponent,
                        live,
                        maximize,
                        parallel,
                    )?;
                    for (value, add) in out.iter_mut().zip(values) {
                        *value += add;
                    }
                }
            }
        }
    }
    finite(&out, 0, id, player)?;
    Ok(out)
}
