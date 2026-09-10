//! Checked state-major strategy rows in one flat buffer.
use crate::allocation::{filled, reserved};
use crate::{
    Game, NodeId, NodeKind, Real, SolveError,
    game::{Layout, TraversalLayout},
};
use std::sync::Arc;

/// Probabilities indexed by public node, then private state, then action.
///
/// Every node's row lives in one contiguous buffer, sliced by the layout's
/// `row_offsets`; non-player nodes own an empty slice. Every private state's
/// action row sums to one.
#[derive(Clone, Debug)]
pub struct Strategy {
    pub(crate) layout: Arc<TraversalLayout>,
    pub(crate) legacy_binding: Option<Arc<Layout>>,
    pub(crate) values: Vec<Real>,
}

impl Strategy {
    /// Validates the game and all state-major flattened rows, node by node.
    pub fn from_rows(game: &dyn Game, rows: Vec<Vec<Real>>) -> Result<Self, SolveError> {
        let binding = Arc::new(Layout::new(game)?);
        Self::from_node_rows(binding.traversal.clone(), Some(binding), rows)
    }

    /// Validates one flat buffer of state-major rows against a bound layout.
    pub(crate) fn from_values(
        layout: Arc<TraversalLayout>,
        legacy_binding: Option<Arc<Layout>>,
        values: Vec<Real>,
    ) -> Result<Self, SolveError> {
        let strategy = Self {
            layout,
            legacy_binding,
            values,
        };
        strategy.validate_rows()?;
        Ok(strategy)
    }

    pub(crate) fn from_node_rows(
        layout: Arc<TraversalLayout>,
        legacy_binding: Option<Arc<Layout>>,
        rows: Vec<Vec<Real>>,
    ) -> Result<Self, SolveError> {
        if rows.len() != layout.num_nodes() {
            return Err(SolveError::InvalidGame(
                "strategy node count mismatch".into(),
            ));
        }
        let mut values = reserved(layout.entries())?;
        for (id, row) in rows.iter().enumerate() {
            if row.len() != layout.row_len(id) {
                return Err(SolveError::InvalidGame(format!(
                    "strategy row length mismatch at node {id}"
                )));
            }
            values.extend_from_slice(row);
        }
        Self::from_values(layout, legacy_binding, values)
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
        let mut values = filled(layout.entries(), 0.0)?;
        for id in 0..layout.num_nodes() {
            if let NodeKind::Player { num_actions, .. } = layout.kinds[id] {
                let range = layout.row_range(id);
                values[range].fill(1.0 / Real::from(num_actions));
            }
        }
        Ok(Self {
            layout,
            legacy_binding,
            values,
        })
    }

    /// Reads a public node's flattened state-major row, or None for an invalid ID.
    #[must_use]
    pub fn row(&self, node: NodeId) -> Option<&[Real]> {
        let node = node as usize;
        (node < self.layout.num_nodes()).then(|| &self.values[self.layout.row_range(node)])
    }

    /// Every node's rows end to end, in node order. Read one node's slice with
    /// [`Self::row`]; this is the buffer those slices come from.
    #[must_use]
    pub fn values(&self) -> &[Real] {
        &self.values
    }

    /// A copy of each node's row as its own vector, in node order.
    ///
    /// The rows themselves live in one flat buffer, so this allocates: it is
    /// for a caller that wants to edit rows and hand them back through
    /// [`Self::from_rows`], not for anything on a walk.
    #[must_use]
    pub fn node_rows(&self) -> Vec<Vec<Real>> {
        (0..self.layout.num_nodes())
            .map(|node| self.values[self.layout.row_range(node)].to_vec())
            .collect()
    }

    pub(crate) fn validate_rows(&self) -> Result<(), SolveError> {
        if self.values.len() != self.layout.entries() {
            return Err(SolveError::InvalidGame(
                "strategy entry count mismatch".into(),
            ));
        }
        for id in 0..self.layout.num_nodes() {
            if let NodeKind::Player { num_actions, .. } = self.layout.kinds[id] {
                let row = &self.values[self.layout.row_range(id)];
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

    /// Policy rows for a walk that reads a stored strategy.
    pub(crate) fn source(&self) -> PolicySource<'_> {
        PolicySource::Stored(self)
    }

    pub(crate) fn check_game(&self, game: &dyn Game) -> Result<(), SolveError> {
        self.legacy_binding
            .as_ref()
            .ok_or_else(|| {
                SolveError::InvalidGame(
                    "owned river strategies cannot be rebound to callback games".into(),
                )
            })?
            .check_game(game)?;
        self.validate_rows()
    }
}

/// Regret matching: positive part over its own sum, uniform when nothing is
/// positive, and rescaled when finite positive entries overflow their sum.
pub(crate) fn normalize_positive(values: &[Real], out: &mut [Real]) {
    let sum: Real = values.iter().map(|value| value.max(0.0)).sum();
    if sum > 0.0 && sum.is_finite() {
        for (value, target) in values.iter().zip(out) {
            *target = value.max(0.0) / sum;
        }
    } else if sum == 0.0 {
        let uniform = 1.0 / out.len() as Real;
        out.fill(uniform);
    } else {
        // Scale before summing when finite positive entries overflow their sum.
        let scale = values.iter().copied().fold(0.0, Real::max);
        let scaled_sum: Real = values.iter().map(|v| v.max(0.0) / scale).sum();
        for (value, target) in values.iter().zip(out) {
            *target = (value.max(0.0) / scale) / scaled_sum;
        }
    }
}

/// Reusable row buffers, so a walk that derives a policy row per decision node
/// allocates once per depth rather than once per visit.
///
/// A row is taken before the node's children are walked and given back when the
/// node is done, so the pool holds at most one row per level of the path the
/// walk is on. The traversal term of the memory estimate charges for them.
#[derive(Clone, Debug, Default)]
pub(crate) struct RowPool(Vec<Vec<Real>>);

impl RowPool {
    pub fn take(&mut self, len: usize) -> Result<Vec<Real>, SolveError> {
        match self.0.pop() {
            Some(mut row) => {
                row.clear();
                row.try_reserve_exact(len).map_err(|error| {
                    SolveError::Allocation(format!("cannot reserve a {len}-entry row: {error}"))
                })?;
                row.resize(len, 0.0);
                Ok(row)
            }
            None => filled(len, 0.0),
        }
    }

    pub fn give(&mut self, row: Vec<Real>) {
        self.0.push(row);
    }
}

/// Where a walk reads its policy rows from.
///
/// A measurement used to take a whole retained average first. It does not have
/// to: regret matching over the cumulative strategy sums is exactly the average
/// strategy, one row at a time, so a best-response walk can normalise the sums
/// as it reads them and retain nothing. [`Self::Stored`] is the other case, a
/// policy that already exists as rows, and it hands the walk a borrow.
pub(crate) enum PolicySource<'a> {
    /// A materialised strategy; rows are borrowed straight out of its buffer.
    Stored(&'a Strategy),
    /// Cumulative strategy sums, normalised per state row as they are read.
    Normalised {
        layout: &'a TraversalLayout,
        values: &'a [Real],
    },
}

/// One node's policy row: borrowed when it is stored, derived into a pooled
/// buffer when it is not.
pub(crate) struct PolicyRow<'a> {
    borrowed: Option<&'a [Real]>,
    derived: Vec<Real>,
}

impl PolicyRow<'_> {
    pub fn values(&self) -> &[Real] {
        self.borrowed.unwrap_or(&self.derived)
    }
}

impl<'a> PolicySource<'a> {
    pub fn layout(&self) -> &'a TraversalLayout {
        match self {
            Self::Stored(strategy) => &strategy.layout,
            Self::Normalised { layout, .. } => layout,
        }
    }

    /// The row at one decision node, and a buffer to give back when it is done.
    pub fn row(
        &self,
        node: NodeId,
        actions: usize,
        pool: &mut RowPool,
    ) -> Result<PolicyRow<'a>, SolveError> {
        let range = self.layout().row_range(node as usize);
        match self {
            Self::Stored(strategy) => Ok(PolicyRow {
                borrowed: Some(&strategy.values[range]),
                derived: Vec::new(),
            }),
            Self::Normalised { values, .. } => {
                let mut derived = pool.take(range.len())?;
                for (sums, out) in values[range]
                    .chunks_exact(actions)
                    .zip(derived.chunks_exact_mut(actions))
                {
                    normalize_positive(sums, out);
                }
                Ok(PolicyRow {
                    borrowed: None,
                    derived,
                })
            }
        }
    }

    pub fn give(&self, pool: &mut RowPool, row: PolicyRow<'_>) {
        if row.borrowed.is_none() {
            pool.give(row.derived);
        }
    }
}
