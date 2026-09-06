//! Deliberately invalid terminal values for solver error-path tests.

use postflop::{Game, NodeId, NodeKind, Real};

/// One decision and one terminal whose utility is nonfinite.
pub struct NanGame;

impl Game for NanGame {
    fn num_nodes(&self) -> usize { 2 }
    fn root(&self) -> NodeId { 0 }
    fn kind(&self, node: NodeId) -> NodeKind {
        if node == 0 { NodeKind::Player { player: 0, num_actions: 1 } } else { NodeKind::Terminal }
    }
    fn child(&self, _node: NodeId, _index: usize) -> NodeId { 1 }
    fn num_private_states(&self, _player: usize) -> usize { 1 }
    fn initial_weights(&self, _player: usize) -> &[Real] { &[1.0] }
    fn compatible(&self, _p0_state: usize, _p1_state: usize) -> bool { true }
    fn chance_prob(&self, _node: NodeId, _outcome: usize) -> Real { 0.0 }
    fn chance_mask(&self, _node: NodeId, _outcome: usize, _player: usize) -> &[Real] { &[] }
    fn terminal_values(&self, _node: NodeId, _player: usize, _opp_reach: &[Real], out: &mut [Real]) { out.fill(Real::NAN); }
    fn starting_pot(&self) -> Real { 2.0 }
    fn info_label(&self, node: NodeId, player: usize, state: usize) -> String { format!("NaN node={node} player={player} state={state}") }
}
