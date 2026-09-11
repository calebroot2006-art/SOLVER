//! Structural, source-claim and aggregate admission regressions.

use cards::Combo;
use spots::format::*;
use spots::resource::{MemoryClass, ResourceBudget, ResourceLimits};

fn limits() -> ResourceLimits {
    ResourceLimits {
        live_bytes: 8_000_000, retained_bytes: 4_000_000,
        encoded_bytes: 4_000_000, header_bytes: 500_000, node_bytes: 1_000_000,
        nesting: 32, nodes: 100, combos: 10_000, action_entries: 100_000,
        string_bytes: 300_000,
    }
}

fn header() -> SpotHeaderV1<'static> {
    let empty = SizeMenu { bets: &[], raises: &[] };
    SpotHeaderV1 {
        version: 1, spot_id: "spot", scenario_id: "scenario",
        positions: ["OOP", "IP"], range_labels: ["range 0", "range 1"],
        game: GameSpecV1 {
            board: &[0, 5, 10], ranges: ["5d5c:0.5,6d6c:0.5", "7d7c:0.5,8d8c:0.5"], chips_per_big_blind: 2.0,
            tree: TreeSpec {
                starting_pot: 100, effective_stack: 1000, min_bet: 2,
                start_street: Street::Flop, sizes: [[empty; 2]; 3], max_raises: 4,
                add_all_in_threshold: -0.0, force_all_in_threshold: -0.0, max_nodes: 100,
            },
        },
        coverage: Coverage::ActionDepth(3), payoff_model: PayoffModel::HeadsUpChipEvNoRake,
        ev_units: EvUnits::NetChipsFromFixedRootOrigin,
        accuracy_units: AccuracyUnits::NashconvChipsAndHalfAsRootPotPercent,
        uncovered_policy: UncoveredPolicy::Ungraded,
        source: SourceRecord {
            producer_run_id: "run", attempt: Some(Attempt { solver: 1, generation: 1 }),
            policy_iteration: 15, measurement: None, root_values: None,
            stop_reason: StopReason::Cancelled, target_pct_of_pot: -0.0,
            max_iterations: 10, check_every: 5, variant: Variant::Vanilla,
            precision: Precision::F64, elapsed_nanoseconds: 50, root_compatible_mass: 4.0,
        },
        provenance: ProvenanceRecord {
            solver_revision: "revision", solver_version: "0.1", producer_version: "0.1",
            generated_at_utc: "2024-02-29T01:02:03Z", host: "host",
        },
        quantization: Quantization {
            algorithm: QuantizationAlgorithm::LargestRemainderU16ScaledI16V1,
            max_probability_error: 0.0, max_ev_error: 0.0, max_reach_error: 0.0,
        },
    }
}

fn combo(text: &str) -> ComboRecord<'static> {
    ComboRecord {
        combo_id: text.parse::<Combo>().unwrap().id(), probabilities: &[65535],
        action_evs: &[Some(0)], own_reach: 1.0, reach_error: 0.0, opponent_mass: 2.0,
    }
}

fn node<'a>(id: u32, combos: &'a [ComboRecord<'a>]) -> NodeRecord<'a> {
    NodeRecord {
        node_id: id, compact_id: 0, history: &[], board: &[0, 5, 10],
        street: Street::Flop, player: 0, actions: &[Action::Check], contributions: [0; 2],
        ev_scale: 1.0, max_probability_error: 0.0, max_ev_error: 0.0,
        mapping: Mapping { representative_node: id, suit_permutation: [0, 1, 2, 3] }, combos,
    }
}

fn header_error(h: &SpotHeaderV1<'_>) -> &'static str {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let baseline = budget.snapshot();
    let error = SpotBuilder::new(&budget, &limits(), h).err().expect("must refuse");
    assert_eq!(budget.snapshot().live_bytes, baseline.live_bytes);
    assert_eq!(budget.snapshot().retained_bytes, baseline.retained_bytes);
    error.0
}

#[test]
fn borrowed_storage_sorts_ids_preserves_bits_and_provides_exact_rows() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let baseline = budget.snapshot();
    let mut builder = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
    let first = [combo("5c5d")]; let second = [combo("6c6d")];
    builder.try_push_node(&node(9, &second)).unwrap();
    builder.try_push_node(&node(3, &first)).unwrap();
    let spot = builder.finish().unwrap();
    assert_eq!(spot.nodes().map(|n| n.metadata().node_id).collect::<Vec<_>>(), vec![3, 9]);
    let h = spot.header();
    assert_eq!(h.game.tree.add_all_in_threshold.to_bits(), 0.0f64.to_bits());
    assert_eq!(h.game.tree.force_all_in_threshold.to_bits(), 0.0f64.to_bits());
    assert_eq!(h.source.target_pct_of_pot.to_bits(), (-0.0f64).to_bits());
    let key = LookupKey { history: &[], board: h.game.board, player: 0, combo_id: first[0].combo_id, actions: &[Action::Check], suit_permutation: [0, 1, 2, 3] };
    assert_eq!(spot.lookup(&h.game, &key).unwrap(), first[0]);
    assert_eq!(spot.lookup(&h.game, &LookupKey { combo_id: combo("7c7d").combo_id, ..key }), Err(UncoveredReason::MissingCombo));
    assert_eq!(spot.lookup(&h.game, &LookupKey { actions: &[Action::Call], ..key }), Err(UncoveredReason::MissingNode));
    assert_eq!(spot.lookup(&h.game, &LookupKey { suit_permutation: [1, 0, 2, 3], ..key }), Err(UncoveredReason::UnsupportedMapping));
    let history = [HistoryEvent::Action { player: 0, action: Action::Check }; 3];
    assert_eq!(spot.lookup(&h.game, &LookupKey { history: &history, ..key }), Err(UncoveredReason::OutsideCut));
    let other_game = GameSpecV1 { chips_per_big_blind: 3.0, ..h.game };
    assert_eq!(spot.lookup(&other_game, &key), Err(UncoveredReason::DifferentGame));
    assert_eq!(spot.nodes().next().unwrap().combos().next().unwrap(), first[0]);
    drop(spot);
    assert_eq!(budget.snapshot().live_bytes, baseline.live_bytes);
    assert_eq!(budget.snapshot().retained_bytes, baseline.retained_bytes);
}

#[test]
fn duplicate_ids_full_keys_and_combo_order_refuse_without_commit() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut builder = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
    let rows = [combo("5c5d")];
    builder.try_push_node(&node(1, &rows)).unwrap();
    let before = builder.counts(); let memory = budget.snapshot();
    assert_eq!(builder.try_push_node(&node(1, &rows)), Err(FormatError("duplicate node ID")));
    assert_eq!(builder.try_push_node(&node(2, &rows)), Err(FormatError("duplicate full lookup key")));
    let repeated = [rows[0], rows[0]];
    assert_eq!(builder.try_push_node(&node(3, &repeated)), Err(FormatError("combos are not unique and increasing")));
    let reversed = [combo("6c6d"), combo("5c5d")];
    assert!(builder.try_push_node(&node(3, &reversed)).is_err());
    assert_eq!(builder.counts(), before);
    assert_eq!(budget.snapshot().live_bytes, memory.live_bytes);
}

#[test]
fn off_path_and_zero_opponent_rows_keep_policy_with_all_evs_missing() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut builder = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
    let rows = [ComboRecord { own_reach: 0.0, action_evs: &[None], ..combo("5c5d") }, ComboRecord { opponent_mass: -0.0, action_evs: &[None], ..combo("6c6d") }];
    builder.try_push_node(&node(1, &rows)).unwrap();
    let spot = builder.finish().unwrap();
    let view = spot.nodes().next().unwrap();
    for row in view.combos() { assert_eq!(row.probabilities, &[65535]); assert_eq!(row.action_evs, &[None]); }
    assert_eq!(view.combos().nth(1).unwrap().opponent_mass.to_bits(), (-0.0f64).to_bits());
    for row in [ComboRecord { own_reach: 0.0, ..combo("5c5d") }, ComboRecord { action_evs: &[None], ..combo("5c5d") }, ComboRecord { action_evs: &[Some(i16::MIN)], ..combo("5c5d") }] {
        let mut b = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
        assert_eq!(b.try_push_node(&node(2, &[row])), Err(FormatError("EV presence disagrees with reach or uses sentinel")));
    }
}

#[test]
fn board_history_mapping_and_later_blockers_are_checked() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut builder = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
    let rows = [combo("5c5d")]; let base = node(1, &rows);
    let turn = NodeRecord { board: &[0, 5, 10, 12], street: Street::Turn, history: &[HistoryEvent::Deal { card: 12 }], ..base };
    assert_eq!(builder.try_push_node(&turn), Err(FormatError("combo is blocked or outside acting range")));
    let bad = NodeRecord { board: &[0, 5, 10, 14], ..turn };
    assert_eq!(builder.try_push_node(&bad), Err(FormatError("history deals disagree with board")));
    for (permutation, expected) in [([0, 0, 2, 3], "invalid suit permutation"), ([1, 0, 2, 3], "unsupported runout mapping")] {
        let bad = NodeRecord { mapping: Mapping { representative_node: 1, suit_permutation: permutation }, ..base };
        assert_eq!(builder.try_push_node(&bad), Err(FormatError(expected)));
    }
    assert_eq!(builder.try_push_node(&NodeRecord { mapping: Mapping { representative_node: 2, ..base.mapping }, ..base }), Err(FormatError("unsupported runout mapping")));
    let good = NodeRecord { board: &[0, 5, 10, 14], history: &[HistoryEvent::Deal { card: 14 }], ..turn };
    builder.try_push_node(&good).unwrap();
    let mut start = header(); start.coverage = Coverage::StartStreet;
    let mut b = SpotBuilder::new(&budget, &limits(), &start).unwrap();
    assert_eq!(b.try_push_node(&good), Err(FormatError("node is outside declared coverage")));
}

#[test]
fn scaled_ranges_handle_uniform_halves_blocked_maxima_and_smallest_weights() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    // The raw denominator claim stays untrusted: import cannot bind it to a
    // differently ordered local sum. Uniform halves are scaled to unit weights.
    let mut h = header(); h.source.root_compatible_mass = 0.25;
    let spot = SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    assert_eq!(spot.header().source.root_compatible_mass, 0.25);
    drop(spot);
    h.game.ranges = ["2d2c,5d5c:5e-324", "2h2c,7d7c:5e-324"];
    let spot = SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    assert_eq!(spot.header().game.ranges, h.game.ranges);
    drop(spot);
    h.game.ranges = ["5d5c:5e-324,6d6c", "7d7c:5e-324,8d8c"];
    assert_eq!(header_error(&h), "compatible range product underflow");
    h.game.ranges = ["5d5c", "5h5c"];
    assert_eq!(header_error(&h), "empty or nonfinite compatible range mass");
    h.game.ranges = ["2d2c", "7d7c"];
    assert_eq!(header_error(&h), "range is empty after root blockers");
    for invalid in ["55", "5c5d", "5d5c:0.50", "5d5c,5d5c", "5d5c 6d6c", "5d5c:NaN"] {
        h.game.ranges = [invalid, "7d7c"];
        assert!(matches!(header_error(&h), "range text is not canonical" | "invalid canonical range"));
    }
}

#[test]
fn source_reports_preserve_resumed_caps_and_derive_staleness() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut h = header();
    h.source.measurement = Some(Measurement { iteration: 15, br_values: [3.0, -1.0], nash_conv: 2.0, average: 1.0, pct_of_pot: 1.0 });
    h.source.stop_reason = StopReason::IterationCap;
    assert!(!h.source.measurement_is_stale());
    SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    h.source.measurement.as_mut().unwrap().iteration = 14;
    assert!(h.source.measurement_is_stale());
    assert_eq!(header_error(&h), "completion requires a fresh measurement");
    h.source.stop_reason = StopReason::Cancelled;
    SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    h.source.measurement.as_mut().unwrap().iteration = 16;
    assert_eq!(header_error(&h), "invalid measurement claim");
    h.source.measurement.as_mut().unwrap().iteration = 15;
    h.source.stop_reason = StopReason::TargetReached;
    assert_eq!(header_error(&h), "target report exceeds target");
    h.source.target_pct_of_pot = 1.0;
    SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    h.source.stop_reason = StopReason::IterationCap;
    assert_eq!(header_error(&h), "invalid iteration-cap report");
}

#[test]
fn source_metrics_enforce_exact_operations_and_checked_extremes() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut h = header();
    h.game.tree.starting_pot = 3;
    // This selected vector distinguishes the approved operation order from
    // multiplying by 50 before dividing by the pot.
    let nash = 0.01;
    let pct = (nash / 3.0) * 50.0;
    assert_ne!(pct, (nash * 50.0) / 3.0);
    h.source.measurement = Some(Measurement { iteration: 15, br_values: [nash, 0.0], nash_conv: nash, average: nash / 2.0, pct_of_pot: pct });
    SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    h.source.measurement.as_mut().unwrap().pct_of_pot = (nash * 50.0) / 3.0;
    assert_eq!(header_error(&h), "inconsistent measurement metrics");
    for (br, pot, expected) in [([f64::MAX, f64::MAX], 1, "invalid best-response sum"), ([f64::MAX, 0.0], 1, "accuracy conversion overflow or underflow"), ([f64::from_bits(1), 0.0], 1_000_000_000, "accuracy conversion overflow or underflow"), ([-1.0, 0.0], 100, "invalid best-response sum")] {
        h.game.tree.starting_pot = pot;
        h.source.measurement = Some(Measurement { iteration: 15, br_values: br, nash_conv: 0.0, average: 0.0, pct_of_pot: 0.0 });
        assert_eq!(header_error(&h), expected);
    }
    h.game.tree.starting_pot = 100;
    h.source.measurement = Some(Measurement { iteration: 15, br_values: [-1e-12, 0.0], nash_conv: -0.0, average: -0.0, pct_of_pot: -0.0 });
    let spot = SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().unwrap();
    assert_eq!(spot.header().source.measurement.unwrap().nash_conv.to_bits(), (-0.0f64).to_bits());
}

#[test]
fn malformed_header_fields_and_calendar_dates_refuse() {
    let mut h = header(); h.version = 2;
    assert_eq!(header_error(&h), "unsupported spot version"); h = header();
    h.game.tree.max_nodes = 0; assert_eq!(header_error(&h), "invalid canonical game scalar"); h = header();
    h.game.board = &[0, 0, 10]; assert_eq!(header_error(&h), "duplicate board card"); h = header();
    h.coverage = Coverage::ActionDepth(0); assert_eq!(header_error(&h), "invalid coverage depth"); h = header();
    h.source.attempt.as_mut().unwrap().solver = 0; assert_eq!(header_error(&h), "invalid source metadata"); h = header();
    h.source.variant = Variant::Discounted { alpha: 0.0, beta: -1.0, gamma: 0.0 }; assert_eq!(header_error(&h), "invalid discount exponent"); h = header();
    h.source.root_values = Some(RootValues { iteration: 14, values: [0.0; 2] }); assert_eq!(header_error(&h), "invalid root value claim"); h = header();
    for date in ["2023-02-29T00:00:00Z", "1900-02-29T00:00:00Z", "2024-04-31T00:00:00Z", "0000-01-01T00:00:00Z", "2024-01-01T24:00:00Z", "2024-01-01T00:00:60Z", "2024-01-01T00:00:00+00:00"] {
        h.provenance.generated_at_utc = date;
        assert_eq!(header_error(&h), "invalid UTC calendar timestamp");
    }
    h = header();
    h.game.tree.sizes[0][0].bets = &[BetSize::PreviousBet(2.0)]; assert_eq!(header_error(&h), "invalid bet size");
    h.game.tree.sizes[0][0].bets = &[BetSize::Pot(0.5), BetSize::Pot(0.5)]; assert_eq!(header_error(&h), "duplicate bet size");
}

#[test]
fn cumulative_counts_include_menus_history_actions_and_pairs() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut h = header(); h.game.tree.sizes[2][1].raises = &[BetSize::Pot(0.5), BetSize::AllIn];
    let mut l = limits(); l.action_entries = 5; l.combos = 2;
    let mut b = SpotBuilder::new(&budget, &l, &h).unwrap();
    assert_eq!(b.counts().action_entries, 2);
    let rows = [combo("5c5d")];
    let n = NodeRecord { history: &[HistoryEvent::Action { player: 1, action: Action::Check }], ..node(1, &rows) };
    b.try_push_node(&n).unwrap();
    assert_eq!(b.counts().action_entries, 5);
    let before = b.counts(); let memory = budget.snapshot();
    assert_eq!(b.try_push_node(&node(2, &[combo("6c6d")])), Err(FormatError("action entry limit exceeded")));
    assert_eq!(b.counts(), before); assert_eq!(budget.snapshot().live_bytes, memory.live_bytes);
    assert!(b.finish().is_ok());
}

#[test]
fn exact_binary_node_size_and_all_semantic_limits_precede_copy() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let rows = [combo("5c5d")]; let n = node(1, &rows);
    // 60 fixed + 3 board + 1 check + 22 combo prefix + 4 pair = 90.
    let mut l = limits(); l.node_bytes = 89;
    let mut b = SpotBuilder::new(&budget, &l, &header()).unwrap();
    assert_eq!(b.try_push_node(&n), Err(FormatError("node byte limit exceeded")));
    drop(b); l.node_bytes = 90;
    let mut b = SpotBuilder::new(&budget, &l, &header()).unwrap();
    let before = b.counts(); b.try_push_node(&n).unwrap();
    assert_eq!(b.counts().minimum_encoded_bytes - before.minimum_encoded_bytes, 94);
    drop(b);
    for field in 0..4 {
        let mut l = limits();
        match field { 0 => l.nodes = 0, 1 => l.combos = 0, 2 => l.action_entries = 0, _ => l.encoded_bytes = 1 }
        if let Ok(mut b) = SpotBuilder::new(&budget, &l, &header()) { assert!(b.try_push_node(&n).is_err()); }
    }
    let mut l = limits(); l.nodes = 4_000_001;
    assert_eq!(SpotBuilder::new(&budget, &l, &header()).err().unwrap().0, "resource limit exceeds a schema ceiling");
    let mut l = limits(); l.live_bytes = u64::MAX; l.retained_bytes = u64::MAX; l.string_bytes = u64::MAX;
    SpotBuilder::new(&budget, &l, &header()).unwrap().finish().unwrap();
}

#[test]
fn shared_budget_refusals_rollback_partial_node_copies_and_constructor() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let baseline = budget.snapshot();
    let mut b = SpotBuilder::new(&budget, &limits(), &header()).unwrap();
    let used = budget.snapshot();
    // Leave space for the stack wrapper and some payloads, but not every arena.
    let external = budget.reserve_external(8_000_000 - used.live_bytes - 900, MemoryClass::Temporary).unwrap();
    let before = budget.snapshot();
    let rows = [ComboRecord { probabilities: &[32768, 32767], action_evs: &[Some(0), Some(1)], ..combo("5c5d") }];
    let n = NodeRecord { actions: &[Action::Check, Action::Bet(10)], ..node(1, &rows) };
    assert!(b.try_push_node(&n).is_err());
    assert_eq!(b.counts().nodes, 0);
    assert_eq!(budget.snapshot().live_bytes, before.live_bytes);
    assert_eq!(budget.snapshot().retained_bytes, before.retained_bytes);
    drop(external);
    b.try_push_node(&n).unwrap();
    let spot = b.finish().unwrap();
    let retained = budget.snapshot();
    let hold = budget.reserve_external(8_000_000 - retained.live_bytes - 100, MemoryClass::Temporary).unwrap();
    assert!(SpotBuilder::new(&budget, &limits(), &header()).is_err());
    assert_eq!(budget.snapshot().live_bytes, 7_999_900);
    drop(hold); drop(spot);
    assert_eq!(budget.snapshot().live_bytes, baseline.live_bytes);
    assert_eq!(budget.snapshot().retained_bytes, baseline.retained_bytes);
}

#[test]
fn maxima_are_claims_checked_exactly_at_finish_including_empty_spots() {
    let budget = ResourceBudget::new(8_000_000, 4_000_000).unwrap();
    let mut h = header(); h.quantization.max_ev_error = 0.25;
    assert_eq!(SpotBuilder::new(&budget, &limits(), &h).unwrap().finish().err().unwrap().0, "header quantization maxima do not match records");
    h.quantization.max_probability_error = 1.0 / 65535.0;
    h.quantization.max_reach_error = 2f64.powi(-25);
    let mut b = SpotBuilder::new(&budget, &limits(), &h).unwrap();
    let rows = [ComboRecord { reach_error: h.quantization.max_reach_error, ..combo("5c5d") }];
    let n = NodeRecord { max_probability_error: h.quantization.max_probability_error, max_ev_error: 0.25, ..node(1, &rows) };
    b.try_push_node(&n).unwrap(); b.finish().unwrap();
    assert!(FiniteF64::new(f64::NAN).is_err()); assert!(FiniteF32::new(f32::INFINITY).is_err());
    assert_eq!(FiniteF64::new(-0.0).unwrap().get().to_bits(), (-0.0f64).to_bits());
}
