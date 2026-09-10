//! The exact working-set table for the phase 4 gate trees.
//!
//! Step 5c of `docs/phase-4/PLAN.md`, which answers Astra's finding R5: one row
//! per buffer, with its representation, bytes, lifetime and whether it overlaps
//! another row, for the approved turn gate and flop gate trees (Decisions 1, 9
//! and 10) at three storage widths.
//!
//! Only `f64` is implemented. The `f32` and `i16` columns are arithmetic over
//! the same entry counts, and so is every column that charges fewer than 1326
//! private states: in-range compaction is step 6, `f32` is step 7 and `i16` with
//! a per-node scale is step 10. Nothing here is a measurement of a solve.
//!
//! Run it with `cargo run -p postflop --example memory_table [workers]`.

use cards::{Card, CardSet, Combo, Range};
use postflop::{
    Precision, SolverConfig,
    config::MEMORY_LIMIT_CEILING_BYTES,
    streets::{MemoryRow, PostflopMemory, StoragePlan},
};
use std::{error::Error, fs, path::Path};
use tree::{BetSizeOptions, PostflopTree, PostflopTreeConfig, Street};

/// Decision 4's hard ceiling, from the one constant that owns it.
const CEILING: usize = MEMORY_LIMIT_CEILING_BYTES as usize;

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/solver.toml");

/// Decision 4's configured default, read from the file that owns it rather than
/// repeated here, so a change to the limit cannot leave this table behind.
fn default_limit() -> Result<usize, Box<dyn Error>> {
    if !Path::new(CONFIG).exists() {
        return Err(format!("cannot price a table without the solver config at {CONFIG}").into());
    }
    let config = SolverConfig::load(CONFIG)
        .map_err(|error| format!("cannot read the memory limit from {CONFIG}: {error}"))?;
    Ok(config.memory_limit_mib * 1024 * 1024)
}

/// Decision 9, verbatim from `tests/reference/turn/cases.json`. OOP is the big
/// blind caller and IP is the button opener, which is the order the game takes
/// its ranges in.
const OOP_RANGE: &str = "22-TT, JJ:0.5, QQ:0.25, A2s-AJs, AQs:0.5, K2s-KQs, Q4s-QJs, J6s-JTs, T6s-T9s, 96s-98s, 85s-87s, 74s-76s, 64s-65s, 53s-54s, 43s, A2o-AJo, K9o-KQo, Q9o-QJo, J9o-JTo, T8o-T9o, 98o";
/// Decision 9's button open, same source.
const IP_RANGE: &str = "22+, A2s-AKs, K2s-KQs, Q3s-QJs, J5s-JTs, T6s-T9s, 96s-98s, 86s-87s, 75s-76s, 65s, 54s, A3o-AKo, K8o-KQo, Q9o-QJo, J9o-JTo, T9o";

const CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/reference/turn/cases.json"
);
const FLOPS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/reference/flop/flops.json"
);

/// The three approved turn cases: identifier and four-card board.
const TURN_BOARDS: [(&str, &str); 3] = [
    ("turn_100bb_dry_rainbow", "9c 5d 2h Ks"),
    ("turn_100bb_paired", "8h 8d 3c Ks"),
    ("turn_100bb_flush_possible", "As Js 8s 4h"),
];

/// The flop gate boards: the three-card prefixes of the turn cases, then one
/// rainbow, one two-tone and one monotone flop drawn from the committed
/// 49-flop subset (Decision 8, `tests/reference/flop/flops.json`).
const FLOP_BOARDS: [(&str, &str); 6] = [
    ("prefix of turn_100bb_dry_rainbow (rainbow)", "9c 5d 2h"),
    ("prefix of turn_100bb_paired (paired, rainbow)", "8h 8d 3c"),
    ("prefix of turn_100bb_flush_possible (monotone)", "As Js 8s"),
    ("flops.json AcJd5h (unpaired, rainbow)", "Ac Jd 5h"),
    ("flops.json KcQd3c (unpaired, two-tone)", "Kc Qd 3c"),
    ("flops.json Ac4c3c (unpaired, monotone)", "Ac 4c 3c"),
];

/// One priced configuration.
struct Gate {
    name: &'static str,
    /// Why this tree and not another one.
    provenance: &'static str,
    tree: PostflopTree,
    board_len: usize,
    boards: &'static [(&'static str, &'static str)],
}

fn menus(bets: &str, raises: &str) -> Result<[BetSizeOptions; 2], Box<dyn Error>> {
    let sizes = BetSizeOptions::try_from((bets, raises))?;
    Ok([sizes.clone(), sizes])
}

/// The decided gate menu at the phase 3 chip scale used by
/// `crates/tree/tests/postflop.rs::the_gate_menu_counts_its_nodes_per_street`:
/// 33% pot plus all-in on the flop and the turn, two river sizes with no all-in
/// token, one raise per street at 100% of pot. A 100bb single-raised pot at ten
/// chips per big blind leaves 55 in the pot and 975 behind.
fn gate_tree(start: Street) -> Result<PostflopTree, Box<dyn Error>> {
    Ok(PostflopTree::new(PostflopTreeConfig {
        starting_pot: 55,
        effective_stack: 975,
        min_bet: 10,
        start_street: start,
        sizes: [
            menus("33%,a", "100%,a")?,
            menus("33%,a", "100%,a")?,
            menus("33%,75%", "100%,a")?,
        ],
        max_raises: 1,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })?)
}

/// The same menu at the two-chips-per-big-blind scale the reference capture
/// uses (`tests/reference/turn/cases.json`: pot 11, stack 195, min bet 1).
/// Printed beside the gate tree so a difference in node counts between the two
/// scalings is visible rather than assumed away.
fn capture_scale_tree(start: Street) -> Result<PostflopTree, Box<dyn Error>> {
    Ok(PostflopTree::new(PostflopTreeConfig {
        starting_pot: 11,
        effective_stack: 195,
        min_bet: 1,
        start_street: start,
        sizes: [
            menus("33%,a", "100%,a")?,
            menus("33%,a", "100%,a")?,
            menus("33%,75%", "100%,a")?,
        ],
        max_raises: 1,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })?)
}

fn board(text: &str) -> Result<Vec<Card>, Box<dyn Error>> {
    text.split_ascii_whitespace()
        .map(|card| card.parse::<Card>().map_err(Into::into))
        .collect()
}

/// Combos with positive weight that the board prefix does not block, which is
/// the private-state count step 6's compaction charges.
fn live_combos(range: &Range, board: &[Card]) -> Result<usize, Box<dyn Error>> {
    let dead = CardSet::new(board)?;
    let range = range.without_cards(dead);
    Ok(Combo::all()
        .filter(|combo| range.weight(*combo) > 0.0)
        .count())
}

/// Whitespace-free copy, so a range or a board can be looked for in a JSON file
/// without a JSON parser and without depending on its indentation.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Fail if the constants above have drifted from the committed inputs.
fn check_inputs() -> Result<(), Box<dyn Error>> {
    let cases = compact(&fs::read_to_string(CASES)?);
    for range in [OOP_RANGE, IP_RANGE] {
        if !cases.contains(&compact(range)) {
            return Err(format!("range not found in {CASES}: {range}").into());
        }
    }
    for (id, text) in TURN_BOARDS {
        let cards = text
            .split_ascii_whitespace()
            .map(|card| format!("\"{card}\""))
            .collect::<Vec<_>>()
            .join(",");
        if !cases.contains(&format!("\"board\":[{cards}]")) {
            return Err(format!("board {text} of {id} not found in {CASES}").into());
        }
    }
    let flops = compact(&fs::read_to_string(FLOPS)?);
    for (label, text) in FLOP_BOARDS {
        let Some(name) = label.strip_prefix("flops.json ") else {
            // The other flop boards are the three-card prefixes of the turn
            // cases, and have to stay that way for the two tables to describe
            // the same spots.
            let id = label
                .strip_prefix("prefix of ")
                .and_then(|rest| rest.split_whitespace().next())
                .ok_or_else(|| format!("flop board {label} names neither a case nor a flop"))?;
            let (case, turn) = TURN_BOARDS
                .iter()
                .find(|(case, _)| *case == id)
                .ok_or_else(|| format!("flop board {label} names no turn case"))?;
            let prefix = turn
                .split_ascii_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            if prefix != text {
                return Err(format!(
                    "{text} is not the flop of {case} ({turn}), which is {prefix}"
                )
                .into());
            }
            continue;
        };
        let flop = name.split_whitespace().next().unwrap_or_default();
        if !flops.contains(&format!("\"{flop}\"")) {
            return Err(format!("flop {flop} not found in {FLOPS}").into());
        }
        if compact(text) != flop {
            return Err(format!("flop label {label} does not name the board {text}").into());
        }
    }
    Ok(())
}

fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Bytes in the largest unit that keeps the number readable, alongside the
/// exact count, because the exact count is what a test pins.
fn scaled(bytes: usize) -> String {
    const UNITS: [(&str, f64); 4] = [
        ("GiB", 1024.0 * 1024.0 * 1024.0),
        ("MiB", 1024.0 * 1024.0),
        ("KiB", 1024.0),
        ("B", 1.0),
    ];
    for (unit, size) in UNITS {
        if bytes as f64 >= size {
            return format!("{:.2} {unit}", bytes as f64 / size);
        }
    }
    format!("{bytes} B")
}

fn verdict(bytes: usize, limit: usize, name: &str) -> String {
    if bytes <= limit {
        format!(
            "fits the {name} with {} spare",
            scaled(limit.saturating_sub(bytes))
        )
    } else {
        format!(
            "refused by the {name}: {} over, {:.1}x the limit",
            scaled(bytes.saturating_sub(limit)),
            bytes as f64 / limit as f64
        )
    }
}

fn print_rows(table: &[MemoryRow]) {
    println!(
        "  {:<38} {:>18}  {:<18}  {:<24} representation",
        "row", "bytes", "lifetime", "overlaps another row?"
    );
    for row in table {
        println!(
            "  {:<38} {:>18}  {:<18}  {:<24} {}",
            row.name,
            thousands(row.bytes),
            row.lifetime.as_str(),
            row.overlap.as_str(),
            row.representation
        );
        if !row.note.is_empty() {
            println!("  {:<38} {}", "", row.note);
        }
    }
}

/// Today's layout at `precision`, and the layout step 6 targets: the current
/// policy derived at visit time, no snapshot retained during the solve, and
/// private states compacted to the live combos of `states`.
fn plans(precision: Precision, states: [usize; 2]) -> [(&'static str, StoragePlan); 3] {
    [
        ("today (3 arrays, 2 snapshots, 1326 states)", {
            StoragePlan::today().at(precision)
        }),
        (
            "after step 6 (2 arrays, 0 snapshots, live states)",
            StoragePlan {
                precision,
                states,
                snapshots: 0,
                store_current_policy: false,
            },
        ),
        (
            "after step 6 + browsing (2 arrays, 1 snapshot, live)",
            StoragePlan {
                precision,
                states,
                snapshots: 1,
                store_current_policy: false,
            },
        ),
    ]
}

fn report(gate: &Gate, workers: usize, default: usize) -> Result<(), Box<dyn Error>> {
    let memory = PostflopMemory::for_tree(&gate.tree, gate.board_len, workers)?;
    let today = StoragePlan::today();
    let entries = memory.entries_under(&today)?;
    println!("\n===== {} =====", gate.name);
    println!("{}", gate.provenance);
    println!(
        "compact tree: {} nodes, max depth {}",
        thousands(gate.tree.nodes().len()),
        gate.tree.max_depth()
    );
    println!(
        "expanded: {} public nodes, {} decision nodes, {} board states, {} showdown tables",
        thousands(memory.expanded_nodes),
        thousands(memory.expanded_decision_nodes),
        thousands(memory.board_states),
        thousands(memory.showdown_tables)
    );
    println!(
        "action slots: OOP {}, IP {}; {} entries per stored array at 1326 states",
        thousands(memory.action_slots[0]),
        thousands(memory.action_slots[1]),
        thousands(entries)
    );
    println!("workers charged: {}", memory.workers);

    let ranges = [Range::parse(OOP_RANGE)?, Range::parse(IP_RANGE)?];
    let mut live_by_board = Vec::new();
    println!("\nlive combos after the board prefix, and the entries they compact to:");
    for (label, text) in gate.boards {
        let cards = board(text)?;
        let live = [
            live_combos(&ranges[0], &cards)?,
            live_combos(&ranges[1], &cards)?,
        ];
        let compacted = memory.entries_under(&StoragePlan {
            states: live,
            ..today
        })?;
        println!(
            "  {text:<12} {label:<46} OOP {:>4}  IP {:>4}  entries {:>15} ({:.1}% of full width)",
            live[0],
            live[1],
            thousands(compacted),
            100.0 * compacted as f64 / entries as f64
        );
        live_by_board.push((*label, *text, live, compacted));
    }

    println!("\nrows at f64, today's layout (the sum is the estimate's bound):");
    let table = memory.rows()?;
    print_rows(&table);
    let bound = memory.bound_under(&today)?;
    println!(
        "  {:<38} {:>18}  ({})",
        "counted total",
        thousands(bound),
        scaled(bound)
    );
    if bound != memory.working_set_bound_bytes {
        return Err("the rows do not sum to the estimate".into());
    }
    println!(
        "  {:<38} {:>18}  (PostflopMemory::working_set_bound_bytes, equal by test)",
        "estimate",
        thousands(memory.working_set_bound_bytes)
    );

    println!("\nsums per storage width. f32 and i16 are arithmetic, not measured:");
    println!(
        "  {:<52} {:>18} {:>18} {:>18}",
        "layout", "f64", "f32", "i16"
    );
    // The widest live-combo board is the one a gate has to fit, so the
    // compacted rows below use it rather than the friendliest board.
    let widest = live_by_board
        .iter()
        .max_by_key(|(_, _, _, compacted)| *compacted)
        .ok_or("a gate needs at least one board")?;
    for index in 0..3 {
        let mut sums = Vec::new();
        let mut label = "";
        for precision in [Precision::F64, Precision::F32, Precision::I16] {
            let (name, plan) = plans(precision, widest.2)[index];
            label = name;
            sums.push(memory.bound_under(&plan)?);
        }
        println!(
            "  {label:<52} {:>18} {:>18} {:>18}",
            thousands(sums[0]),
            thousands(sums[1]),
            thousands(sums[2])
        );
    }
    println!(
        "  compacted rows use {} ({} / {} live combos), the widest board above",
        widest.1, widest.2[0], widest.2[1]
    );

    println!("\nagainst the limits:");
    for index in 0..3 {
        for precision in [Precision::F64, Precision::F32, Precision::I16] {
            let (name, plan) = plans(precision, widest.2)[index];
            let bytes = memory.bound_under(&plan)?;
            println!(
                "  {:<52} {:<4} {:>18}  {}; {}",
                name,
                precision.as_str(),
                thousands(bytes),
                verdict(bytes, default, &format!("{} default", scaled(default))),
                verdict(bytes, CEILING, &format!("{} ceiling", scaled(CEILING)))
            );
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let workers: usize = match std::env::args().nth(1) {
        Some(text) => text.parse()?,
        None => 1,
    };
    if workers == 0 {
        return Err("a worker count of zero prices nothing".into());
    }
    check_inputs()?;
    let default = default_limit()?;
    println!(
        "Postflop working-set table, {} worker(s), revision {}",
        workers,
        std::env::var("GITHUB_SHA").unwrap_or_else(|_| "local".into())
    );
    println!(
        "Limits: {} configured in config/solver.toml, {} hard ceiling.",
        scaled(default),
        scaled(CEILING)
    );
    println!(
        "f64 is what the code stores. f32 (step 7), i16 with a per-node f32 scale (step 10) and\n\
         compaction to live combos (step 6) are arithmetic over the same entry counts, not runs."
    );

    let gates = [
        Gate {
            name: "Turn gate",
            provenance: "Decisions 1, 9 and 10 at the phase 3 chip scale (pot 55, stack 975, min bet 10),\n\
                         the tree crates/tree/tests/postflop.rs::the_gate_menu_counts_its_nodes_per_street builds.",
            tree: gate_tree(Street::Turn)?,
            board_len: 4,
            boards: &TURN_BOARDS,
        },
        Gate {
            name: "Flop gate",
            provenance: "The same tree started on the flop: the roadmap's 100bb single-raised-pot gate.",
            tree: gate_tree(Street::Flop)?,
            board_len: 3,
            boards: &FLOP_BOARDS,
        },
    ];
    for gate in &gates {
        report(gate, workers, default)?;
    }

    // The gate menu is priced above at ten chips per big blind, and captured at
    // two (tests/reference/turn/cases.json). Both scalings have to build the
    // same tree for the table to describe the capture, so it is checked rather
    // than assumed.
    let captured = PostflopMemory::for_tree(&capture_scale_tree(Street::Turn)?, 4, workers)?;
    let priced = PostflopMemory::for_tree(&gate_tree(Street::Turn)?, 4, workers)?;
    if captured == priced {
        println!(
            "\nThe turn gate at the capture's chip scale (pot 11, stack 195, min bet 1) expands \
             to the same {} nodes and the same {} byte bound as the table above.",
            thousands(captured.expanded_nodes),
            thousands(captured.working_set_bound_bytes)
        );
    } else {
        println!(
            "\nThe turn gate at the capture's chip scale (pot 11, stack 195, min bet 1) differs: \
             {} nodes and {} bytes against {} nodes and {} bytes above.",
            thousands(captured.expanded_nodes),
            thousands(captured.working_set_bound_bytes),
            thousands(priced.expanded_nodes),
            thousands(priced.working_set_bound_bytes)
        );
    }

    println!("\nreference estimates for the same three turn cases, from the turn-wasm-reference");
    println!("artifact of CI run 34401787355 (reference_memory_estimate_bytes, its own accounting");
    println!("of its own solver, not comparable term by term with the rows above):");
    for (id, bytes, hands) in [
        ("turn_100bb_dry_rainbow", 24_670_040_usize, [469, 470]),
        ("turn_100bb_paired", 18_654_832, [468, 473]),
        ("turn_100bb_flush_possible", 17_239_588, [445, 448]),
    ] {
        println!(
            "  {id:<28} {:>14}  ({}), private hands {} / {}",
            thousands(bytes),
            scaled(bytes),
            hands[0],
            hands[1]
        );
    }
    Ok(())
}
