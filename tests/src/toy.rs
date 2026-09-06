use std::collections::HashMap;

use payoff::{ChipEv, Payoff};
use postflop::{Game, NodeId, NodeKind, Real};

/// The explicitly supported toy-poker rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rules {
    /// Three distinct ranks, one betting round and one raise of one chip.
    Kuhn,
    /// Three ranks with two physical copies, two rounds and raises of two/four.
    Leduc,
}

#[derive(Clone, Debug)]
struct Node {
    kind: NodeKind,
    children: Vec<NodeId>,
    actions: Vec<char>,
    history: String,
    board: Option<usize>,
    contributions: [Real; 2],
    winner: Option<usize>,
}

#[derive(Clone)]
struct Betting {
    player: usize,
    raises: usize,
    checks: usize,
    contributions: [Real; 2],
    board: Option<usize>,
    history: String,
}

/// A materialized public tree with small, brute-force terminal evaluation.
#[derive(Clone, Debug)]
pub struct ToyGame {
    rules: Rules,
    nodes: Vec<Node>,
    weights: [Vec<Real>; 2],
    masks: Vec<Vec<Real>>,
    history_nodes: HashMap<String, NodeId>,
}

impl ToyGame {
    /// Build a full tree. Use [`Self::with_weights`] for blocker/normalization tests.
    #[must_use]
    pub fn new(rules: Rules) -> Self {
        let states = if rules == Rules::Kuhn { 3 } else { 6 };
        let mut game = Self {
            rules,
            nodes: Vec::new(),
            weights: [vec![1.0; states], vec![1.0; states]],
            masks: (0..states)
                .map(|board| (0..states).map(|hand| if hand != board { 1.0 } else { 0.0 }).collect())
                .collect(),
            history_nodes: HashMap::new(),
        };
        game.build(Betting {
            player: 0,
            raises: 0,
            checks: 0,
            contributions: [1.0, 1.0],
            board: None,
            history: String::new(),
        });
        game
    }

    /// Replace unnormalized private weights; the solver performs input validation.
    #[must_use]
    pub fn with_weights(mut self, weights: [Vec<Real>; 2]) -> Self {
        self.weights = weights;
        self
    }

    /// The rules used to generate this tree.
    #[must_use]
    pub fn rules(&self) -> Rules { self.rules }

    /// Locate a public history. `c` means check/call, `r` bet/raise and `f` fold.
    /// A physical public card is written `/0/`, ..., `/5/` between rounds.
    #[must_use]
    pub fn node_for_history(&self, history: &str) -> Option<NodeId> {
        self.history_nodes.get(history).copied()
    }

    /// Action characters in row order, empty for chance and terminal nodes.
    #[must_use]
    pub fn actions(&self, node: NodeId) -> &[char] { &self.nodes[node as usize].actions }

    /// Public card at a node, if the second Leduc round has started.
    #[must_use]
    pub fn board(&self, node: NodeId) -> Option<usize> { self.nodes[node as usize].board }

    /// Count legal information sets structurally, regardless of policy/range reach.
    #[must_use]
    pub fn legal_information_sets(&self) -> usize {
        self.nodes.iter().filter(|node| matches!(node.kind, NodeKind::Player { .. }))
            .map(|node| self.num_private_states(0) - usize::from(node.board.is_some())).sum()
    }

    fn allocate(&mut self, state: &Betting, kind: NodeKind, winner: Option<usize>) -> NodeId {
        let id = NodeId::try_from(self.nodes.len()).expect("toy tree fits u32");
        self.history_nodes.insert(state.history.clone(), id);
        self.nodes.push(Node {
            kind, children: Vec::new(), actions: Vec::new(), history: state.history.clone(),
            board: state.board, contributions: state.contributions, winner,
        });
        id
    }

    fn round_end(&mut self, state: Betting) -> NodeId {
        if self.rules == Rules::Kuhn || state.board.is_some() {
            return self.allocate(&state, NodeKind::Terminal, None);
        }
        let id = self.allocate(&state, NodeKind::Chance { num_outcomes: 6 }, None);
        let mut children = Vec::with_capacity(6);
        for board in 0..6 {
            children.push(self.build(Betting {
                player: 0, raises: 0, checks: 0, contributions: state.contributions,
                board: Some(board), history: format!("{}/{board}/", state.history),
            }));
        }
        self.nodes[id as usize].children = children;
        id
    }

    fn build(&mut self, state: Betting) -> NodeId {
        let facing = state.contributions[state.player] < state.contributions[1 - state.player];
        let max_raises = if self.rules == Rules::Kuhn { 1 } else { 2 };
        let mut actions = if facing { vec!['f', 'c'] } else { vec!['c'] };
        if state.raises < max_raises { actions.push('r'); }
        let id = self.allocate(&state, NodeKind::Player {
            player: state.player as u8, num_actions: actions.len() as u8,
        }, None);
        let mut children = Vec::new();
        for &action in &actions {
            let mut next = state.clone();
            next.history.push(action);
            next.player = 1 - state.player;
            let child = match action {
                'f' => self.allocate(&next, NodeKind::Terminal, Some(next.player)),
                'c' => {
                    next.contributions[state.player] = state.contributions[1 - state.player];
                    next.checks += 1;
                    if facing || next.checks == 2 { self.round_end(next) } else { self.build(next) }
                }
                'r' => {
                    let increment = match (self.rules, state.board) {
                        (Rules::Kuhn, _) => 1.0,
                        (Rules::Leduc, None) => 2.0,
                        (Rules::Leduc, Some(_)) => 4.0,
                    };
                    next.contributions[state.player] = state.contributions[1 - state.player] + increment;
                    next.raises += 1;
                    next.checks = 0;
                    self.build(next)
                }
                _ => unreachable!("builder emits only poker actions"),
            };
            children.push(child);
        }
        self.nodes[id as usize].actions = actions;
        self.nodes[id as usize].children = children;
        id
    }

    fn showdown_strength(&self, card: usize, board: Option<usize>) -> usize {
        match self.rules {
            Rules::Kuhn => card,
            Rules::Leduc => {
                let rank = card / 2;
                rank + if board.is_some_and(|b| b / 2 == rank) { 3 } else { 0 }
            }
        }
    }
}

impl Game for ToyGame {
    fn num_nodes(&self) -> usize { self.nodes.len() }
    fn root(&self) -> NodeId { 0 }
    fn kind(&self, node: NodeId) -> NodeKind { self.nodes[node as usize].kind }
    fn child(&self, node: NodeId, index: usize) -> NodeId { self.nodes[node as usize].children[index] }
    fn num_private_states(&self, _player: usize) -> usize { if self.rules == Rules::Kuhn { 3 } else { 6 } }
    fn initial_weights(&self, player: usize) -> &[Real] { &self.weights[player] }
    fn compatible(&self, p0_state: usize, p1_state: usize) -> bool { p0_state != p1_state }
    fn chance_prob(&self, _node: NodeId, _outcome: usize) -> Real { 0.25 }
    fn chance_mask(&self, _node: NodeId, outcome: usize, _player: usize) -> &[Real] { &self.masks[outcome] }
    fn starting_pot(&self) -> Real { 2.0 }
    fn info_label(&self, node: NodeId, player: usize, state: usize) -> String {
        format!("player={player}; card={state}; history={}", self.nodes[node as usize].history)
    }
    fn terminal_values(&self, node: NodeId, player: usize, opp_reach: &[Real], out: &mut [Real]) {
        let terminal = &self.nodes[node as usize];
        for (hand, value) in out.iter_mut().enumerate() {
            *value = 0.0;
            for (opponent, reach) in opp_reach.iter().enumerate() {
                if *reach == 0.0 || hand == opponent || terminal.board == Some(hand) || terminal.board == Some(opponent) { continue; }
                let mut shares = [0.5, 0.5];
                let winner = terminal.winner.or_else(|| {
                    let us = self.showdown_strength(hand, terminal.board);
                    let them = self.showdown_strength(opponent, terminal.board);
                    if us == them { None } else { Some(if us > them { player } else { 1 - player }) }
                });
                if let Some(winner) = winner { shares = [0.0, 0.0]; shares[winner] = 1.0; }
                let mut utilities = [0.0; 2];
                ChipEv.utilities(&[100.0, 100.0], &terminal.contributions, &shares, &mut utilities);
                *value += reach * utilities[player];
            }
        }
    }
}
