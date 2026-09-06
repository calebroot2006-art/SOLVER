//! Independent explicit-deal evaluator and scalar information-set CFR reference.
//!
//! Betting legality and terminal payoffs are implemented here from action strings,
//! without calling `Game::child`, `chance_mask`, `terminal_values`, or `Payoff`.
//! The public tree is used only to locate the strategy row for a public history.

use std::collections::BTreeMap;

use postflop::{Game, NodeId, Real, SolveError, Strategy, Variant};

use crate::{Rules, ToyGame};

#[derive(Clone)]
enum Kind {
    Terminal([Real; 2]),
    Chance(Vec<Real>),
    Decision { player: usize, hand: usize, public: NodeId },
}

#[derive(Clone)]
struct HistoryNode {
    kind: Kind,
    children: Vec<usize>,
    depth: usize,
}

#[derive(Clone)]
struct Position {
    cards: [usize; 2],
    board: Option<usize>,
    spent: [Real; 2],
    round_actions: String,
    history: String,
}

/// An explicit tree with separate nodes for every compatible private deal.
pub struct HistoryOracle {
    nodes: Vec<HistoryNode>,
    normalization: Real,
    weights: [Vec<Real>; 2],
    max_depth: usize,
}

impl HistoryOracle {
    /// Enumerate legal deals and public histories independently of `Game` traversal.
    pub fn new(game: &ToyGame) -> Result<Self, SolveError> {
        let count = game.num_private_states(0);
        let weights = [game.initial_weights(0).to_vec(), game.initial_weights(1).to_vec()];
        if weights.iter().any(|w| w.len() != count || w.iter().any(|x| !x.is_finite() || *x < 0.0)) {
            return Err(SolveError::InvalidGame("oracle private weights".into()));
        }
        let mut deals = Vec::new();
        let mut normalization = 0.0;
        for first in 0..count {
            for second in 0..count {
                let mass = weights[0][first] * weights[1][second];
                if first != second && mass > 0.0 { deals.push(([first, second], mass)); normalization += mass; }
            }
        }
        if normalization <= 0.0 || !normalization.is_finite() { return Err(SolveError::EmptyGame); }
        let mut oracle = Self { nodes: Vec::new(), normalization, weights, max_depth: 0 };
        oracle.nodes.push(HistoryNode { kind: Kind::Chance(Vec::new()), children: Vec::new(), depth: 0 });
        let mut probabilities = Vec::new();
        let mut children = Vec::new();
        for (cards, mass) in deals {
            probabilities.push(mass / normalization);
            children.push(oracle.expand(game, Position {
                cards, board: None, spent: [1.0, 1.0], round_actions: String::new(), history: String::new(),
            }, 1));
        }
        oracle.nodes[0].kind = Kind::Chance(probabilities);
        oracle.nodes[0].children = children;
        Ok(oracle)
    }

    fn expand(&mut self, game: &ToyGame, position: Position, depth: usize) -> usize {
        self.max_depth = self.max_depth.max(depth);
        let index = self.nodes.len();
        self.nodes.push(HistoryNode { kind: Kind::Terminal([0.0; 2]), children: Vec::new(), depth });
        let actions = position.round_actions.as_bytes();
        let folded = actions.last() == Some(&b'f');
        let ended = actions.len() >= 2 && actions.last() == Some(&b'c');
        if folded || (ended && (game.rules() == Rules::Kuhn || position.board.is_some())) {
            let share0 = if folded {
                // The player due to act next won the fold.
                if actions.len() % 2 == 0 { 1.0 } else { 0.0 }
            } else {
                let ranks = if game.rules() == Rules::Kuhn { position.cards } else { position.cards.map(|c| c / 2) };
                let board_rank = position.board.map(|b| b / 2);
                let strengths = ranks.map(|rank| (board_rank == Some(rank), rank));
                match strengths[0].cmp(&strengths[1]) {
                    std::cmp::Ordering::Less => 0.0,
                    std::cmp::Ordering::Equal => 0.5,
                    std::cmp::Ordering::Greater => 1.0,
                }
            };
            let pot: Real = position.spent.iter().sum();
            let first = share0 * pot - position.spent[0];
            self.nodes[index].kind = Kind::Terminal([first, -first]);
        } else if ended {
            let mut children = Vec::new();
            for board in 0..6 {
                if position.cards.contains(&board) { continue; }
                let mut next = position.clone();
                next.board = Some(board);
                next.round_actions.clear();
                next.history = format!("{}/{board}/", position.history);
                children.push(self.expand(game, next, depth + 1));
            }
            self.nodes[index].kind = Kind::Chance(vec![0.25; 4]);
            self.nodes[index].children = children;
        } else {
            let player = actions.len() % 2;
            let raises = actions.iter().filter(|a| **a == b'r').count();
            let mut legal = Vec::new();
            if actions.last() == Some(&b'r') { legal.push('f'); }
            legal.push('c');
            if raises < if game.rules() == Rules::Kuhn { 1 } else { 2 } { legal.push('r'); }
            let public = game.node_for_history(&position.history).expect("independent legal history exists in public tree");
            assert_eq!(legal, game.actions(public), "legal actions for {}", position.history);
            self.nodes[index].kind = Kind::Decision { player, hand: position.cards[player], public };
            let mut children = Vec::new();
            for action in legal {
                let mut next = position.clone();
                next.history.push(action);
                next.round_actions.push(action);
                if action == 'c' { next.spent[player] = next.spent[1 - player]; }
                if action == 'r' {
                    let amount = if game.rules() == Rules::Kuhn { 1.0 } else if position.board.is_none() { 2.0 } else { 4.0 };
                    next.spent[player] = next.spent[1 - player] + amount;
                }
                children.push(self.expand(game, next, depth + 1));
            }
            self.nodes[index].children = children;
        }
        index
    }

    /// Expected utility from scalar histories, in chips per hand.
    #[must_use]
    pub fn expected_value(&self, strategy: &Strategy, player: usize) -> Real {
        let mut values = vec![0.0; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate().rev() {
            values[index] = match &node.kind {
                Kind::Terminal(payoffs) => payoffs[player],
                Kind::Chance(probabilities) => node.children.iter().zip(probabilities).map(|(child, probability)| values[*child] * probability).sum(),
                Kind::Decision { hand, public, .. } => {
                    let count = node.children.len();
                    let row = strategy.row(*public).expect("decision row");
                    node.children.iter().enumerate().map(|(action, child)| values[*child] * row[hand * count + action]).sum()
                }
            };
        }
        values[0]
    }

    /// A legal information-set best response; hidden deals are aggregated before max.
    #[must_use]
    pub fn best_response(&self, strategy: &Strategy, player: usize) -> Real {
        let mut reach = vec![0.0; self.nodes.len()];
        reach[0] = 1.0;
        for (index, node) in self.nodes.iter().enumerate() {
            for (action, child) in node.children.iter().enumerate() {
                let probability = match &node.kind {
                    Kind::Terminal(_) => unreachable!(),
                    Kind::Chance(probabilities) => probabilities[action],
                    Kind::Decision { player: acting, hand, public } => {
                        if *acting == player { 1.0 } else { strategy.row(*public).expect("decision row")[hand * node.children.len() + action] }
                    }
                };
                reach[*child] = reach[index] * probability;
            }
        }
        let mut values = vec![0.0; self.nodes.len()];
        for depth in (0..=self.max_depth).rev() {
            let mut groups: BTreeMap<(NodeId, usize), Vec<usize>> = BTreeMap::new();
            for (index, node) in self.nodes.iter().enumerate().filter(|(_, n)| n.depth == depth) {
                values[index] = match &node.kind {
                    Kind::Terminal(payoffs) => payoffs[player],
                    Kind::Chance(probabilities) => node.children.iter().zip(probabilities).map(|(child, p)| values[*child] * p).sum(),
                    Kind::Decision { player: acting, hand, public } => {
                        if *acting == player { groups.entry((*public, *hand)).or_default().push(index); 0.0 }
                        else {
                            let row = strategy.row(*public).expect("decision row");
                            node.children.iter().enumerate().map(|(a, child)| values[*child] * row[hand * node.children.len() + a]).sum()
                        }
                    }
                };
            }
            for histories in groups.values() {
                let count = self.nodes[histories[0]].children.len();
                let mut scores = vec![0.0; count];
                for &history in histories {
                    for (a, score) in scores.iter_mut().enumerate() { *score += reach[history] * values[self.nodes[history].children[a]]; }
                }
                let selected = scores.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).expect("legal action").0;
                for &history in histories { values[history] = values[self.nodes[history].children[selected]]; }
            }
        }
        values[0]
    }

    /// Run scalar alternating CFR; intended for a few iterations on edge fixtures.
    pub fn cfr(&self, game: &ToyGame, variant: Variant, iterations: u64) -> Result<(Strategy, Strategy), SolveError> {
        let initial = Strategy::uniform(game)?;
        let mut current = initial.rows().to_vec();
        let mut regrets: Vec<Vec<Real>> = current.iter().map(|r| vec![0.0; r.len()]).collect();
        let mut averages = regrets.clone();
        for iteration in 1..=iterations {
            for player in 0..2 {
                self.cfr_walk(0, player, &current, &mut regrets, &mut averages, [1.0; 2], 1.0, if matches!(variant, Variant::Plus) { iteration as Real } else { 1.0 });
                for (index, node) in (0..game.num_nodes()).map(|i| (i, game.kind(i as NodeId))) {
                    if let postflop::NodeKind::Player { player: acting, num_actions } = node {
                        if usize::from(acting) != player { continue; }
                        for hand in 0..game.num_private_states(player) {
                            let count = usize::from(num_actions);
                            let start = hand * count;
                            let row = &mut regrets[index][start..start + count];
                            if matches!(variant, Variant::Plus) { row.iter_mut().for_each(|r| *r = r.max(0.0)); }
                            let positive: Real = row.iter().map(|r| r.max(0.0)).sum();
                            for a in 0..count { current[index][start + a] = if positive > 0.0 { row[a].max(0.0) / positive } else { 1.0 / count as Real }; }
                        }
                    }
                }
            }
            if let Variant::Discounted { alpha, beta, gamma } = variant {
                let t = iteration as Real;
                for row in &mut regrets { for value in row { let power = t.powf(if *value > 0.0 { alpha } else { beta }); *value *= power / (power + 1.0); } }
                for row in &mut averages { for value in row { *value *= (t / (t + 1.0)).powf(gamma); } }
            }
        }
        for (index, node) in (0..game.num_nodes()).map(|i| (i, game.kind(i as NodeId))) {
            if let postflop::NodeKind::Player { num_actions, .. } = node {
                for row in averages[index].chunks_mut(usize::from(num_actions)) {
                    let total: Real = row.iter().sum();
                    let uniform = 1.0 / row.len() as Real;
                    for probability in row { *probability = if total > 0.0 { *probability / total } else { uniform }; }
                }
            }
        }
        Ok((Strategy::from_rows(game, current)?, Strategy::from_rows(game, averages)?))
    }

    #[allow(clippy::too_many_arguments)]
    fn cfr_walk(&self, index: usize, player: usize, policy: &[Vec<Real>], regrets: &mut [Vec<Real>], averages: &mut [Vec<Real>], reach: [Real; 2], chance: Real, average_weight: Real) -> Real {
        let node = &self.nodes[index];
        match &node.kind {
            Kind::Terminal(payoffs) => payoffs[player],
            Kind::Chance(probabilities) => node.children.iter().zip(probabilities).map(|(child, p)| p * self.cfr_walk(*child, player, policy, regrets, averages, reach, chance * p, average_weight)).sum(),
            Kind::Decision { player: acting, hand, public } => {
                let count = node.children.len();
                let row = &policy[*public as usize][hand * count..(hand + 1) * count];
                let mut children = Vec::new();
                for (action, child) in node.children.iter().enumerate() {
                    let mut next_reach = reach;
                    next_reach[*acting] *= row[action];
                    children.push(self.cfr_walk(*child, player, policy, regrets, averages, next_reach, chance, average_weight));
                }
                let value: Real = children.iter().zip(row).map(|(v, p)| v * p).sum();
                if *acting == player {
                    let counterfactual = chance * self.normalization / self.weights[player][*hand] * reach[1 - player];
                    for a in 0..count {
                        regrets[*public as usize][hand * count + a] += counterfactual * (children[a] - value);
                        averages[*public as usize][hand * count + a] += average_weight * reach[player] * row[a];
                    }
                }
                value
            }
        }
    }
}
