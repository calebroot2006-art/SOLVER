use super::memory::{Budget, RiverMemory};
use crate::{
    NodeId, NodeKind, Real, SolveError,
    allocation::{collect, filled, reserved},
    game::{NodeBuild, TraversalLayout},
    terminal::{OutcomeUtilities, ShowdownScratch, ShowdownTable, evaluate_fold},
    traversal::TerminalEvaluator,
};
use cards::{Card, CardSet, Combo, Range};
use std::{fmt, sync::Arc};
use tree::{RiverNodeKind, RiverTree, Terminal};

#[derive(Clone, Copy)]
enum Payoff {
    Decision,
    Fold(f64),
    Showdown([OutcomeUtilities; 2]),
}

pub(super) struct Inner {
    pub board: [Card; 5],
    pub dead: CardSet,
    pub ranges: [Range; 2],
    pub tree: RiverTree,
    pub layout: Arc<TraversalLayout>,
    pub parents: Vec<Option<(NodeId, usize)>>,
    pub memory: RiverMemory,
    pub budget: Arc<Budget>,
    showdown: ShowdownTable,
    payoffs: Vec<Payoff>,
}

/// Immutable river inputs and shared rank groups. Clones retain the same binding.
#[derive(Clone)]
pub struct RiverGame {
    pub(super) inner: Arc<Inner>,
}

impl fmt::Debug for RiverGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RiverGame")
            .field("board", &self.inner.board)
            .field("nodes", &self.inner.tree.nodes().len())
            .field("memory", &self.inner.memory)
            .finish()
    }
}

impl RiverGame {
    /// Own checked inputs and reject invalid joint mass or an insufficient byte budget.
    /// The budget covers all retained solvers, strategy snapshots and active queries
    /// sharing this game. Separate calls to `new` have independent budgets.
    pub fn new(
        board: [Card; 5],
        ranges: [Range; 2],
        tree: RiverTree,
        memory_limit_bytes: usize,
    ) -> Result<Self, SolveError> {
        let dead = CardSet::new(&board).map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let memory = RiverMemory::estimate(&tree)?;
        if memory_limit_bytes == 0
            || memory_limit_bytes as u128 > crate::config::MEMORY_LIMIT_CEILING_BYTES
        {
            return Err(SolveError::Config(format!(
                "river memory limit must be positive and at most {} MiB",
                crate::config::MEMORY_LIMIT_CEILING_MIB
            )));
        }
        if memory.working_set_bound_bytes > memory_limit_bytes {
            return Err(SolveError::MemoryLimit {
                required: memory.working_set_bound_bytes,
                limit: memory_limit_bytes,
            });
        }
        let weights = [scaled(&ranges[0], dead)?, scaled(&ranges[1], dead)?];
        // This one-time pair check detects positive pair mass that multiplication
        // cannot represent. No private-pair matrix or terminal kernel is stored.
        let combos: Vec<_> = collect(Combo::all())?;
        for (a, &wa) in combos.iter().zip(&weights[0]) {
            if wa == 0.0 {
                continue;
            }
            for (b, &wb) in combos.iter().zip(&weights[1]) {
                if wb > 0.0 && a.mask() & b.mask() == 0 && wa * wb == 0.0 {
                    return Err(SolveError::InvalidGame(
                        "positive compatible pair weight underflows".into(),
                    ));
                }
            }
        }
        let mut opposing_mass = [0.0; 1326];
        let opposing: &[f64; 1326] = weights[1]
            .as_slice()
            .try_into()
            .expect("fixed combo vector");
        evaluate_fold(dead, opposing, 1.0, &mut opposing_mass)
            .map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        let normalizer: f64 = weights[0]
            .iter()
            .zip(opposing_mass)
            .map(|(a, b)| a * b)
            .sum();
        if !normalizer.is_finite() || normalizer <= 0.0 {
            return Err(SolveError::EmptyGame);
        }
        let mut nodes = reserved(tree.nodes().len())?;
        let mut payoffs = reserved(tree.nodes().len())?;
        let mut parents = filled(tree.nodes().len(), None)?;
        let half_pot = tree.config().starting_pot as f64 / 2.0;
        for (id, node) in tree.nodes().iter().enumerate() {
            let contributions = node.contributions().map(|n| n as f64);
            let kind = match node.kind() {
                RiverNodeKind::Decision { player } => {
                    payoffs.push(Payoff::Decision);
                    NodeKind::Player {
                        player,
                        num_actions: node.actions().len().try_into().map_err(|_| {
                            SolveError::InvalidGame("river action count exceeds 255".into())
                        })?,
                    }
                }
                RiverNodeKind::Terminal(terminal) => {
                    if contributions[0] != contributions[1] {
                        return Err(SolveError::InvalidGame(
                            "river terminal must refund uncalled contributions".into(),
                        ));
                    }
                    let amount = half_pot + contributions[0];
                    payoffs.push(match terminal {
                        Terminal::Fold { winner } => {
                            Payoff::Fold(if winner == 0 { amount } else { -amount })
                        }
                        Terminal::Showdown => Payoff::Showdown([
                            OutcomeUtilities::new(amount, 0.0, -amount)
                                .map_err(|e| SolveError::InvalidGame(e.to_string()))?,
                            OutcomeUtilities::new(amount, 0.0, -amount)
                                .map_err(|e| SolveError::InvalidGame(e.to_string()))?,
                        ]),
                    });
                    NodeKind::Terminal
                }
            };
            for (action, child) in node.children().iter().enumerate() {
                parents[*child as usize] = Some((id as NodeId, action));
            }
            nodes.push(NodeBuild {
                kind,
                children: collect(node.children().iter().copied())?,
                probabilities: Vec::new(),
                masks: Vec::new(),
            });
        }
        let showdown =
            ShowdownTable::new(board).map_err(|e| SolveError::InvalidGame(e.to_string()))?;
        if showdown.storage_bytes() > 65_536 {
            return Err(SolveError::Allocation(
                "showdown table exceeds its preflight bound".into(),
            ));
        }
        // A river tree has no chance node, so nothing is pooled.
        let layout = Arc::new(TraversalLayout::new(
            tree.root(),
            [1326; 2],
            weights,
            nodes,
            Vec::new(),
            normalizer,
            tree.config().starting_pot as f64,
        )?);
        Ok(Self {
            inner: Arc::new(Inner {
                board,
                dead,
                ranges,
                tree,
                layout,
                parents,
                memory,
                budget: Budget::new(memory_limit_bytes, memory.shared_bytes),
                showdown,
                payoffs,
            }),
        })
    }

    /// Fixed five-card board in the original order.
    pub fn board(&self) -> [Card; 5] {
        self.inner.board
    }
    /// Original inclusion ranges, before board removal and harmless per-range scaling.
    pub fn ranges(&self) -> &[Range; 2] {
        &self.inner.ranges
    }
    /// Checked immutable betting histories and wager labels.
    pub fn tree(&self) -> &RiverTree {
        &self.inner.tree
    }
    /// Conservative retained and temporary allocation estimates.
    pub fn memory_usage(&self) -> RiverMemory {
        self.inner.memory
    }
    /// Bytes currently reserved across all objects and queries sharing this binding.
    pub fn reserved_bytes(&self) -> usize {
        self.inner.budget.used()
    }
    /// Root normalizer after each board-filtered range is divided by its maximum.
    pub fn compatible_weight(&self) -> f64 {
        self.inner.layout.normalizer
    }
    /// Board-filtered and scaled inclusion weights in canonical combo order.
    pub fn initial_weights(&self, player: usize) -> Option<&[f64]> {
        self.inner.layout.weights.get(player).map(Vec::as_slice)
    }
}

fn scaled(range: &Range, dead: CardSet) -> Result<Vec<f64>, SolveError> {
    let mut result = collect(range.weights().iter().copied())?;
    for combo in Combo::all() {
        if combo.mask() & dead.bits() != 0 {
            result[usize::from(combo.id())] = 0.0;
        }
    }
    let maximum = result.iter().copied().fold(0.0_f64, f64::max);
    if maximum == 0.0 {
        return Err(SolveError::EmptyGame);
    }
    for value in &mut result {
        *value /= maximum;
    }
    Ok(result)
}

pub(super) struct RiverTerminal<'a> {
    pub game: &'a Inner,
    pub scratch: &'a mut ShowdownScratch,
}

impl TerminalEvaluator for RiverTerminal<'_> {
    fn checks_reach_underflow(&self) -> bool {
        true
    }
    fn evaluate_terminal(
        &mut self,
        node: NodeId,
        player: usize,
        opponent: &[Real],
        output: &mut [Real],
        iteration: u64,
    ) -> Result<(), SolveError> {
        let fail = |reason: String| SolveError::Terminal {
            iteration,
            node,
            player,
            reason,
        };
        let opponent: &[f64; 1326] = opponent
            .try_into()
            .map_err(|_| fail("invalid opponent vector length".into()))?;
        let output: &mut [f64; 1326] = output
            .try_into()
            .map_err(|_| fail("invalid output vector length".into()))?;
        if player > 1 {
            return Err(fail("invalid player".into()));
        }
        match self.game.payoffs.get(node as usize) {
            Some(Payoff::Fold(value)) => evaluate_fold(
                self.game.dead,
                opponent,
                if player == 0 { *value } else { -*value },
                output,
            ),
            Some(Payoff::Showdown(utilities)) => {
                self.game
                    .showdown
                    .evaluate(opponent, utilities[player], output, self.scratch)
            }
            _ => return Err(fail("node is not a river terminal".into())),
        }
        .map_err(|e| fail(e.to_string()))
    }
}
