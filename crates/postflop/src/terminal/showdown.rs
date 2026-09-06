use super::mass::{Bucket, weighted_value};
use super::{OutcomeUtilities, TerminalError, validate_reach};
use cards::{Card, Combo, HandValue, RiverEvaluator};

struct RankedCombo {
    combo: Combo,
    value: HandValue,
}

struct Group {
    start: usize,
    end: usize,
}

/// Immutable rank groups for every unblocked hold'em combo on one river board.
/// Higher hand values are stronger. Construction evaluates and sorts once;
/// repeated traversals require linear work in the number of live combos.
pub struct ShowdownTable {
    entries: Vec<RankedCombo>,
    groups: Vec<Group>,
}

/// Reusable fixed-size workspace for checked showdown evaluation.
/// Output staging, three outcome masses, and exact blocker sums use linear
/// storage; no pairwise payoff matrix or allocation is needed per traversal.
pub struct ShowdownScratch {
    masses: [[f64; 3]; 1326],
    result: [f64; 1326],
    weaker: Bucket,
    equal: Bucket,
    stronger: Bucket,
}

impl Default for ShowdownScratch {
    fn default() -> Self {
        Self {
            masses: [[0.0; 3]; 1326],
            result: [0.0; 1326],
            weaker: Bucket::default(),
            equal: Bucket::default(),
            stronger: Bucket::default(),
        }
    }
}

impl ShowdownScratch {
    /// Bytes occupied by this workspace, with no external heap allocations.
    pub fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl ShowdownTable {
    /// Validates five distinct board cards and caches all unblocked combo ranks.
    pub fn new(board: [Card; 5]) -> Result<Self, TerminalError> {
        let evaluator = RiverEvaluator::new(board)?;
        let dead = board.iter().fold(0, |mask, card| mask | card.mask());
        let mut entries = Vec::with_capacity(1081);
        for combo in Combo::all() {
            if combo.mask() & dead == 0 {
                entries.push(RankedCombo {
                    combo,
                    value: evaluator.evaluate(combo)?,
                });
            }
        }
        entries.sort_unstable_by(|lhs, rhs| {
            lhs.value
                .cmp(&rhs.value)
                .then_with(|| lhs.combo.id().cmp(&rhs.combo.id()))
        });
        let mut groups = Vec::new();
        let mut start = 0;
        while start < entries.len() {
            let mut end = start + 1;
            while end < entries.len() && entries[end].value == entries[start].value {
                end += 1;
            }
            groups.push(Group { start, end });
            start = end;
        }
        Ok(Self { entries, groups })
    }

    /// Storage occupied by the table and its allocated entry/group buffers.
    /// This excludes the evaluator's process-wide static lookup tables.
    pub fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.entries.capacity() * std::mem::size_of::<RankedCombo>()
            + self.groups.capacity() * std::mem::size_of::<Group>()
    }

    /// Computes unnormalized counterfactual values in canonical combo order.
    ///
    /// Blocker sums and subtractions are exact for the supplied f64 reaches,
    /// then each compatible mass is rounded once to nearest, ties to even.
    /// Each mass-times-utility product rounds normally; their signed sum is
    /// accumulated exactly before its final rounding. Nonzero products that
    /// underflow to zero, overflow, and invalid inputs return an error.
    /// `output` stays unchanged on every error; scratch contents are unspecified.
    pub fn evaluate(
        &self,
        opponent_reach: &[f64; 1326],
        utilities: OutcomeUtilities,
        output: &mut [f64; 1326],
        scratch: &mut ShowdownScratch,
    ) -> Result<(), TerminalError> {
        validate_reach(opponent_reach)?;
        scratch.masses.fill([0.0; 3]);
        scratch.result.fill(0.0);
        scratch.weaker.clear();
        scratch.stronger.clear();
        for group in &self.groups {
            scratch.equal.clear();
            for entry in &self.entries[group.start..group.end] {
                scratch
                    .equal
                    .add(entry.combo, opponent_reach[usize::from(entry.combo.id())])?;
            }
            for entry in &self.entries[group.start..group.end] {
                let id = usize::from(entry.combo.id());
                scratch.masses[id][0] = scratch.weaker.compatible(entry.combo, 0.0)?;
                scratch.masses[id][1] =
                    scratch.equal.compatible(entry.combo, opponent_reach[id])?;
            }
            for entry in &self.entries[group.start..group.end] {
                scratch
                    .weaker
                    .add(entry.combo, opponent_reach[usize::from(entry.combo.id())])?;
            }
        }
        for group in self.groups.iter().rev() {
            for entry in &self.entries[group.start..group.end] {
                let id = usize::from(entry.combo.id());
                scratch.masses[id][2] = scratch.stronger.compatible(entry.combo, 0.0)?;
            }
            for entry in &self.entries[group.start..group.end] {
                scratch
                    .stronger
                    .add(entry.combo, opponent_reach[usize::from(entry.combo.id())])?;
            }
        }
        for entry in &self.entries {
            let id = usize::from(entry.combo.id());
            scratch.result[id] = weighted_value(scratch.masses[id], utilities.values())?;
        }
        output.copy_from_slice(&scratch.result);
        Ok(())
    }
}
