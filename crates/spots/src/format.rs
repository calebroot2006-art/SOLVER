//! Borrowed version 1 input records and private, bounded structural storage.
//!
//! Public records are unvalidated DTOs, not ownership or provenance certificates.
//! Only the builder produces validated storage. It never builds a betting tree.

use std::fmt::Write;
use std::mem::size_of;

use cards::{Card, Combo, Range};
pub use tree::{Action, BetSize, Street};

use crate::resource::{BudgetVec, Lease, ResourceBudget, ResourceError, ResourceLimits, add};

/// A named structural or resource refusal without allocated diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatError(pub &'static str);

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.0) }
}
impl std::error::Error for FormatError {}
impl From<ResourceError> for FormatError {
    fn from(value: ResourceError) -> Self { Self(value.0) }
}

/// Finite f64 bits, without field-specific sign or magnitude constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteF64(f64);
impl FiniteF64 {
    /// Refuse NaNs and infinities, preserving every other bit.
    pub fn new(value: f64) -> Result<Self, FormatError> {
        if value.is_finite() { Ok(Self(value)) } else { Err(FormatError("nonfinite f64")) }
    }
    /// Original finite value.
    pub const fn get(self) -> f64 { self.0 }
}

/// Finite f32 bits, without field-specific sign or magnitude constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteF32(f32);
impl FiniteF32 {
    /// Refuse NaNs and infinities, preserving every other bit.
    pub fn new(value: f32) -> Result<Self, FormatError> {
        if value.is_finite() { Ok(Self(value)) } else { Err(FormatError("nonfinite f32")) }
    }
    /// Original finite value.
    pub const fn get(self) -> f32 { self.0 }
}

/// Declared decision coverage; completeness remains a source claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    /// Decisions on the root street.
    StartStreet,
    /// Decisions with fewer than this many preceding player actions.
    ActionDepth(u16),
}

/// Version 1's sole payoff convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayoffModel { /// Heads-up chip EV without rake.
    HeadsUpChipEvNoRake }
/// Version 1's sole EV origin convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvUnits { /// Net chips from the fixed root origin.
    NetChipsFromFixedRootOrigin }
/// Version 1's sole accuracy convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccuracyUnits { /// NashConv chips and half as a percentage of root pot.
    NashconvChipsAndHalfAsRootPotPercent }
/// Missing coverage has no grade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UncoveredPolicy { /// Leave uncovered decisions ungraded.
    Ungraded }
/// Version 1 quantization algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantizationAlgorithm { /// Largest-remainder probabilities and scaled i16 EVs.
    LargestRemainderU16ScaledI16V1 }

/// Ordered sizes for one street/player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeMenu<'a> {
    /// Opening bets, preserving source order.
    pub bets: &'a [BetSize],
    /// Raises, preserving source order.
    pub raises: &'a [BetSize],
}

/// Canonical compact-tree input; validation does not expand the tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeSpec<'a> {
    /// Pot at the root, in chips.
    pub starting_pot: u64,
    /// Maximum commitment per player from the root.
    pub effective_stack: u64,
    /// Minimum opening bet.
    pub min_bet: u64,
    /// Root street.
    pub start_street: Street,
    /// Three streets, each with two player menus.
    pub sizes: [[SizeMenu<'a>; 2]; 3],
    /// Maximum raises on each street.
    pub max_raises: u8,
    /// Automatic all-in addition threshold.
    pub add_all_in_threshold: f64,
    /// Automatic all-in replacement threshold.
    pub force_all_in_threshold: f64,
    /// Compact public node ceiling.
    pub max_nodes: u32,
}

/// Authoritative canonical game inputs, without a provisional digest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSpecV1<'a> {
    /// Ordered root board IDs.
    pub board: &'a [u8],
    /// Original canonical weighted ranges in player order.
    pub ranges: [&'a str; 2],
    /// Chip size of one big blind.
    pub chips_per_big_blind: f64,
    /// All compact-tree settings.
    pub tree: TreeSpec<'a>,
}

/// Claimed process-local solve attempt identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attempt {
    /// Positive process-local solver ID.
    pub solver: u64,
    /// Positive generation.
    pub generation: u32,
}

/// Claimed residual accuracy at one policy iteration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    /// Measured iteration.
    pub iteration: u64,
    /// Two best-response values in chips.
    pub br_values: [f64; 2],
    /// Nonnegative sum of best-response values.
    pub nash_conv: f64,
    /// Half NashConv in chips.
    pub average: f64,
    /// Half NashConv as a percentage of the root pot.
    pub pct_of_pot: f64,
}

/// Claimed root values at the represented policy iteration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootValues {
    /// Evaluated policy iteration.
    pub iteration: u64,
    /// Two root utility values.
    pub values: [f64; 2],
}

/// Claimed completion reason; failure is never a stored solved entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    /// Fresh measurement met the target.
    TargetReached,
    /// Fresh measurement exceeded the target at or past the cap.
    IterationCap,
    /// Cancellation can retain absent, stale or fresh measurement.
    Cancelled,
}

/// Claimed source CFR variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variant {
    /// Vanilla CFR.
    Vanilla,
    /// CFR+.
    Plus,
    /// Discounted CFR with nonnegative finite exponents.
    Discounted {
        /// Positive-regret exponent.
        alpha: f64,
        /// Negative-regret exponent.
        beta: f64,
        /// Strategy-sum exponent.
        gamma: f64,
    },
}

/// Claimed storage precision; this does not certify producer support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Precision {
    /// f64 storage.
    F64,
    /// f32 storage.
    F32,
    /// i16 storage.
    I16,
}

/// Untrusted source metadata with internally consistent reporting operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceRecord<'a> {
    /// Nonempty producer process/run identifier.
    pub producer_run_id: &'a str,
    /// Optional process-local attempt claim.
    pub attempt: Option<Attempt>,
    /// Completed iteration of the represented policy.
    pub policy_iteration: u64,
    /// Optional latest measurement, possibly stale after cancellation.
    pub measurement: Option<Measurement>,
    /// Optional policy root values.
    pub root_values: Option<RootValues>,
    /// Claimed stop reason.
    pub stop_reason: StopReason,
    /// Requested accuracy target.
    pub target_pct_of_pot: f64,
    /// Requested cap, possibly lowered below the resumed iteration.
    pub max_iterations: u64,
    /// Positive measurement interval.
    pub check_every: u64,
    /// Claimed CFR variant.
    pub variant: Variant,
    /// Claimed source storage precision.
    pub precision: Precision,
    /// Monotonic elapsed duration.
    pub elapsed_nanoseconds: u64,
    /// Positive finite raw source claim for the scaled compatible denominator.
    pub root_compatible_mass: f64,
}

impl SourceRecord<'_> {
    /// Whether the available measurement predates the represented policy.
    pub fn measurement_is_stale(&self) -> bool {
        self.measurement.is_some_and(|m| m.iteration < self.policy_iteration)
    }
}

/// Display metadata; never a source binding certificate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProvenanceRecord<'a> {
    /// Claimed solver revision.
    pub solver_revision: &'a str,
    /// Claimed solver version.
    pub solver_version: &'a str,
    /// Claimed producer version.
    pub producer_version: &'a str,
    /// UTC Gregorian date/time in YYYY-MM-DDTHH:MM:SSZ form.
    pub generated_at_utc: &'a str,
    /// Claimed host.
    pub host: &'a str,
}

/// Quantization error claims, checked against contained records at finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantization {
    /// Versioned algorithm.
    pub algorithm: QuantizationAlgorithm,
    /// Largest per-action probability error.
    pub max_probability_error: f64,
    /// Largest per-action EV error in chips.
    pub max_ev_error: f64,
    /// Largest own-reach error.
    pub max_reach_error: f64,
}

/// Borrowed, unvalidated version 1 header DTO.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotHeaderV1<'a> {
    /// Exactly one.
    pub version: u16,
    /// Nonempty descriptive spot identifier.
    pub spot_id: &'a str,
    /// Nonempty descriptive scenario identifier.
    pub scenario_id: &'a str,
    /// Two nonempty position labels.
    pub positions: [&'a str; 2],
    /// Two nonempty range labels.
    pub range_labels: [&'a str; 2],
    /// Canonical game configuration.
    pub game: GameSpecV1<'a>,
    /// Declared coverage cut.
    pub coverage: Coverage,
    /// Payoff convention.
    pub payoff_model: PayoffModel,
    /// EV origin and units.
    pub ev_units: EvUnits,
    /// Accuracy units.
    pub accuracy_units: AccuracyUnits,
    /// Treatment of uncovered decisions.
    pub uncovered_policy: UncoveredPolicy,
    /// Internally checked source claims.
    pub source: SourceRecord<'a>,
    /// Untrusted provenance claims.
    pub provenance: ProvenanceRecord<'a>,
    /// Aggregate representation error claims.
    pub quantization: Quantization,
}

/// One event preceding a stored decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HistoryEvent {
    /// A player action.
    Action {
        /// Acting player, zero or one.
        player: u8,
        /// Chosen action.
        action: Action,
    },
    /// The next ordered public card.
    Deal {
        /// Card ID.
        card: u8,
    },
}

/// Original-to-representative runout mapping. Version 1 requires identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mapping {
    /// Must equal this record's node ID.
    pub representative_node: u32,
    /// Must be [0,1,2,3].
    pub suit_permutation: [u8; 4],
}

/// Borrowed, unvalidated combo row DTO.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComboRecord<'a> {
    /// Canonical combo ID.
    pub combo_id: u16,
    /// Ordered probabilities, summing to 65535.
    pub probabilities: &'a [u16],
    /// Ordered optional EV integers; -32768 is not a finite value.
    pub action_evs: &'a [Option<i16>],
    /// Stored own reach, excluding chance factors.
    pub own_reach: f32,
    /// Measured absolute reach quantization error claim.
    pub reach_error: f64,
    /// Compatible opposing range/action/chance mass claim.
    pub opponent_mass: f64,
}

/// Borrowed, unvalidated decision record DTO.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeRecord<'a> {
    /// Expanded node ID.
    pub node_id: u32,
    /// Compact source node ID.
    pub compact_id: u32,
    /// Events from the root, excluding this decision's own choice.
    pub history: &'a [HistoryEvent],
    /// Ordered board including the root prefix.
    pub board: &'a [u8],
    /// Decision street.
    pub street: Street,
    /// Acting player.
    pub player: u8,
    /// Exact ordered action menu.
    pub actions: &'a [Action],
    /// Commitments from the root.
    pub contributions: [u64; 2],
    /// Positive finite EV scale.
    pub ev_scale: f32,
    /// Maximum probability error claim.
    pub max_probability_error: f64,
    /// Maximum EV error claim.
    pub max_ev_error: f64,
    /// Identity runout mapping.
    pub mapping: Mapping,
    /// Increasing, unique live combos.
    pub combos: &'a [ComboRecord<'a>],
}

/// Committed semantic resource counts and minimum binary file size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceCounts {
    /// Stored nodes.
    pub nodes: u64,
    /// Stored combo rows.
    pub combos: u64,
    /// Menus, actions, history actions and probability/EV pairs.
    pub action_entries: u64,
    /// Stored UTF-8 string bytes.
    pub string_bytes: u64,
    /// Binary framing plus exact node bytes plus a header lower bound.
    pub minimum_encoded_bytes: u64,
}

struct StoredHeader<'a> {
    template: SpotHeaderV1<'static>,
    board: [u8; 5],
    board_len: usize,
    text: BudgetVec<'a, u8>,
    text_spans: [(usize, usize); 14],
    menus: BudgetVec<'a, BetSize>,
    menu_spans: [(usize, usize); 12],
}

impl<'a> StoredHeader<'a> {
    fn copy(budget: &'a ResourceBudget, limits: ResourceLimits, header: &SpotHeaderV1<'_>) -> Result<Self, FormatError> {
        let empty = SizeMenu { bets: &[], raises: &[] };
        let mut template = SpotHeaderV1 {
            spot_id: "", scenario_id: "", positions: [""; 2], range_labels: [""; 2],
            game: GameSpecV1 { board: &[], ranges: [""; 2], tree: TreeSpec { sizes: [[empty; 2]; 3], ..header.game.tree }, ..header.game },
            source: SourceRecord { producer_run_id: "", ..header.source },
            provenance: ProvenanceRecord { solver_revision: "", solver_version: "", producer_version: "", generated_at_utc: "", host: "" },
            ..*header
        };
        if template.game.tree.add_all_in_threshold == 0.0 { template.game.tree.add_all_in_threshold = 0.0; }
        if template.game.tree.force_all_in_threshold == 0.0 { template.game.tree.force_all_in_threshold = 0.0; }
        let mut value = Self {
            template, board: [0; 5], board_len: header.game.board.len(),
            text: BudgetVec::new(budget, limits)?, text_spans: [(0, 0); 14],
            menus: BudgetVec::new(budget, limits)?, menu_spans: [(0, 0); 12],
        };
        value.board[..value.board_len].copy_from_slice(header.game.board);
        let strings = header_strings(header);
        let string_count = strings.iter().try_fold(0usize, |n, s| n.checked_add(s.len()).ok_or(FormatError("resource arithmetic overflow")))?;
        value.text.reserve_len(string_count, string_count)?;
        let mut offset = 0;
        for (index, s) in strings.iter().enumerate() {
            value.text_spans[index] = (offset, s.len());
            for byte in s.bytes() { value.text.push_reserved(byte); }
            offset += s.len();
        }
        let count: usize = header.game.tree.sizes.iter().flatten().map(|m| m.bets.len() + m.raises.len()).sum();
        value.menus.reserve_len(count, count)?;
        offset = 0;
        for (index, menu) in header.game.tree.sizes.iter().flatten().flat_map(|m| [m.bets, m.raises]).enumerate() {
            value.menu_spans[index] = (offset, menu.len());
            for size in menu { value.menus.push_reserved(*size); }
            offset += menu.len();
        }
        Ok(value)
    }

    fn text(&self, index: usize) -> &str {
        let (offset, len) = self.text_spans[index];
        // Spans come exclusively from copies of valid UTF-8 strings.
        std::str::from_utf8(&self.text.as_slice()[offset..offset + len]).expect("private UTF-8 span")
    }

    fn view(&self) -> SpotHeaderV1<'_> {
        let mut result = self.template;
        result.spot_id = self.text(0); result.scenario_id = self.text(1);
        result.positions = [self.text(2), self.text(3)];
        result.range_labels = [self.text(4), self.text(5)];
        result.game.board = &self.board[..self.board_len];
        result.game.ranges = [self.text(6), self.text(7)];
        result.source.producer_run_id = self.text(8);
        result.provenance = ProvenanceRecord { solver_revision: self.text(9), solver_version: self.text(10), producer_version: self.text(11), generated_at_utc: self.text(12), host: self.text(13) };
        for street in 0..3 {
            for player in 0..2 {
                let index = (street * 2 + player) * 2;
                let (b, bl) = self.menu_spans[index]; let (r, rl) = self.menu_spans[index + 1];
                result.game.tree.sizes[street][player] = SizeMenu { bets: &self.menus.as_slice()[b..b + bl], raises: &self.menus.as_slice()[r..r + rl] };
            }
        }
        result
    }
}

struct StoredNode<'a> {
    template: NodeRecord<'static>,
    board: [u8; 5],
    board_len: usize,
    history: BudgetVec<'a, HistoryEvent>,
    actions: BudgetVec<'a, Action>,
    combos: BudgetVec<'a, ComboRecord<'static>>,
    probabilities: BudgetVec<'a, u16>,
    evs: BudgetVec<'a, Option<i16>>,
}

impl<'a> StoredNode<'a> {
    fn copy(budget: &'a ResourceBudget, limits: ResourceLimits, node: &NodeRecord<'_>) -> Result<Self, FormatError> {
        let mut result = Self {
            template: NodeRecord { history: &[], board: &[], actions: &[], combos: &[], ..*node },
            board: [0; 5], board_len: node.board.len(),
            history: BudgetVec::new(budget, limits)?, actions: BudgetVec::new(budget, limits)?,
            combos: BudgetVec::new(budget, limits)?, probabilities: BudgetVec::new(budget, limits)?, evs: BudgetVec::new(budget, limits)?,
        };
        result.board[..result.board_len].copy_from_slice(node.board);
        result.history.copy_from(node.history)?;
        result.actions.copy_from(node.actions)?;
        result.combos.reserve_len(node.combos.len(), node.combos.len())?;
        let entries = node.combos.len().checked_mul(node.actions.len()).ok_or(FormatError("resource arithmetic overflow"))?;
        result.probabilities.reserve_len(entries, entries)?;
        result.evs.reserve_len(entries, entries)?;
        for combo in node.combos {
            result.combos.push_reserved(ComboRecord { probabilities: &[], action_evs: &[], ..*combo });
            for p in combo.probabilities { result.probabilities.push_reserved(*p); }
            for ev in combo.action_evs { result.evs.push_reserved(*ev); }
        }
        Ok(result)
    }

    fn metadata(&self) -> NodeRecord<'_> {
        NodeRecord { board: &self.board[..self.board_len], history: self.history.as_slice(), actions: self.actions.as_slice(), ..self.template }
    }

    fn combo(&self, index: usize) -> ComboRecord<'_> {
        let count = self.actions.as_slice().len();
        let begin = index * count;
        ComboRecord { probabilities: &self.probabilities.as_slice()[begin..begin + count], action_evs: &self.evs.as_slice()[begin..begin + count], ..self.combos.as_slice()[index] }
    }
}

/// Borrowed node view. Combo rows borrow the same immutable storage.
#[derive(Clone, Copy)]
pub struct NodeView<'a> { node: &'a StoredNode<'a> }

/// Borrowed decision metadata; combo rows are available separately on `NodeView`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeMetadata<'a> {
    /// Expanded node ID.
    pub node_id: u32,
    /// Compact source node ID.
    pub compact_id: u32,
    /// Events preceding this decision.
    pub history: &'a [HistoryEvent],
    /// Ordered board.
    pub board: &'a [u8],
    /// Decision street.
    pub street: Street,
    /// Acting player.
    pub player: u8,
    /// Exact ordered actions.
    pub actions: &'a [Action],
    /// Commitments from the root.
    pub contributions: [u64; 2],
    /// Positive finite EV scale.
    pub ev_scale: f32,
    /// Probability error maximum claim.
    pub max_probability_error: f64,
    /// EV error maximum claim.
    pub max_ev_error: f64,
    /// Identity runout mapping.
    pub mapping: Mapping,
}
impl<'a> NodeView<'a> {
    /// Scalar fields and borrowed history, board and actions.
    pub fn metadata(self) -> NodeMetadata<'a> {
        let n = self.node.metadata();
        NodeMetadata { node_id: n.node_id, compact_id: n.compact_id, history: n.history, board: n.board, street: n.street, player: n.player, actions: n.actions, contributions: n.contributions, ev_scale: n.ev_scale, max_probability_error: n.max_probability_error, max_ev_error: n.max_ev_error, mapping: n.mapping }
    }
    /// Increasing combo rows, with their borrowed probability/EV slices.
    pub fn combos(self) -> impl ExactSizeIterator<Item = ComboRecord<'a>> {
        (0..self.node.combos.as_slice().len()).map(move |i| self.node.combo(i))
    }
}

/// Incremental structural builder. Input DTO allocations remain caller-owned.
/// Failed insertions publish no record and leave semantic counters unchanged.
pub struct SpotBuilder<'a> {
    header: StoredHeader<'a>,
    nodes: BudgetVec<'a, StoredNode<'a>>,
    // Range payload precedes its temporary lease, including constructor failure.
    ranges: [Range; 2],
    range_lease: Lease<'a>,
    budget: &'a ResourceBudget,
    limits: ResourceLimits,
    counts: ResourceCounts,
    maxima: [f64; 3],
    // Prices inline builder/output/header wrappers, including temporary moves.
    housekeeping: Lease<'a>,
}

impl<'a> SpotBuilder<'a> {
    /// Validate and copy a borrowed header after all resource admissions.
    pub fn new(budget: &'a ResourceBudget, limits: &ResourceLimits, header: &SpotHeaderV1<'_>) -> Result<Self, FormatError> {
        limits.validate()?;
        let counts = validate_header(header, *limits)?;
        let housekeeping = budget.reserve((size_of::<Self>() + size_of::<ValidatedSpot<'_>>() + size_of::<StoredHeader<'_>>()) as u64, (size_of::<Self>() + size_of::<ValidatedSpot<'_>>() + size_of::<StoredHeader<'_>>()) as u64, limits.live_bytes, limits.retained_bytes)?;
        // Range parser: bounded 1326 canonical tokens, token-vector spare
        // capacity, expansion scratch, error text and Display's two numbers.
        // Four inline ranges cover returned values and transient moves/copies.
        let range_bytes = (4 * size_of::<Range>() + 4096 * size_of::<&str>() + 1326 * size_of::<Combo>() + 4096) as u64;
        let range_lease = budget.reserve(range_bytes, 0, limits.live_bytes, limits.retained_bytes)?;
        let ranges = parse_ranges(header.game)?;
        let stored = StoredHeader::copy(budget, *limits, header)?;
        let nodes = BudgetVec::new(budget, *limits)?;
        Ok(Self { header: stored, nodes, ranges, range_lease, budget, limits: *limits, counts, maxima: [0.0; 3], housekeeping })
    }

    /// Counts of successfully committed content only.
    pub const fn counts(&self) -> ResourceCounts { self.counts }

    /// Validate before copying; preserve sorted IDs even for out-of-order input.
    pub fn try_push_node(&mut self, node: &NodeRecord<'_>) -> Result<(), FormatError> {
        let header = self.header.view();
        let (next, maxima) = validate_node(&header, node, &self.ranges, self.counts, self.limits)?;
        let at = match self.nodes.as_slice().binary_search_by_key(&node.node_id, |n| n.template.node_id) {
            Ok(_) => return Err(FormatError("duplicate node ID")), Err(at) => at,
        };
        // A bounded scan has no hidden index allocation. Same node context with
        // disjoint combos is permitted; only complete overlapping keys refuse.
        for existing in self.nodes.as_slice() {
            if same_context(&existing.metadata(), node) && existing.combos.as_slice().iter().any(|c| node.combos.binary_search_by_key(&c.combo_id, |n| n.combo_id).is_ok()) {
                return Err(FormatError("duplicate full lookup key"));
            }
        }
        let scratch = self.budget.reserve(size_of::<StoredNode<'_>>() as u64, 0, self.limits.live_bytes, self.limits.retained_bytes)?;
        let owned = StoredNode::copy(self.budget, self.limits, node)?;
        // The newly copied node must drop before its stack-wrapper reservation
        // if admission of the outer vector fails.
        let pending = (owned, scratch);
        let count = usize::try_from(next.nodes).map_err(|_| FormatError("node count is not representable"))?;
        let ceiling = usize::try_from(self.limits.nodes).map_err(|_| FormatError("node ceiling is not representable"))?;
        self.nodes.reserve_len(count, ceiling)?;
        self.nodes.insert_reserved(at, pending.0);
        self.counts = next;
        for (current, value) in self.maxima.iter_mut().zip(maxima) { *current = current.max(value); }
        Ok(())
    }

    /// Check aggregate error maxima and finish immutable, untrusted storage.
    pub fn finish(self) -> Result<ValidatedSpot<'a>, FormatError> {
        let q = self.header.template.quantization;
        if self.maxima != [q.max_probability_error, q.max_ev_error, q.max_reach_error] {
            return Err(FormatError("header quantization maxima do not match records"));
        }
        let Self { header, nodes, range_lease, counts, housekeeping, .. } = self;
        // Ranges contain no heap payload and are no longer used after finish.
        drop(range_lease);
        Ok(ValidatedSpot { header, nodes, counts, housekeeping })
    }
}

/// Immutable structural data. All provenance, accuracy and completeness remain
/// untrusted claims. No API promotes an imported claim to a capture certificate.
pub struct ValidatedSpot<'a> {
    header: StoredHeader<'a>,
    nodes: BudgetVec<'a, StoredNode<'a>>,
    counts: ResourceCounts,
    housekeeping: Lease<'a>,
}

impl ValidatedSpot<'_> {
    /// Borrow the complete preserved header.
    pub fn header(&self) -> SpotHeaderV1<'_> { self.header.view() }
    /// Borrow decisions in increasing node-ID order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = NodeView<'_>> {
        self.nodes.as_slice().iter().map(|node| NodeView { node })
    }
    /// Committed resource counts.
    pub const fn counts(&self) -> ResourceCounts { self.counts }
    /// Conservative inline storage charge retained by this result.
    pub fn housekeeping_bytes(&self) -> u64 { self.housekeeping.retained_bytes() }
    /// Exact canonical-game and decision lookup. No approximate translation.
    pub fn lookup(&self, game: &GameSpecV1<'_>, key: &LookupKey<'_>) -> Result<ComboRecord<'_>, UncoveredReason> {
        let header = self.header.view();
        if header.game != *game { return Err(UncoveredReason::DifferentGame); }
        if key.suit_permutation != [0, 1, 2, 3] { return Err(UncoveredReason::UnsupportedMapping); }
        if !inside_cut(header.coverage, header.game.tree.start_street, key.board.len(), key.history) { return Err(UncoveredReason::OutsideCut); }
        let mut found_context = false;
        for node in self.nodes.as_slice() {
            let n = node.metadata();
            if n.history == key.history && n.board == key.board && n.player == key.player && n.actions == key.actions {
                found_context = true;
                if let Ok(index) = node.combos.as_slice().binary_search_by_key(&key.combo_id, |c| c.combo_id) { return Ok(node.combo(index)); }
            }
        }
        Err(if found_context { UncoveredReason::MissingCombo } else { UncoveredReason::MissingNode })
    }
}

/// Full exact lookup key, including the requested runout mapping.
#[derive(Clone, Copy, Debug)]
pub struct LookupKey<'a> {
    /// Ordered preceding events.
    pub history: &'a [HistoryEvent],
    /// Ordered public cards.
    pub board: &'a [u8],
    /// Acting player.
    pub player: u8,
    /// Private combo ID.
    pub combo_id: u16,
    /// Exact ordered action menu.
    pub actions: &'a [Action],
    /// Requested original-to-representative permutation.
    pub suit_permutation: [u8; 4],
}

/// Explicit reason a lookup cannot provide a stored row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UncoveredReason {
    /// Canonical game inputs differ.
    DifferentGame,
    /// The requested decision is outside the declared cut.
    OutsideCut,
    /// Version 1 does not support this permutation.
    UnsupportedMapping,
    /// No exact decision context is stored.
    MissingNode,
    /// The exact decision exists but the private combo is missing.
    MissingCombo,
}

fn header_strings<'a>(h: &SpotHeaderV1<'a>) -> [&'a str; 14] {
    [h.spot_id, h.scenario_id, h.positions[0], h.positions[1], h.range_labels[0], h.range_labels[1], h.game.ranges[0], h.game.ranges[1], h.source.producer_run_id, h.provenance.solver_revision, h.provenance.solver_version, h.provenance.producer_version, h.provenance.generated_at_utc, h.provenance.host]
}

fn nonnegative(value: f64) -> bool { FiniteF64::new(value).is_ok() && value >= 0.0 }
fn positive(value: f64) -> bool { FiniteF64::new(value).is_ok() && value > 0.0 }
fn board_mask(board: &[u8]) -> Result<u64, FormatError> {
    if !(3..=5).contains(&board.len()) { return Err(FormatError("invalid board length")); }
    let mut mask = 0;
    for &id in board {
        let card = Card::from_id(id).map_err(|_| FormatError("invalid card ID"))?;
        if mask & card.mask() != 0 { return Err(FormatError("duplicate board card")); }
        mask |= card.mask();
    }
    Ok(mask)
}

fn validate_header(h: &SpotHeaderV1<'_>, limits: ResourceLimits) -> Result<ResourceCounts, FormatError> {
    if h.version != 1 { return Err(FormatError("unsupported spot version")); }
    if matches!(h.coverage, Coverage::ActionDepth(n) if !(1..=128).contains(&n)) { return Err(FormatError("invalid coverage depth")); }
    let mut string_bytes = 0;
    for (i, text) in header_strings(h).iter().enumerate() {
        let ceiling = if i == 6 || i == 7 { 131_072 } else { 4096 };
        if text.is_empty() || text.len() > ceiling { return Err(FormatError("invalid string length")); }
        string_bytes = add(string_bytes, text.len() as u64)?;
    }
    if string_bytes > limits.string_bytes { return Err(FormatError("string byte limit exceeded")); }
    board_mask(h.game.board)?;
    let tree = h.game.tree;
    if h.game.board.len() != tree.start_street.index() + 3 { return Err(FormatError("root board disagrees with street")); }
    if !positive(h.game.chips_per_big_blind) || tree.starting_pot == 0 || tree.starting_pot > 1_000_000_000 || tree.effective_stack > 1_000_000_000 || tree.min_bet == 0 || tree.min_bet > 1_000_000_000 || tree.max_raises > 32 || !(1..=1_000_000).contains(&tree.max_nodes) || !nonnegative(tree.add_all_in_threshold) || !nonnegative(tree.force_all_in_threshold) {
        return Err(FormatError("invalid canonical game scalar"));
    }
    let mut actions = 0;
    for menu in tree.sizes.iter().flatten() {
        for (raise, sizes) in [(false, menu.bets), (true, menu.raises)] {
            if sizes.len() > 64 { return Err(FormatError("size menu limit exceeded")); }
            actions = add(actions, sizes.len() as u64)?;
            for (i, size) in sizes.iter().enumerate() {
                let valid = match size { BetSize::Pot(v) => positive(*v), BetSize::PreviousBet(v) => raise && positive(*v) && *v > 1.0, BetSize::Additive(v) => (1..=1_000_000_000).contains(v), BetSize::AllIn => true };
                if !valid { return Err(FormatError("invalid bet size")); }
                if sizes[..i].contains(size) { return Err(FormatError("duplicate bet size")); }
            }
        }
    }
    if actions > limits.action_entries { return Err(FormatError("action entry limit exceeded")); }
    if !valid_timestamp(h.provenance.generated_at_utc) { return Err(FormatError("invalid UTC calendar timestamp")); }
    validate_source(h.source, tree.starting_pot)?;
    let q = h.quantization;
    if !nonnegative(q.max_probability_error) || q.max_probability_error > 1.0 / 65535.0 || !nonnegative(q.max_ev_error) || !nonnegative(q.max_reach_error) || q.max_reach_error > 2f64.powi(-25) { return Err(FormatError("invalid header quantization error")); }
    // Every UTF-8 source byte and menu entry requires at least one encoded byte.
    // This is deliberately only a lower bound for the future JSON header codec.
    let header_minimum = add(string_bytes, actions)?;
    if header_minimum > limits.header_bytes { return Err(FormatError("minimum header byte limit exceeded")); }
    let minimum_encoded_bytes = add(28, header_minimum)?;
    if minimum_encoded_bytes > limits.encoded_bytes { return Err(FormatError("minimum encoded byte limit exceeded")); }
    Ok(ResourceCounts { nodes: 0, combos: 0, action_entries: actions, string_bytes, minimum_encoded_bytes })
}

fn validate_source(s: SourceRecord<'_>, pot: u64) -> Result<(), FormatError> {
    if s.attempt.is_some_and(|a| a.solver == 0 || a.generation == 0) || !nonnegative(s.target_pct_of_pot) || s.max_iterations == 0 || s.check_every == 0 || !positive(s.root_compatible_mass) { return Err(FormatError("invalid source metadata")); }
    if let Variant::Discounted { alpha, beta, gamma } = s.variant
        && ![alpha, beta, gamma].into_iter().all(nonnegative) {
        return Err(FormatError("invalid discount exponent"));
    }
    if let Some(root) = s.root_values
        && (root.iteration != s.policy_iteration || !root.values.into_iter().all(f64::is_finite)) {
        return Err(FormatError("invalid root value claim"));
    }
    if let Some(m) = s.measurement {
        if m.iteration > s.policy_iteration || !m.br_values.into_iter().all(f64::is_finite) || ![m.nash_conv, m.average, m.pct_of_pot].into_iter().all(nonnegative) { return Err(FormatError("invalid measurement claim")); }
        let [a, b] = m.br_values;
        let raw = a + b;
        let scale = a.abs().max(b.abs()).max(1.0);
        let left = a / scale; let right = b / scale;
        let normalized = (left + right) / (1.0 / scale + left.abs() + right.abs());
        // Mirrors postflop/src/error.rs::normalized_sum and the reporting
        // operations in postflop/src/best_response.rs, without a solver edge.
        if !raw.is_finite() || normalized < -1e-10 { return Err(FormatError("invalid best-response sum")); }
        let nash = raw.max(0.0);
        let pct = (nash / pot as f64) * 50.0;
        if !pct.is_finite() || (nash > 0.0 && pct == 0.0) { return Err(FormatError("accuracy conversion overflow or underflow")); }
        if m.nash_conv != nash || m.average != nash / 2.0 || m.pct_of_pot != pct { return Err(FormatError("inconsistent measurement metrics")); }
    }
    match s.stop_reason {
        StopReason::Cancelled => {},
        StopReason::TargetReached | StopReason::IterationCap => {
            let m = s.measurement.ok_or(FormatError("completion requires a fresh measurement"))?;
            if m.iteration != s.policy_iteration { return Err(FormatError("completion requires a fresh measurement")); }
            if s.stop_reason == StopReason::TargetReached {
                if m.pct_of_pot > s.target_pct_of_pot { return Err(FormatError("target report exceeds target")); }
            } else if s.policy_iteration < s.max_iterations || m.pct_of_pot <= s.target_pct_of_pot { return Err(FormatError("invalid iteration-cap report")); }
        },
    }
    Ok(())
}

fn valid_timestamp(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' { return false; }
    let number = |slice: &[u8]| -> Option<u32> { slice.iter().try_fold(0, |n, &c| if c.is_ascii_digit() { Some(n * 10 + u32::from(c - b'0')) } else { None }) };
    let parts = [number(&b[..4]), number(&b[5..7]), number(&b[8..10]), number(&b[11..13]), number(&b[14..16]), number(&b[17..19])];
    let [Some(y), Some(m), Some(d), Some(h), Some(min), Some(s)] = parts else { return false; };
    if y == 0 || !(1..=12).contains(&m) || h > 23 || min > 59 || s > 59 { return false; }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m { 2 if leap => 29, 2 => 28, 4 | 6 | 9 | 11 => 30, _ => 31 };
    (1..=days).contains(&d)
}

fn parse_ranges(game: GameSpecV1<'_>) -> Result<[Range; 2], FormatError> {
    // Canonical form names each positive combo once; reject excess tokens before
    // entering Range's separately bounded parser.
    for text in game.ranges {
        if text.bytes().filter(|&b| b == b',').count() >= 1326 { return Err(FormatError("canonical range token limit exceeded")); }
        // Canonical explicit combos plus a shortest finite weight need fewer
        // than 64 ASCII bytes. This rejects whitespace token expansion and
        // bounds the parser's owned error token before calling it.
        if text.split(',').any(|token| token.len() > 64 || token.bytes().any(|b| !b.is_ascii() || b.is_ascii_whitespace())) {
            return Err(FormatError("range text is not canonical"));
        }
    }
    let mut ranges = [Range::parse(game.ranges[0]).map_err(|_| FormatError("invalid canonical range"))?, Range::parse(game.ranges[1]).map_err(|_| FormatError("invalid canonical range"))?];
    struct Compare<'a> { remaining: &'a str }
    impl Write for Compare<'_> {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.remaining = self.remaining.strip_prefix(text).ok_or(std::fmt::Error)?;
            Ok(())
        }
    }
    let mask = board_mask(game.board)?;
    for (range, text) in ranges.iter_mut().zip(game.ranges) {
        let mut compare = Compare { remaining: text };
        if write!(compare, "{range}").is_err() || !compare.remaining.is_empty() { return Err(FormatError("range text is not canonical")); }
        let maximum = Combo::all().filter(|c| c.mask() & mask == 0).map(|c| range.weight(c)).fold(0.0, f64::max);
        if maximum <= 0.0 { return Err(FormatError("range is empty after root blockers")); }
        for combo in Combo::all() {
            let weight = if combo.mask() & mask == 0 { range.weight(combo) / maximum } else { 0.0 };
            range.set_weight(combo, weight).map_err(|_| FormatError("invalid scaled range"))?;
        }
    }
    let mut compatible = 0.0;
    for a in Combo::all().filter(|c| ranges[0].weight(*c) > 0.0) {
        for b in Combo::all().filter(|c| ranges[1].weight(*c) > 0.0 && c.mask() & a.mask() == 0) {
            let product = ranges[0].weight(a) * ranges[1].weight(b);
            if product == 0.0 { return Err(FormatError("compatible range product underflow")); }
            compatible += product;
        }
    }
    if !positive(compatible) { return Err(FormatError("empty or nonfinite compatible range mass")); }
    Ok(ranges)
}

fn action_bytes(action: Action, stack: u64) -> Result<u64, FormatError> {
    match action {
        Action::Fold | Action::Check | Action::Call => Ok(1),
        Action::Bet(to) | Action::Raise(to) | Action::AllIn(to) if to > 0 && to <= stack => Ok(9),
        _ => Err(FormatError("invalid action commitment")),
    }
}

fn inside_cut(coverage: Coverage, root: Street, board_len: usize, history: &[HistoryEvent]) -> bool {
    match coverage {
        Coverage::StartStreet => board_len == root.index() + 3,
        Coverage::ActionDepth(depth) => history.iter().filter(|e| matches!(e, HistoryEvent::Action { .. })).count() < usize::from(depth),
    }
}

fn same_context(a: &NodeRecord<'_>, b: &NodeRecord<'_>) -> bool {
    a.history == b.history && a.board == b.board && a.player == b.player && a.actions == b.actions
}

fn validate_node(h: &SpotHeaderV1<'_>, n: &NodeRecord<'_>, ranges: &[Range; 2], counts: ResourceCounts, limits: ResourceLimits) -> Result<(ResourceCounts, [f64; 3]), FormatError> {
    if n.history.len() > 128 || n.actions.is_empty() || n.actions.len() > 255 || n.combos.is_empty() || n.combos.len() > 1326 { return Err(FormatError("node collection limit exceeded")); }
    let node_count = add(counts.nodes, 1)?;
    let combos = add(counts.combos, n.combos.len() as u64)?;
    let pairs = (n.combos.len() as u64).checked_mul(n.actions.len() as u64).ok_or(FormatError("resource arithmetic overflow"))?;
    let mut actions = add(add(counts.action_entries, n.actions.len() as u64)?, pairs)?;
    if node_count > limits.nodes || combos > limits.combos { return Err(FormatError("cumulative node or combo limit exceeded")); }
    if n.player > 1 { return Err(FormatError("invalid acting player")); }
    let mask = board_mask(n.board)?;
    if !n.board.starts_with(h.game.board) || n.board.len() != n.street.index() + 3 { return Err(FormatError("node board disagrees with root or street")); }
    let mut board_count = h.game.board.len();
    // Fixed fields in the specified binary node payload, excluding board,
    // history event, action and combo payloads: 60 bytes.
    let mut encoded = add(60, n.board.len() as u64)?;
    for event in n.history {
        match *event {
            HistoryEvent::Action { player, action } => {
                if player > 1 { return Err(FormatError("invalid history player")); }
                actions = add(actions, 1)?;
                encoded = add(encoded, add(2, action_bytes(action, h.game.tree.effective_stack)?)?)?;
            },
            HistoryEvent::Deal { card } => {
                if board_count >= n.board.len() || n.board[board_count] != card { return Err(FormatError("history deals disagree with board")); }
                board_count += 1;
                encoded = add(encoded, 2)?;
            },
        }
    }
    if board_count != n.board.len() { return Err(FormatError("history deals disagree with board")); }
    if !inside_cut(h.coverage, h.game.tree.start_street, n.board.len(), n.history) { return Err(FormatError("node is outside declared coverage")); }
    if actions > limits.action_entries { return Err(FormatError("action entry limit exceeded")); }
    for (i, &action) in n.actions.iter().enumerate() {
        encoded = add(encoded, action_bytes(action, h.game.tree.effective_stack)?)?;
        if i > 0 && n.actions[i - 1] >= action { return Err(FormatError("node actions are not unique and ordered")); }
    }
    if n.contributions.iter().any(|&v| v > h.game.tree.effective_stack) || FiniteF32::new(n.ev_scale).is_err() || n.ev_scale <= 0.0 || !nonnegative(n.max_probability_error) || n.max_probability_error > 1.0 / 65535.0 || !nonnegative(n.max_ev_error) || n.max_ev_error > f64::from(n.ev_scale) / 2.0 { return Err(FormatError("invalid node scalar or error bound")); }
    let mut suits = 0u8;
    for suit in n.mapping.suit_permutation {
        if suit > 3 || suits & (1 << suit) != 0 { return Err(FormatError("invalid suit permutation")); }
        suits |= 1 << suit;
    }
    if n.mapping.suit_permutation != [0, 1, 2, 3] || n.mapping.representative_node != n.node_id { return Err(FormatError("unsupported runout mapping")); }
    let mut max_reach: f64 = 0.0;
    for (i, c) in n.combos.iter().enumerate() {
        let combo = Combo::from_id(c.combo_id).map_err(|_| FormatError("invalid combo ID"))?;
        if i > 0 && n.combos[i - 1].combo_id >= c.combo_id { return Err(FormatError("combos are not unique and increasing")); }
        if combo.mask() & mask != 0 || ranges[usize::from(n.player)].weight(combo) <= 0.0 { return Err(FormatError("combo is blocked or outside acting range")); }
        if c.probabilities.len() != n.actions.len() || c.action_evs.len() != n.actions.len() { return Err(FormatError("combo row length mismatch")); }
        if c.probabilities.iter().map(|&v| u32::from(v)).sum::<u32>() != 65535 { return Err(FormatError("probabilities do not sum to 65535")); }
        if FiniteF32::new(c.own_reach).is_err() || !(0.0..=1.0).contains(&c.own_reach) || !nonnegative(c.reach_error) || c.reach_error > 2f64.powi(-25) || !nonnegative(c.opponent_mass) { return Err(FormatError("invalid reach or opponent mass")); }
        let reachable = c.own_reach > 0.0 && c.opponent_mass > 0.0;
        if c.action_evs.iter().any(|&ev| ev == Some(i16::MIN) || ev.is_some() != reachable) { return Err(FormatError("EV presence disagrees with reach or uses sentinel")); }
        max_reach = max_reach.max(c.reach_error);
        encoded = add(encoded, add(22, 4 * n.actions.len() as u64)?)?;
    }
    if encoded > limits.node_bytes { return Err(FormatError("node byte limit exceeded")); }
    let minimum_encoded_bytes = add(counts.minimum_encoded_bytes, add(4, encoded)?)?;
    if minimum_encoded_bytes > limits.encoded_bytes { return Err(FormatError("minimum encoded byte limit exceeded")); }
    Ok((ResourceCounts { nodes: node_count, combos, action_entries: actions, minimum_encoded_bytes, ..counts }, [n.max_probability_error, n.max_ev_error, max_reach]))
}
