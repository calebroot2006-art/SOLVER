//! Checked state-major strategy rows.
use crate::{Game, NodeId, NodeKind, Real, SolveError, game::{Layout, TraversalLayout}};
use std::sync::Arc;
use crate::allocation::{filled, reserved};

/// Probabilities indexed by public node, then private state, then action.
/// Non-player nodes have empty rows. Every private state's action row sums to one.
#[derive(Clone, Debug)]
pub struct Strategy {
    pub(crate) layout: Arc<TraversalLayout>,
    pub(crate) legacy_binding: Option<Arc<Layout>>,
    pub(crate) rows: Vec<Vec<Real>>,
}

impl Strategy {
    /// Validates the game and all state-major flattened rows.
    pub fn from_rows(game: &dyn Game, rows: Vec<Vec<Real>>) -> Result<Self, SolveError> {
        let binding = Arc::new(Layout::new(game)?);
        let strategy = Self {
            layout: binding.traversal.clone(),
            legacy_binding: Some(binding),
            rows,
        };
        strategy.validate_rows()?;
        Ok(strategy)
    }

    /// Creates uniform probabilities at every information set.
    pub fn uniform(game: &dyn Game) -> Result<Self, SolveError> {
        let binding = Arc::new(Layout::new(game)?);
        Self::uniform_layout(binding.traversal.clone(), Some(binding))
    }

    pub(crate) fn uniform_layout(
        layout: Arc<TraversalLayout>,
        legacy_binding: Option<Arc<Layout>>,
    ) -> Result<Self, SolveError> {
        let mut rows = reserved(layout.nodes.len())?;
        for (id, node) in layout.nodes.iter().enumerate() {
            rows.push(match node.kind {
                NodeKind::Player { num_actions, .. } => {
                    filled(layout.row_len(id), 1.0 / Real::from(num_actions))?
                }
                _ => Vec::new(),
            });
        }
        Ok(Self { layout, legacy_binding, rows })
    }

    /// Reads a public node's flattened state-major row, or None for an invalid ID.
    #[must_use]
    pub fn row(&self, node: NodeId) -> Option<&[Real]> {
        self.rows.get(node as usize).map(Vec::as_slice)
    }

    /// Reads all node-indexed rows, including empty non-player rows.
    #[must_use]
    pub fn rows(&self) -> &[Vec<Real>] {
        &self.rows
    }

    pub(crate) fn validate_rows(&self) -> Result<(), SolveError> {
        if self.rows.len() != self.layout.nodes.len() {
            return Err(SolveError::InvalidGame(
                "strategy node count mismatch".into(),
            ));
        }
        for (id, row) in self.rows.iter().enumerate() {
            if row.len() != self.layout.row_len(id) {
                return Err(SolveError::InvalidGame(format!(
                    "strategy row length mismatch at node {id}"
                )));
            }
            if let NodeKind::Player { num_actions, .. } = self.layout.nodes[id].kind {
                for values in row.chunks_exact(num_actions as usize) {
                    if values
                        .iter()
                        .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
                        || (values.iter().sum::<Real>() - 1.0).abs() > 1e-12
                    {
                        return Err(SolveError::InvalidGame(format!(
                            "invalid strategy probabilities at node {id}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn check_game(&self, game: &dyn Game) -> Result<(), SolveError> {
        self.legacy_binding.as_ref().ok_or_else(|| SolveError::InvalidGame(
            "owned river strategies cannot be rebound to callback games".into(),
        ))?.check_game(game)?;
        self.validate_rows()
    }
}
